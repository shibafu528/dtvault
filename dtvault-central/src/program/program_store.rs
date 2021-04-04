use crate::config::Config;
use crate::program::{Persistence, Program as StoredProgram};
use crate::program::{ProgramKey, Video as StoredVideo};
use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as ResponseStatus;
use dtvault_types::shibafu528::dtvault::central::PersistStore;
use dtvault_types::shibafu528::dtvault::Program;
use fs2::FileExt;
use mime::Mime;
use prost::Message;
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

type ProgramStoreBackend = BTreeMap<ProgramKey, Arc<StoredProgram>>;
type VideoStoreBackend = BTreeMap<Uuid, Arc<StoredVideo>>;

#[derive(thiserror::Error, Debug)]
#[error("poisoned lock: another task failed inside")]
pub struct MutexPoisonError;

#[derive(thiserror::Error, Debug)]
pub enum MetadataWriteError<'a> {
    #[error("Program not found (id = {0})")]
    ProgramNotFound(&'a ProgramKey),
    #[error(transparent)]
    Poisoned(#[from] MutexPoisonError),
}

#[derive(thiserror::Error, Debug)]
pub enum VideoWriteError<'a> {
    #[error("Program not found (id = {0})")]
    ProgramNotFound(&'a ProgramKey),
    #[error("Provider ID `{0}` already exists")]
    AlreadyExists(String),
    #[error(transparent)]
    Poisoned(#[from] MutexPoisonError),
}

#[derive(thiserror::Error, Debug)]
pub enum VideoThumbnailUpdateError {
    #[error("Video not found (id = {0})")]
    VideoNotFound(Uuid),
    #[error(transparent)]
    Poisoned(#[from] MutexPoisonError),
}

pub enum FindOrCreateNotice {
    Created,
    AlreadyExists,
}

impl Into<i32> for FindOrCreateNotice {
    fn into(self) -> i32 {
        match self {
            Self::Created => ResponseStatus::Created as i32,
            Self::AlreadyExists => ResponseStatus::AlreadyExists as i32,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InitializeError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Protobuf decode error: {0}")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Protobuf decode error: broken message ({}) in field {}[{}]", .description, .field_name, .index)]
    BrokenMessage {
        field_name: String,
        index: usize,
        description: String,
    },
}

pub struct ProgramStore {
    config: Arc<Config>,
    programs: RwLock<ProgramStoreBackend>,
    videos: RwLock<VideoStoreBackend>,
}

impl ProgramStore {
    pub fn new(config: Arc<Config>) -> Result<Self, InitializeError> {
        let mut programs = ProgramStoreBackend::new();
        let mut videos = VideoStoreBackend::new();

        let path = config.database.programs_file_path();
        if path.is_file() {
            let bin = std::fs::read(path)?;
            let store = PersistStore::decode(&bin[..])?;
            for (index, persisted) in store.programs.into_iter().enumerate() {
                let sp = StoredProgram::from_persisted(persisted).map_err(|err| InitializeError::BrokenMessage {
                    field_name: "programs".to_string(),
                    index,
                    description: format!("{}", err),
                })?;
                programs.insert(ProgramKey::from_stored_program(&sp), Arc::new(sp));
            }
            for (index, persisted) in store.videos.into_iter().enumerate() {
                let sv = StoredVideo::from_persisted(persisted).map_err(|err| InitializeError::BrokenMessage {
                    field_name: "videos".to_string(),
                    index,
                    description: format!("{}", err),
                })?;
                videos.insert(sv.id, Arc::new(sv));
            }

            println!("{} programs, {} videos loaded.", programs.len(), videos.len());
        }

        Ok(ProgramStore {
            config,
            programs: RwLock::new(programs),
            videos: RwLock::new(videos),
        })
    }

    pub fn all(&self) -> Result<Vec<Arc<StoredProgram>>, MutexPoisonError> {
        let store = self.programs.read().map_err(|_| MutexPoisonError)?;
        Ok(store.values().cloned().collect())
    }

    pub fn find(&self, key: &ProgramKey) -> Result<Option<Arc<StoredProgram>>, MutexPoisonError> {
        let store = self.programs.read().map_err(|_| MutexPoisonError)?;
        Ok(store.get(key).map(Arc::clone))
    }

    pub fn find_or_create(
        &self,
        program: Program,
    ) -> Result<(Arc<StoredProgram>, FindOrCreateNotice), MutexPoisonError> {
        self.mutation(|skip| {
            let mut store = self.programs.write().map_err(|_| MutexPoisonError)?;
            let key = ProgramKey::from_program(&program);
            let mut notice = FindOrCreateNotice::AlreadyExists;
            let sp = store
                .entry(key)
                .or_insert_with(|| {
                    notice = FindOrCreateNotice::Created;
                    Arc::new(StoredProgram::from_exchanged(program.clone()).unwrap())
                })
                .clone();
            if let FindOrCreateNotice::AlreadyExists = notice {
                *skip = true;
            }
            Ok((sp, notice))
        })
    }

    pub fn find_video(&self, id: &Uuid) -> Result<Option<Arc<StoredVideo>>, MutexPoisonError> {
        let video = self.videos.read().map_err(|_| MutexPoisonError)?;
        Ok(video.get(id).map(|v| v.clone()))
    }

    pub fn find_videos(&self, ids: &[Uuid]) -> Result<Vec<Option<Arc<StoredVideo>>>, MutexPoisonError> {
        let video = self.videos.read().map_err(|_| MutexPoisonError)?;
        let mut result = vec![];
        for id in ids {
            if let Some(v) = video.get(id) {
                result.push(Some(v.clone()));
            } else {
                result.push(None);
            }
        }
        Ok(result)
    }

    pub fn create_video<'a>(
        &'a self,
        key: &'a ProgramKey,
        video: StoredVideo,
    ) -> Result<Arc<StoredVideo>, VideoWriteError<'a>> {
        self.mutation(|_| {
            let mut programs = self.programs.write().map_err(|_| MutexPoisonError)?;
            let mut videos = self.videos.write().map_err(|_| MutexPoisonError)?;
            let mut program = match programs.get(key) {
                Some(p) => (**p).clone(),
                None => return Err(VideoWriteError::ProgramNotFound(key)),
            };

            for video_id in program.video_ids() {
                if let Some(v) = videos.get(video_id) {
                    if v.provider_id == video.provider_id {
                        return Err(VideoWriteError::AlreadyExists(video.provider_id.clone()));
                    }
                }
            }

            let video = Arc::new(video);
            program.video_ids_mut().push(video.id);
            programs.insert(key.clone(), Arc::new(program));
            videos.insert(video.id, video.clone()); // TODO: VideoID重複チェック

            Ok(video)
        })
    }

    pub fn update_program_metadata<'a>(
        &'a self,
        key: &'a ProgramKey,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<(), MetadataWriteError<'a>> {
        self.mutation(|_| {
            let mut store = self.programs.write().map_err(|_| MutexPoisonError)?;
            match store.get(key) {
                Some(sp) => {
                    let mut sp = (**sp).clone();
                    sp.metadata_mut()
                        .insert(metadata_key.to_string(), metadata_value.to_string());
                    store.insert(key.clone(), Arc::new(sp));
                    Ok(())
                }
                None => Err(MetadataWriteError::ProgramNotFound(key)),
            }
        })
    }

    pub fn update_video_thumbnail(
        &self,
        id: &Uuid,
        bin: Vec<u8>,
        mime_type: Mime,
    ) -> Result<(), VideoThumbnailUpdateError> {
        self.mutation(|_| {
            let mut store = self.videos.write().map_err(|_| MutexPoisonError)?;
            match store.get(id) {
                Some(video) => {
                    let mut video = (**video).clone();
                    video.thumbnail = bin;
                    video.thumbnail_mime_type = Some(mime_type);
                    store.insert(id.clone(), Arc::new(video));
                    Ok(())
                }
                None => Err(VideoThumbnailUpdateError::VideoNotFound(id.clone())),
            }
        })
    }

    fn mutation<F: FnOnce(&mut bool) -> Result<T, U>, T, U: From<MutexPoisonError>>(&self, op: F) -> Result<T, U> {
        let mut skip = false;
        let result = op(&mut skip)?;
        if !skip {
            self.persist()?;
        }
        Ok(result)
    }

    // TODO: 非同期化する
    fn persist(&self) -> Result<(), MutexPoisonError> {
        let programs = self.programs.read().map_err(|_| MutexPoisonError)?;
        let videos = self.videos.read().map_err(|_| MutexPoisonError)?;

        let path = self.config.database.programs_file_path();
        let file = std::fs::File::create(path).unwrap();
        file.lock_exclusive().unwrap();

        let mut writer = std::io::BufWriter::new(&file);
        let persisted = PersistStore {
            programs: programs.values().map(|p| p.persist()).collect(),
            videos: videos.values().map(|v| v.persist()).collect(),
        };
        let mut buf: Vec<u8> = vec![];
        persisted.encode(&mut buf).unwrap();
        writer.write(&buf).unwrap();
        writer.flush().unwrap();

        file.unlock().unwrap();
        Ok(())
    }
}

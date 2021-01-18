use crate::program::{Persistence, Program as StoredProgram};
use crate::program::{ProgramKey, Video as StoredVideo};
use crate::Config;
use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as ResponseStatus;
use dtvault_types::shibafu528::dtvault::central::PersistProgram;
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Header as VideoHeader;
use dtvault_types::shibafu528::dtvault::Program;
use fs2::FileExt;
use prost::bytes::Buf;
use prost::Message;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

type ProgramStoreBackend = BTreeMap<ProgramKey, Arc<StoredProgram>>;
type StoreReadPoisonError<'a> = PoisonError<RwLockReadGuard<'a, ProgramStoreBackend>>;
type StoreWritePoisonError<'a> = PoisonError<RwLockWriteGuard<'a, ProgramStoreBackend>>;

pub enum MetadataWriteError<'a> {
    ProgramNotFound(&'a ProgramKey),
    PoisonError(StoreWritePoisonError<'a>),
}

impl<'a> From<StoreWritePoisonError<'a>> for MetadataWriteError<'a> {
    fn from(err: StoreWritePoisonError<'a>) -> Self {
        Self::PoisonError(err)
    }
}

impl fmt::Display for MetadataWriteError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramNotFound(key) => write!(f, "Program not found (id = {})", key),
            Self::PoisonError(e) => write!(f, "{}", e),
        }
    }
}

pub enum VideoWriteError<'a> {
    ProgramNotFound(&'a ProgramKey),
    AlreadyExists(String),
    PoisonError(StoreWritePoisonError<'a>),
}

impl<'a> From<StoreWritePoisonError<'a>> for VideoWriteError<'a> {
    fn from(err: StoreWritePoisonError<'a>) -> Self {
        Self::PoisonError(err)
    }
}

impl fmt::Display for VideoWriteError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramNotFound(key) => write!(f, "Program not found (id = {})", key),
            Self::AlreadyExists(s) => write!(f, "Provider ID `{}` already exists", s),
            Self::PoisonError(e) => write!(f, "{}", e),
        }
    }
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
    #[error("Protobuf decode error: broken message ({}) in position {}", .description, .position)]
    BrokenMessage { position: usize, description: String },
}

pub struct ProgramStore {
    config: Arc<Config>,
    store: RwLock<ProgramStoreBackend>,
}

impl ProgramStore {
    pub fn new(config: Arc<Config>) -> Result<Self, InitializeError> {
        let mut store = ProgramStoreBackend::new();

        let path = config.programs_file_path();
        if path.is_file() {
            let bin = std::fs::read(path)?;
            let mut buf = &bin[..];
            while buf.has_remaining() {
                let position = bin.len() - buf.remaining(); // for error report
                let persisted = PersistProgram::decode_length_delimited(&mut buf)?;
                let sp = StoredProgram::from_persisted(persisted).map_err(|err| InitializeError::BrokenMessage {
                    position,
                    description: format!("{}", err),
                })?;
                store.insert(ProgramKey::from_stored_program(&sp), Arc::new(sp));
            }

            println!("{} programs loaded.", store.len());
        }

        Ok(ProgramStore {
            config,
            store: RwLock::new(store),
        })
    }

    pub fn all(&self) -> Result<Vec<Arc<StoredProgram>>, StoreReadPoisonError> {
        let store = self.store.read()?;
        Ok(store.values().cloned().collect())
    }

    pub fn find(&self, key: &ProgramKey) -> Result<Option<Arc<StoredProgram>>, StoreReadPoisonError> {
        let store = self.store.read()?;
        Ok(store.get(key).map(Arc::clone))
    }

    pub fn find_or_create(
        &self,
        program: Program,
    ) -> Result<(Arc<StoredProgram>, FindOrCreateNotice), StoreWritePoisonError> {
        let mut store = self.store.write()?;
        let key = ProgramKey::from_program(&program);
        let mut notice = FindOrCreateNotice::AlreadyExists;
        let sp = store
            .entry(key)
            .or_insert_with(|| {
                notice = FindOrCreateNotice::Created;
                Arc::new(StoredProgram::from_exchanged(program.clone()).unwrap())
            })
            .clone();
        if let FindOrCreateNotice::Created = notice {
            self.persist(&store);
        }
        Ok((sp, notice))
    }

    pub fn create_video<'a>(
        &'a self,
        key: &'a ProgramKey,
        video_header: VideoHeader,
    ) -> Result<Arc<StoredVideo>, VideoWriteError<'a>> {
        let mut store = self.store.write()?;
        let mut program = match store.get(key) {
            Some(p) => (**p).clone(),
            None => return Err(VideoWriteError::ProgramNotFound(key)),
        };

        for video in program.videos() {
            if video.provider_id == video_header.provider_id {
                return Err(VideoWriteError::AlreadyExists(video_header.provider_id.clone()));
            }
        }

        let video = Arc::new(StoredVideo::from_exchanged(&program, video_header));
        program.videos_mut().push(video.clone());
        store.insert(key.clone(), Arc::new(program));
        self.persist(&store);

        Ok(video)
    }

    pub fn update_program_metadata<'a>(
        &'a self,
        key: &'a ProgramKey,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<(), MetadataWriteError<'a>> {
        let mut store = self.store.write()?;
        match store.get(key) {
            Some(sp) => {
                let mut sp = (**sp).clone();
                sp.metadata_mut()
                    .insert(metadata_key.to_string(), metadata_value.to_string());
                store.insert(key.clone(), Arc::new(sp));
                self.persist(&store);
                Ok(())
            }
            None => Err(MetadataWriteError::ProgramNotFound(key)),
        }
    }

    // TODO: 非同期化する
    fn persist<'a>(&self, store: &RwLockWriteGuard<'a, ProgramStoreBackend>) {
        let path = self.config.programs_file_path();
        let file = std::fs::File::create(path).unwrap();
        file.lock_exclusive().unwrap();

        let mut writer = std::io::BufWriter::new(&file);
        for program in store.values() {
            let mut buf: Vec<u8> = vec![];
            program.persist().encode_length_delimited(&mut buf).unwrap();
            writer.write(&buf).unwrap();
        }
        writer.flush().unwrap();

        file.unlock().unwrap();
    }
}

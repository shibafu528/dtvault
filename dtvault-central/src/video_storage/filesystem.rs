use crate::program::{Program, Video};
use crate::video_storage::storage::*;
use fs2::FileExt;
use pin_project::{pin_project, pinned_drop};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadBuf};
use uuid::Uuid;

const FILE_PROGRAM: &str = "program.json";
const FILE_PROGRAM_METADATA: &str = "metadata.json";
const FILE_VIDEO: &str = "video.json";

pub struct FileSystem {
    root_dir: String,
    lock_file_path: PathBuf,
}

// TODO: 全体的に、一時ファイルを用いた安全なファイル更新を行いたい (QtのQSaveFileのような)
impl FileSystem {
    pub fn new(root_dir: String) -> Self {
        let lock_file_path = PathBuf::from(&root_dir).join(".dtvault_storage");
        FileSystem {
            root_dir,
            lock_file_path,
        }
    }

    fn prepare_lock_file(&self, mut file: &std::fs::File) -> Result<(), UnavailableError> {
        let stat = file.metadata().map_err(|e| UnavailableError {
            reason: format!("Error in read .dtvault_storage: {}", e),
        })?;
        if stat.len() == 0 {
            file.lock_exclusive().map_err(|e| UnavailableError {
                reason: format!("Error in lock .dtvault_storage: {}", e),
            })?;

            let meta = Metadata::new();
            match serde_json::to_string(&meta) {
                Ok(meta_json) => match file.write_all(meta_json.as_bytes()) {
                    Ok(_) => match file.seek(std::io::SeekFrom::Start(0)) {
                        Ok(_) => eprintln!("Initialized storage `{}`: UUID = {}", self.root_dir, meta.id),
                        Err(e) => {
                            return Err(UnavailableError {
                                reason: format!("Error in post-process write storage metadata: {}", e),
                            });
                        }
                    },
                    Err(e) => {
                        return Err(UnavailableError {
                            reason: format!("Error in writing storage metadata: {}", e),
                        });
                    }
                },
                Err(e) => {
                    return Err(UnavailableError {
                        reason: format!("Error in preparing storage metadata: {}", e),
                    });
                }
            }

            file.unlock().map_err(|e| UnavailableError {
                reason: format!("Error in unlock .dtvault_storage: {}", e),
            })?;
        }

        Ok(())
    }

    fn take_shared_lock(&self) -> Result<FSSharedLock, UnavailableError> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&self.lock_file_path)
            .map_err(|e| UnavailableError {
                reason: format!("Can't create lock file: {}", e),
            })?;
        self.prepare_lock_file(&mut file)?;

        if let Err(e) = file.lock_shared() {
            return Err(UnavailableError { reason: e.to_string() });
        }

        let mut meta_json = String::new();
        file.read_to_string(&mut meta_json).map_err(|e| UnavailableError {
            reason: format!("Error in reading storage metadata: {}", e),
        })?;
        let meta = serde_json::from_str(&meta_json).map_err(|e| UnavailableError {
            reason: format!("Error in reading storage metadata: {}", e),
        })?;

        Ok(FSSharedLock::new(file, meta))
    }

    // TODO: multi storage support
    fn find_video_dir(&self, video: &Video) -> PathBuf {
        PathBuf::from(&self.root_dir)
            .join(&video.storage_prefix)
            .join(video.stringify_id())
    }

    async fn create_video_dir(&self, video: &Video) -> Result<PathBuf, CreateError> {
        let video_dir = self.find_video_dir(video);
        if video_dir.is_file() {
            return Err(CreateError::CantCreateDirectory);
        }
        if !video_dir.exists() {
            if let Err(_) = tokio::fs::create_dir(&video_dir).await {
                return Err(CreateError::CantCreateDirectory);
            }
        }

        Ok(video_dir)
    }

    async fn store_metadata(&self, video_dir: &PathBuf, program: &Program, video: &Video) -> Result<(), CreateError> {
        async fn write_json(path: PathBuf, json: String) -> Result<(), CreateError> {
            match tokio::fs::File::create(path).await {
                Ok(mut f) => match f.write_all(json.as_bytes()).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(CreateError::MetadataBackupFailed(e.to_string())),
                },
                Err(e) => Err(CreateError::MetadataBackupFailed(e.to_string())),
            }
        }

        let program_json = match serde_json::to_string_pretty(&program) {
            Ok(json) => json,
            Err(e) => return Err(CreateError::MetadataBackupFailed(e.to_string())),
        };
        write_json(video_dir.join(FILE_PROGRAM), program_json).await?;

        let metadata_json = match serde_json::to_string_pretty(program.metadata()) {
            Ok(json) => json,
            Err(e) => return Err(CreateError::MetadataBackupFailed(e.to_string())),
        };
        write_json(video_dir.join(FILE_PROGRAM_METADATA), metadata_json).await?;

        let video_json = match serde_json::to_string_pretty(video) {
            Ok(json) => json,
            Err(e) => return Err(CreateError::MetadataBackupFailed(e.to_string())),
        };
        write_json(video_dir.join(FILE_VIDEO), video_json).await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl Storage for FileSystem {
    fn is_available(&self) -> bool {
        if let Ok(_) = self.take_shared_lock() {
            true
        } else {
            false
        }
    }

    async fn storage_id(&self) -> Result<Uuid, UnavailableError> {
        let lock = self.take_shared_lock()?;
        Ok(lock.metadata.id)
    }

    async fn find_bin(&self, video: &Video) -> Result<Pin<Box<dyn StorageReader + Send>>, FindStatusError> {
        let lock = self.take_shared_lock()?;
        if !verify_storage_id(video, &lock.metadata) {
            return Err(FindStatusError::Unavailable(UnavailableError {
                reason: format!(
                    "Storage ID mismatched (Required = {}, Mounted = {})",
                    video.storage_id, lock.metadata.id
                ),
            }));
        }

        let video_dir = self.find_video_dir(video);
        if !video_dir.is_dir() {
            return Err(FindStatusError::NotFound);
        }
        let path = video_dir.as_path().join(&video.file_name);
        let file = tokio::fs::File::open(path).await?;

        Ok(Box::pin(FSReader::new(file, lock)))
    }

    async fn create(
        &self,
        program: &Program,
        video: &Video,
    ) -> Result<Pin<Box<dyn StorageWriter + Send>>, CreateError> {
        let lock = self.take_shared_lock()?;
        if !verify_storage_id(video, &lock.metadata) {
            return Err(CreateError::Unavailable(UnavailableError {
                reason: format!(
                    "Storage ID mismatched (Required = {}, Mounted = {})",
                    video.storage_id, lock.metadata.id
                ),
            }));
        }

        let video_dir = self.create_video_dir(video).await?;
        self.store_metadata(&video_dir, program, video).await?;

        let path = video_dir.as_path().join(&video.file_name);
        let file = match tokio::fs::File::create(path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CreateError::Unavailable(UnavailableError { reason: e.to_string() }));
            }
        };

        Ok(Box::pin(FSWriter::new(file, video_dir, lock)))
    }
}

pub struct FSSharedLock {
    file: std::fs::File,
    metadata: Metadata,
    unlocked: bool,
}

impl FSSharedLock {
    fn new(file: std::fs::File, metadata: Metadata) -> Self {
        FSSharedLock {
            file,
            metadata,
            unlocked: false,
        }
    }

    pub fn unlock(&mut self) -> std::io::Result<()> {
        let result = self.file.unlock();

        if let Ok(_) = result {
            self.unlocked = true;
        }

        result
    }
}

impl Drop for FSSharedLock {
    fn drop(&mut self) {
        if !self.unlocked {
            let _ = self.file.unlock();
        }
    }
}

#[pin_project(PinnedDrop)]
pub struct FSWriter {
    #[pin]
    writer: BufWriter<File>,
    parent: PathBuf,
    lock: FSSharedLock,
    finished: bool,
}

impl FSWriter {
    fn new(file: File, parent: PathBuf, lock: FSSharedLock) -> Self {
        FSWriter {
            writer: BufWriter::new(file),
            parent,
            lock,
            finished: false,
        }
    }
}

#[pinned_drop]
impl PinnedDrop for FSWriter {
    fn drop(self: Pin<&mut Self>) {
        if !self.finished {
            let _ = std::fs::remove_dir_all(&self.parent);
        }
    }
}

#[tonic::async_trait]
impl StorageWriter for FSWriter {
    async fn finish(self: Pin<&mut Self>) -> Result<(), std::io::Error> {
        let this = self.project();
        if !*this.finished {
            this.lock.unlock()?;
            *this.finished = true;
        }
        Ok(())
    }

    async fn abort(self: Pin<&mut Self>) -> Result<(), std::io::Error> {
        let this = self.project();
        if !*this.finished {
            std::fs::remove_dir_all(&this.parent)?;
            *this.finished = true;
        }
        Ok(())
    }
}

impl AsyncWrite for FSWriter {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, std::io::Error>> {
        AsyncWrite::poll_write(self.project().writer, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        AsyncWrite::poll_flush(self.project().writer, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        AsyncWrite::poll_shutdown(self.project().writer, cx)
    }
}

#[pin_project]
pub struct FSReader {
    #[pin]
    reader: BufReader<File>,
    lock: FSSharedLock,
}

impl FSReader {
    fn new(file: File, lock: FSSharedLock) -> Self {
        FSReader {
            reader: BufReader::new(file),
            lock,
        }
    }
}

impl AsyncRead for FSReader {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        AsyncRead::poll_read(self.project().reader, cx, buf)
    }
}

impl StorageReader for FSReader {}

#[derive(Serialize, Deserialize, Debug)]
struct Metadata {
    #[serde(with = "crate::serde::uuid")]
    id: Uuid,
}

impl Metadata {
    fn new() -> Self {
        Metadata { id: Uuid::new_v4() }
    }
}

fn verify_storage_id(video: &Video, metadata: &Metadata) -> bool {
    video.storage_id == metadata.id
}

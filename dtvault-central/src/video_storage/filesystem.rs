use crate::program::{Program, Video};
use crate::video_storage::storage::*;
use fs2::FileExt;
use pin_project::{pin_project, pinned_drop};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
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

    fn prepare_lock_file(&self, mut file: std::fs::File) -> Result<std::fs::File, UnavailableError> {
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
                    Ok(_) => {
                        eprintln!("Initialized storage `{}`: UUID = {}", self.root_dir, meta.id);
                    }
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

        Ok(file)
    }

    fn take_shared_lock(&self) -> Result<FSSharedLock, UnavailableError> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&self.lock_file_path)
            .map_err(|e| UnavailableError {
                reason: format!("Can't create lock file: {}", e),
            })?;
        let file = self.prepare_lock_file(file)?;

        if let Err(e) = file.lock_shared() {
            return Err(UnavailableError { reason: e.to_string() });
        }

        Ok(FSSharedLock::new(file))
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
impl Storage<FSReader, FSWriter> for FileSystem {
    fn is_available(&self) -> bool {
        if let Ok(_) = self.take_shared_lock() {
            true
        } else {
            false
        }
    }

    async fn find_bin(&self, video: &Video) -> Result<FSReader, FindStatusError> {
        let lock = self.take_shared_lock()?;

        let video_dir = self.find_video_dir(video);
        if !video_dir.is_dir() {
            return Err(FindStatusError::NotFound);
        }
        let path = video_dir.as_path().join(&video.file_name);
        let file = tokio::fs::File::open(path).await?;

        Ok(FSReader::new(file, lock))
    }

    async fn create(&self, program: &Program, video: &Video) -> Result<FSWriter, CreateError> {
        let lock = self.take_shared_lock()?;

        let video_dir = self.create_video_dir(video).await?;
        self.store_metadata(&video_dir, program, video).await?;

        let path = video_dir.as_path().join(&video.file_name);
        let file = match tokio::fs::File::create(path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CreateError::Unavailable(UnavailableError { reason: e.to_string() }));
            }
        };

        Ok(FSWriter::new(file, video_dir, lock))
    }
}

pub struct FSSharedLock {
    file: std::fs::File,
    unlocked: bool,
}

impl FSSharedLock {
    fn new(file: std::fs::File) -> Self {
        FSSharedLock { file, unlocked: false }
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
    file: File,
    parent: PathBuf,
    lock: FSSharedLock,
    finished: bool,
}

impl FSWriter {
    fn new(file: File, parent: PathBuf, lock: FSSharedLock) -> Self {
        FSWriter {
            file,
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

impl StorageWriter for FSWriter {
    fn finish(&mut self) -> Result<(), std::io::Error> {
        if !self.finished {
            self.lock.unlock()?;
            self.finished = true;
        }
        Ok(())
    }

    fn abort(&mut self) -> Result<(), std::io::Error> {
        if !self.finished {
            std::fs::remove_dir_all(&self.parent)?;
            self.finished = true;
        }
        Ok(())
    }
}

impl AsyncWrite for FSWriter {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, std::io::Error>> {
        AsyncWrite::poll_write(self.project().file, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        AsyncWrite::poll_flush(self.project().file, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        AsyncWrite::poll_shutdown(self.project().file, cx)
    }
}

#[pin_project]
pub struct FSReader {
    #[pin]
    file: File,
    lock: FSSharedLock,
}

impl FSReader {
    fn new(file: File, lock: FSSharedLock) -> Self {
        FSReader { file, lock }
    }
}

impl AsyncRead for FSReader {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, std::io::Error>> {
        AsyncRead::poll_read(self.project().file, cx, buf)
    }
}

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

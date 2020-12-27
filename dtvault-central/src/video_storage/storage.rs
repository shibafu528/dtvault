use crate::program::{Program, Video};
use fs2::FileExt;
use pin_project::pin_project;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::AsyncWrite;
use uuid::Uuid;

#[tonic::async_trait]
pub trait Storage<T: AsyncWrite> {
    fn is_available(&self) -> bool;
    fn find_bin(&self);
    async fn create(&self, program: &Program, video: &Video) -> Result<T, CreateError>;
}

pub struct FileSystem {
    root_dir: String,
    lock_file_path: PathBuf,
}

impl FileSystem {
    pub fn new(root_dir: String) -> Self {
        let lock_file_path = PathBuf::from(&root_dir).join(".dtvault_storage");
        FileSystem {
            root_dir,
            lock_file_path,
        }
    }

    fn take_shared_lock(&self) -> Result<FSSharedLock, UnavailableError> {
        if !self.is_available() {
            return Err(UnavailableError {
                reason: "Can't create lock file".to_string(),
            });
        }

        let file = match std::fs::File::open(&self.lock_file_path) {
            Ok(f) => f,
            Err(e) => return Err(UnavailableError { reason: e.to_string() }),
        };
        if let Err(e) = file.lock_shared() {
            return Err(UnavailableError { reason: e.to_string() });
        }

        Ok(FSSharedLock::new(file))
    }

    async fn create_video_dir(&self, video: &Video) -> Result<PathBuf, CreateError> {
        let video_dir = PathBuf::from(&self.root_dir).join(
            &video
                .id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
        );
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

    async fn store_metadata(&self, program: &Program) -> Result<(), CreateError> {
        // TODO: backup program.json, metadata.json
        unimplemented!()
    }
}

#[tonic::async_trait]
impl Storage<FSWriter> for FileSystem {
    fn is_available(&self) -> bool {
        if self.lock_file_path.exists() {
            return true;
        }

        if let Ok(_) = std::fs::File::create(&self.lock_file_path) {
            true
        } else {
            false
        }
    }

    fn find_bin(&self) {
        // TODO: find ts, mp4, orelse and return stream
        unimplemented!()
    }

    async fn create(&self, program: &Program, video: &Video) -> Result<FSWriter, CreateError> {
        let lock = self.take_shared_lock()?;

        let video_dir = self.create_video_dir(video).await?;
        self.store_metadata(program).await?;

        let path = video_dir.as_path().join(&video.file_name);
        let file = match tokio::fs::File::create(path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CreateError::Unavailable(UnavailableError { reason: e.to_string() }));
            }
        };

        Ok(FSWriter { file, lock })
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Target storage is unavailable now: {}", .reason)]
pub struct UnavailableError {
    reason: String,
}

#[derive(thiserror::Error, Debug)]
pub enum CreateError {
    #[error(transparent)]
    Unavailable(#[from] UnavailableError),
    #[error("Can't create video directory")]
    CantCreateDirectory,
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

#[pin_project]
pub struct FSWriter {
    #[pin]
    file: File,
    lock: FSSharedLock,
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

use crate::program::{Program, Video};
use tokio::io::{AsyncRead, AsyncWrite};

#[tonic::async_trait]
pub trait Storage<R: AsyncRead, W: AsyncWrite + StorageWriter> {
    fn is_available(&self) -> bool;
    async fn find_bin(&self, video: &Video) -> Result<R, FindStatusError>;
    async fn create(&self, program: &Program, video: &Video) -> Result<W, CreateError>;
}

pub trait StorageWriter {
    fn finish(&mut self) -> Result<(), std::io::Error>;
    fn abort(&mut self) -> Result<(), std::io::Error>;
}

#[derive(thiserror::Error, Debug)]
#[error("Target storage is unavailable now: {}", .reason)]
pub struct UnavailableError {
    pub reason: String,
}

#[derive(thiserror::Error, Debug)]
pub enum CreateError {
    #[error(transparent)]
    Unavailable(#[from] UnavailableError),
    #[error("Can't create video directory")]
    CantCreateDirectory,
    // TODO: MetadataBackupFailedは型を独立させたほうが取り回しやすいかも
    #[error("Metadata backup failed: {0}")]
    MetadataBackupFailed(String),
}

#[derive(thiserror::Error, Debug)]
pub enum FindStatusError {
    #[error(transparent)]
    Unavailable(#[from] UnavailableError),
    #[error("Video not found")]
    NotFound,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Error reading status: {0}")]
    ReadError(String),
}

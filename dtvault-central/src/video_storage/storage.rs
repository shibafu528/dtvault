use crate::program::{Program, Video};
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

pub type IStorage = dyn Storage + Send + Sync;

#[tonic::async_trait]
pub trait Storage {
    fn is_available(&self) -> bool;
    fn label(&self) -> &str;
    async fn storage_id(&self) -> Result<Uuid, UnavailableError>;
    async fn find_bin(&self, video: &Video) -> Result<Pin<Box<dyn StorageReader + Send>>, FindStatusError>;
    async fn create(&self, program: &Program, video: &Video)
        -> Result<Pin<Box<dyn StorageWriter + Send>>, CreateError>;
}

pub trait StorageReader: AsyncRead {}

#[tonic::async_trait]
pub trait StorageWriter: AsyncWrite {
    async fn finish(self: Pin<&mut Self>) -> Result<(), std::io::Error>;
    async fn abort(self: Pin<&mut Self>) -> Result<(), std::io::Error>;
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

use crate::program::{Program, Video};
use crate::video_storage::{CreateError, FindStatusError, Storage, StorageWriter, UnavailableError};
use pin_project::pin_project;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncWrite, SeekFrom};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct Tempfile {
    storage_id: Uuid,
    files: Arc<RwLock<BTreeMap<Uuid, File>>>,
}

impl Tempfile {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Tempfile {
            storage_id: Uuid::new_v4(),
            files: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

#[tonic::async_trait]
impl Storage<File, Writer> for Tempfile {
    fn is_available(&self) -> bool {
        true
    }

    async fn storage_id(&self) -> Result<Uuid, UnavailableError> {
        Ok(self.storage_id)
    }

    async fn find_bin(&self, video: &Video) -> Result<File, FindStatusError> {
        let files = self.files.read().await;
        match files.get(&video.id) {
            Some(f) => {
                let mut f = f.try_clone().await?;
                // NOTE: テスト用なので、同じファイルに対して同時にアクセスされた場合シーク位置が共有されて壊れるのは気にしない
                f.seek(SeekFrom::Start(0)).await?;
                Ok(f)
            }
            None => Err(FindStatusError::NotFound),
        }
    }

    async fn create(&self, _program: &Program, video: &Video) -> Result<Writer, CreateError> {
        let file = File::from_std(
            tempfile::tempfile().map_err(|e| CreateError::Unavailable(UnavailableError { reason: e.to_string() }))?,
        );
        Ok(Writer {
            file,
            video_id: video.id,
            files: self.files.clone(),
        })
    }
}

#[pin_project]
pub struct Writer {
    #[pin]
    file: File,
    video_id: Uuid,
    files: Arc<RwLock<BTreeMap<Uuid, File>>>,
}

#[tonic::async_trait]
impl StorageWriter for Writer {
    async fn finish(&mut self) -> Result<(), std::io::Error> {
        let mut files = self.files.write().await;
        files.insert(self.video_id, self.file.try_clone().await?);
        Ok(())
    }

    async fn abort(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl AsyncWrite for Writer {
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

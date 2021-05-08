use crate::program::{Program, Video};
use crate::video_storage::{CreateError, FindStatusError, Storage, StorageReader, StorageWriter, UnavailableError};
use pin_project::pin_project;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadBuf, SeekFrom};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct Tempfile {
    label: String,
    storage_id: Uuid,
    files: Arc<RwLock<BTreeMap<Uuid, File>>>,
}

impl Tempfile {
    pub fn new(label: String) -> Self {
        Tempfile {
            label,
            storage_id: Uuid::new_v4(),
            files: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

#[tonic::async_trait]
impl Storage for Tempfile {
    fn is_available(&self) -> bool {
        true
    }

    fn label(&self) -> &str {
        &self.label
    }

    async fn storage_id(&self) -> Result<Uuid, UnavailableError> {
        Ok(self.storage_id)
    }

    async fn find_bin(&self, video: &Video) -> Result<Pin<Box<dyn StorageReader + Send>>, FindStatusError> {
        let files = self.files.read().await;
        match files.get(&video.id) {
            Some(f) => {
                let mut f = f.try_clone().await?;
                // NOTE: テスト用なので、同じファイルに対して同時にアクセスされた場合シーク位置が共有されて壊れるのは気にしない
                f.seek(SeekFrom::Start(0)).await?;
                Ok(Box::pin(Reader {
                    reader: BufReader::new(f),
                }))
            }
            None => Err(FindStatusError::NotFound),
        }
    }

    async fn create(
        &self,
        _program: &Program,
        video: &Video,
    ) -> Result<Pin<Box<dyn StorageWriter + Send>>, CreateError> {
        let file = File::from_std(
            tempfile::tempfile().map_err(|e| CreateError::Unavailable(UnavailableError { reason: e.to_string() }))?,
        );
        Ok(Box::pin(Writer {
            writer: BufWriter::new(file),
            video_id: video.id,
            files: self.files.clone(),
        }))
    }
}

#[pin_project]
pub struct Reader {
    #[pin]
    reader: BufReader<File>,
}

impl AsyncRead for Reader {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        AsyncRead::poll_read(self.project().reader, cx, buf)
    }
}

impl StorageReader for Reader {}

#[pin_project]
pub struct Writer {
    #[pin]
    writer: BufWriter<File>,
    video_id: Uuid,
    files: Arc<RwLock<BTreeMap<Uuid, File>>>,
}

#[tonic::async_trait]
impl StorageWriter for Writer {
    async fn finish(self: Pin<&mut Self>) -> Result<(), std::io::Error> {
        let mut this = self.project();
        this.writer.flush().await?;
        let mut files = this.files.write().await;
        files.insert(*this.video_id, this.writer.get_ref().try_clone().await?);
        Ok(())
    }

    async fn abort(self: Pin<&mut Self>) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl AsyncWrite for Writer {
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

mod filesystem;
mod storage;

pub use self::filesystem::*;
pub use self::storage::*;
use crate::program::{validate_program_id, CachedProgramFinder, ProgramKey, ProgramStore, Video, VideoWriteError};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Part as VideoPart;
use dtvault_types::shibafu528::dtvault::storage::get_video_response::Datagram as GetVideoResponseDatagram;
use dtvault_types::shibafu528::dtvault::storage::get_video_response::Part as GetVideoResponsePart;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageService as VideoStorageServiceTrait;
use dtvault_types::shibafu528::dtvault::storage::{
    CreateVideoRequest, CreateVideoResponse, GetVideoRequest, GetVideoResponse,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::BufWriter;
use tokio::prelude::*;
use tokio::stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

fn map_io_error(e: tokio::io::Error) -> Status {
    Status::internal(format!("IO error: {}", e))
}

pub struct VideoStorageService {
    store: Arc<ProgramStore>,
    storage: Arc<FileSystem>,
}

impl VideoStorageService {
    pub fn new(store: Arc<ProgramStore>, storage: Arc<FileSystem>) -> Self {
        VideoStorageService { store, storage }
    }
}

#[tonic::async_trait]
impl VideoStorageServiceTrait for VideoStorageService {
    async fn create_video(
        &self,
        request: Request<tonic::Streaming<CreateVideoRequest>>,
    ) -> Result<Response<CreateVideoResponse>, Status> {
        let mut stream = request.into_inner();

        let (program, header) = match stream.next().await {
            Some(msg) => {
                let msg = msg?;
                let part = match msg.part {
                    Some(part) => Ok(part),
                    None => Err(Status::invalid_argument("Missing value: part")),
                }?;

                match part {
                    VideoPart::Header(h) => {
                        println!("CreateVideo {:#?}", h);

                        let program_id = match h.program_id.as_ref() {
                            Some(program_id) => match validate_program_id(&program_id) {
                                Ok(_) => Ok(program_id),
                                Err(msg) => {
                                    Err(Status::invalid_argument(format!("Violation in program_id => {}", msg)))
                                }
                            },
                            None => Err(Status::invalid_argument("Missing value: program_id")),
                        }?;

                        let program_key = ProgramKey::from_program_id(&program_id);
                        match self.store.find(&program_key) {
                            Ok(result) => match result {
                                Some(program) => Ok((program, h)),
                                None => Err(Status::not_found(format!("Program not found (id = {})", program_key))),
                            },
                            Err(e) => Err(Status::aborted(format!("{}", e))),
                        }
                    }
                    _ => Err(Status::invalid_argument("Invalid part: need header")),
                }
            }
            None => Err(Status::invalid_argument("Empty stream")),
        }?;

        let videos = match self.store.find_videos(program.video_ids()) {
            Ok(v) => Ok(v),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;
        for video in videos.into_iter().filter_map(|v| v) {
            if video.provider_id == header.provider_id {
                return Err(Status::invalid_argument(format!(
                    "Provider ID `{}` already exists",
                    header.provider_id
                )));
            }
        }
        let video = Video::from_exchanged(&program, header);
        let writer = match self.storage.create(&program, &video).await {
            Ok(w) => Ok(w),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;

        let mut buf = BufWriter::new(writer);
        let mut wrote_length: u64 = 0;
        while let Some(msg) = stream.next().await {
            let msg = msg?;
            let part = match msg.part {
                Some(part) => Ok(part),
                None => Err(Status::invalid_argument("Missing value: part")),
            }?;

            match part {
                VideoPart::Datagram(data) => {
                    if data.offset < wrote_length {
                        return Err(Status::invalid_argument("Invalid offset: already received"));
                    }
                    buf.write_all(&data.payload).await.map_err(map_io_error)?;
                    wrote_length += data.payload.len() as u64;
                }
                _ => return Err(Status::invalid_argument("Invalid part: need datagram")),
            }
        }

        let program_key = ProgramKey::from_stored_program(&program);
        let video = match self.store.create_video(&program_key, video) {
            Ok(video) => Ok(video),
            Err(e) => match e {
                VideoWriteError::ProgramNotFound(e) => {
                    Err(Status::not_found(format!("Program not found (id = {})", e)))
                }
                VideoWriteError::AlreadyExists(s) => {
                    Err(Status::invalid_argument(format!("Provider ID `{}` already exists", s)))
                }
                VideoWriteError::Poisoned(e) => Err(Status::aborted(format!("{}", e))),
            },
        }?;

        if let Err(e) = buf.into_inner().finish() {
            eprintln!("Error in StorageWriter.finish: {}", e);
        }
        println!("CreateVideo finish");

        let mut cache = CachedProgramFinder::new(self.store.clone());
        Ok(Response::new(CreateVideoResponse {
            video: Some(video.exchangeable(&mut cache)),
        }))
    }

    type GetVideoStream = Pin<Box<dyn Stream<Item = Result<GetVideoResponse, Status>> + Send + Sync + 'static>>;

    async fn get_video(&self, request: Request<GetVideoRequest>) -> Result<Response<Self::GetVideoStream>, Status> {
        let msg = request.into_inner();
        if msg.video_id.is_empty() {
            return Err(Status::invalid_argument("Invalid value: video_id"));
        }

        let handle_find_status_error = |e| match e {
            FindStatusError::Unavailable(e) => Err(Status::unavailable(format!("{}", e))),
            FindStatusError::NotFound => Err(Status::not_found("Video not found")),
            FindStatusError::IoError(e) => Err(Status::aborted(format!("{}", e))),
            FindStatusError::ReadError(e) => Err(Status::aborted(format!("{}", e))),
        };

        let video = match self.storage.find_header(&msg.video_id).await {
            Ok(v) => v,
            Err(e) => return handle_find_status_error(e),
        };

        let stream = match self.storage.find_bin(&msg.video_id).await {
            Ok(s) => s,
            Err(e) => return handle_find_status_error(e),
        };
        let mut cache = CachedProgramFinder::new(self.store.clone());

        let (mut tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let header_res = GetVideoResponse {
                part: Some(GetVideoResponsePart::Header(video.exchangeable(&mut cache))),
            };
            if let Err(e) = tx.send(Ok(header_res)).await {
                eprintln!("[[Error in task!]] {}", e);
                return;
            };

            let mut reader = tokio::io::BufReader::new(stream);
            let mut sent: usize = 0;
            loop {
                let mut buffer = vec![0; 1024 * 1024];
                match reader.read(&mut buffer).await {
                    Ok(size) => match size {
                        0 => break,
                        n => {
                            buffer.resize(n, 0);
                            let datagram = GetVideoResponseDatagram {
                                offset: sent as u64,
                                payload: buffer,
                            };
                            let datagram_req = GetVideoResponse {
                                part: Some(GetVideoResponsePart::Datagram(datagram)),
                            };
                            if let Err(e) = tx.send(Ok(datagram_req)).await {
                                eprintln!("[[Error in task!]] {}", e);
                                return;
                            };

                            sent += n;
                        }
                    },
                    Err(e) => {
                        eprintln!("[[Error in task!]] {}", e);
                        if let Err(e) = tx
                            .send(Err(Status::aborted(format!("Error while reading stream: {}", e))))
                            .await
                        {
                            eprintln!("[[Error in task!]] {}", e);
                        }
                        return;
                    }
                };
            }
        });

        Ok(Response::new(Box::pin(rx)))
    }
}

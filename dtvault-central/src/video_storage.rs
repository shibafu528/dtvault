mod storage;

pub use self::storage::*;
use crate::program::{validate_program_id, ProgramKey, ProgramStore, Video, VideoWriteError};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Part as VideoPart;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageService as VideoStorageServiceTrait;
use dtvault_types::shibafu528::dtvault::storage::{CreateVideoRequest, CreateVideoResponse};
use std::sync::Arc;
use tokio::io::BufWriter;
use tokio::prelude::*;
use tokio::stream::StreamExt;
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

        if program.exists_video(&header.provider_id) {
            return Err(Status::invalid_argument(format!(
                "Provider ID `{}` already exists",
                header.provider_id
            )));
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
                VideoWriteError::PoisonError(e) => Err(Status::aborted(format!("{}", e))),
            },
        }?;

        println!("CreateVideo finish");

        Ok(Response::new(CreateVideoResponse {
            video: Some(video.exchangeable()),
        }))
    }
}

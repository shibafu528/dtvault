mod storage;

pub use self::storage::*;
use crate::program::{validate_program_id, ProgramKey, ProgramStore};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::{Header as VideoHeader, Part as VideoPart};
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageService as VideoStorageServiceTrait;
use dtvault_types::shibafu528::dtvault::storage::{CreateVideoRequest, CreateVideoResponse};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::BufWriter;
use tokio::prelude::*;
use tokio::stream::StreamExt;
use tonic::{Request, Response, Status};

fn map_io_error(e: tokio::io::Error) -> Status {
    Status::internal(format!("IO error: {}", e))
}

pub struct VideoStorageService {
    store: Arc<ProgramStore>,
}

impl VideoStorageService {
    pub fn new(store: Arc<ProgramStore>) -> Self {
        VideoStorageService { store }
    }
}

#[tonic::async_trait]
impl VideoStorageServiceTrait for VideoStorageService {
    async fn create_video(
        &self,
        request: Request<tonic::Streaming<CreateVideoRequest>>,
    ) -> Result<Response<CreateVideoResponse>, Status> {
        let mut stream = request.into_inner();

        let mut header = None::<VideoHeader>;
        let tmp = tempfile::NamedTempFile::new().map_err(map_io_error)?;
        let tmpfile = File::create(tmp.path()).await.map_err(map_io_error)?;
        let mut tmp_writer = BufWriter::new(tmpfile);
        let mut wrote_length: u64 = 0;
        while let Some(msg) = stream.next().await {
            let msg = msg?;
            let part = match msg.part {
                Some(part) => Ok(part),
                None => Err(Status::invalid_argument("Missing value: part")),
            }?;

            match header {
                Some(_) => match part {
                    VideoPart::Datagram(data) => {
                        if data.offset < wrote_length {
                            return Err(Status::invalid_argument("Invalid offset: already received"));
                        }
                        tmp_writer.write_all(&data.payload).await.map_err(map_io_error)?;
                        wrote_length += data.payload.len() as u64;
                    }
                    _ => return Err(Status::invalid_argument("Invalid part: need datagram")),
                },
                None => match part {
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
                                Some(_sp) => Ok(()),
                                None => Err(Status::not_found(format!("Program not found (id = {})", program_key))),
                            },
                            Err(e) => Err(Status::aborted(format!("{}", e))),
                        }?;

                        header = Some(h);
                    }
                    _ => return Err(Status::invalid_argument("Invalid part: need header")),
                },
            }
        }
        println!("CreateVideo finish");

        Ok(Response::new(CreateVideoResponse { video: None }))
    }
}

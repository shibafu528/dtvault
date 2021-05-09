mod filesystem;
mod storage;
mod tempfile;
mod validator;

pub use self::filesystem::*;
pub use self::storage::*;
pub use self::tempfile::*;
use crate::config::Config;
use crate::event::{Event, EventEmitter, VideoCreated};
use crate::program::{validate_program_id, Program, ProgramKey, ProgramStore, Video, VideoWriteError};
use crate::video_storage::validator::validate_file_name;
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Part as VideoPart;
use dtvault_types::shibafu528::dtvault::storage::get_video_response::Datagram as GetVideoResponseDatagram;
use dtvault_types::shibafu528::dtvault::storage::get_video_response::Part as GetVideoResponsePart;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageService as VideoStorageServiceTrait;
use dtvault_types::shibafu528::dtvault::storage::{
    CreateVideoRequest, CreateVideoResponse, GetVideoRequest, GetVideoResponse,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use uuid::Uuid;

fn map_io_error(e: tokio::io::Error) -> Status {
    Status::internal(format!("IO error: {}", e))
}

pub struct VideoStorageService {
    config: Arc<Config>,
    store: Arc<ProgramStore>,
    storages: Vec<Arc<IStorage>>,
    event_emitter: EventEmitter,
}

impl VideoStorageService {
    pub fn new(
        config: Arc<Config>,
        store: Arc<ProgramStore>,
        storages: Vec<Arc<IStorage>>,
        event_emitter: EventEmitter,
    ) -> Self {
        VideoStorageService {
            config,
            store,
            storages,
            event_emitter,
        }
    }

    fn primary_storage(&self) -> Arc<IStorage> {
        self.storages.first().unwrap().clone()
    }

    async fn find_storage_by_rule(&self, program: &Program, video: &Video) -> Arc<IStorage> {
        for rule in &self.config.storage_rules {
            if !rule.matches(program, video) {
                continue;
            }

            if !rule.storage_label.is_empty() {
                // find by label
                for storage in &self.storages {
                    if storage.label() == rule.storage_label {
                        return storage.clone();
                    }
                }
            } else {
                // find by uuid
                for storage in &self.storages {
                    if storage.storage_id().await.map_or(false, |id| id == rule.storage_id) {
                        return storage.clone();
                    }
                }
            }
        }

        // fallback
        self.primary_storage()
    }

    async fn find_storage_by_id(&self, storage_id: &Uuid) -> Option<Arc<IStorage>> {
        for storage in &self.storages {
            if let Ok(id) = storage.storage_id().await {
                if id == *storage_id {
                    return Some(storage.clone());
                }
            }
        }
        None
    }
}

#[tonic::async_trait]
impl VideoStorageServiceTrait for VideoStorageService {
    async fn create_video(
        &self,
        request: Request<tonic::Streaming<CreateVideoRequest>>,
    ) -> Result<Response<CreateVideoResponse>, Status> {
        let event_emitter = self.event_emitter.clone();
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

        if let Err(e) = validate_file_name(&header.file_name) {
            return Err(Status::invalid_argument(e));
        }

        // Check video existence
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
        let mut video = Video::from_exchanged(&program, header);

        // Find storage
        let storage = self.find_storage_by_rule(&program, &video).await;
        match storage.storage_id().await {
            Ok(id) => video.storage_id = id,
            Err(e) => return Err(Status::aborted(format!("{}", e))),
        }

        // Set prefix
        for rule in &self.config.prefix_rules {
            if !rule.matches(&program, &video) {
                continue;
            }

            video.storage_prefix = rule.prefix.clone();
            break;
        }

        let mut writer = match storage.create(&program, &video).await {
            Ok(w) => Ok(w),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;

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
                    writer.write_all(&data.payload).await.map_err(map_io_error)?;
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

        if let Err(e) = writer.as_mut().finish().await {
            eprintln!("Error in StorageWriter.finish: {}", e);
        }
        println!("CreateVideo finish");

        if let Err(e) = event_emitter
            .send(Event::VideoCreated(VideoCreated {
                program_key,
                video_id: video.id.clone(),
            }))
            .await
        {
            eprintln!("Error in send event: {}", e);
        }

        Ok(Response::new(CreateVideoResponse {
            video: Some(video.exchangeable()),
        }))
    }

    type GetVideoStream = ReceiverStream<Result<GetVideoResponse, Status>>;

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

        let video_id =
            Uuid::parse_str(&msg.video_id).map_err(|_| Status::invalid_argument("Invalid value: video_id"))?;
        let video = match self.store.find_video(&video_id) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err(Status::not_found("Video not found")),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;

        let storage = match self.find_storage_by_id(&video.storage_id).await {
            Some(s) => s,
            None => {
                return Err(Status::unavailable(
                    "Target storage is temporarily unavailable or not found",
                ))
            }
        };
        let mut reader = match storage.find_bin(&video).await {
            Ok(s) => s,
            Err(e) => return handle_find_status_error(e),
        };

        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let header_res = GetVideoResponse {
                part: Some(GetVideoResponsePart::Header(video.exchangeable())),
            };
            if let Err(e) = tx.send(Ok(header_res)).await {
                eprintln!("[[Error in task!]] {}", e);
                return;
            };

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

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

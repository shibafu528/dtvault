use tonic::{transport::Server, Request, Response, Status};

use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as ResponseStatus;
use dtvault_types::shibafu528::dtvault::central::program_service_server::{
    ProgramService as ProgramServiceTrait, ProgramServiceServer,
};
use dtvault_types::shibafu528::dtvault::central::{
    CreateProgramRequest, CreateProgramResponse, GetProgramMetadataRequest, GetProgramMetadataResponse,
    GetProgramRequest, GetProgramResponse, UpdateProgramMetadataRequest, UpdateProgramMetadataResponse,
};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::{Header as VideoHeader, Part as VideoPart};
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::{
    VideoStorageService as VideoStorageServiceTrait, VideoStorageServiceServer,
};
use dtvault_types::shibafu528::dtvault::storage::{CreateVideoRequest, CreateVideoResponse};
use dtvault_types::shibafu528::dtvault::{Channel, Program, ProgramIdentity, Service};
use once_cell::sync::Lazy;
use prost_types::Timestamp;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::stream::StreamExt;

fn validate_channel(value: &Channel) -> Result<&Channel, String> {
    if value.channel.is_empty() {
        return Err("Invalid value: channel".to_string());
    }
    if value.name.is_empty() {
        return Err("Invalid value: name".to_string());
    }

    Ok(value)
}

fn validate_service(value: &Service) -> Result<&Service, String> {
    if value.service_id == 0 {
        return Err("Invalid value: service_id".to_string());
    }
    if let Some(channel) = &value.channel {
        if let Err(msg) = validate_channel(channel) {
            return Err(format!("Violation in channel => {}", msg));
        }
    }

    Ok(value)
}

fn validate_program_id(value: &ProgramIdentity) -> Result<&ProgramIdentity, String> {
    if value.service_id == 0 {
        return Err("Invalid value: service_id".to_string());
    }
    if value.event_id == 0 {
        return Err("Invalid value: event_id".to_string());
    }
    if value.start_at == None {
        return Err("Missing value: start_at".to_string());
    }

    Ok(value)
}

struct StoredProgram {
    program: Program,
    metadata: HashMap<String, String>,
}

impl StoredProgram {
    pub fn new(program: Program) -> Self {
        StoredProgram {
            program,
            metadata: HashMap::new(),
        }
    }
}

// TODO: ちゃんと保存する
static PROGRAM_STORE: Lazy<Mutex<HashMap<String, StoredProgram>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn program_store_key(id: &ProgramIdentity) -> String {
    let start_at = id.start_at.as_ref().unwrap_or(&Timestamp { seconds: 0, nanos: 0 });
    format!(
        "{}_{}_{}_{}_{}",
        id.network_id, id.service_id, id.event_id, start_at.seconds, start_at.nanos
    )
}

#[derive(Debug, Default)]
struct ProgramService;

#[tonic::async_trait]
impl ProgramServiceTrait for ProgramService {
    async fn get_program(&self, request: Request<GetProgramRequest>) -> Result<Response<GetProgramResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::invalid_argument(format!("Violation in program_id => {}", msg))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;

        let id = program_store_key(&program_id);

        match PROGRAM_STORE.lock() {
            Ok(store) => match store.get(&id) {
                Some(sp) => Ok(Response::new(GetProgramResponse {
                    program: Some(sp.program.clone()),
                })),
                None => Err(Status::not_found(format!("Program not found (id = {})", id))),
            },
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }
    }

    async fn create_program(
        &self,
        request: Request<CreateProgramRequest>,
    ) -> Result<Response<CreateProgramResponse>, Status> {
        let msg = request.into_inner();
        let program = match msg.program {
            Some(p) => Ok(p),
            None => Err(Status::invalid_argument("Missing value: program".to_string())),
        }?;

        if program.service_id == 0 {
            return Err(Status::invalid_argument("Invalid value: service_id".to_string()));
        }
        if program.event_id == 0 {
            return Err(Status::invalid_argument("Invalid value: event_id".to_string()));
        }
        if program.start_at == None {
            return Err(Status::invalid_argument("Missing value: start_at".to_string()));
        }
        if program.name.is_empty() {
            return Err(Status::invalid_argument("Invalid value: name".to_string()));
        }
        if let Some(service) = &program.service {
            if let Err(msg) = validate_service(service) {
                return Err(Status::invalid_argument(format!("Violation in service => {}", msg)));
            }
        }

        println!("Accept => {:#?}", program);
        let (status, ret_program) = match PROGRAM_STORE.lock() {
            Ok(mut store) => {
                let start_at = program.start_at.as_ref().unwrap();
                let id = format!(
                    "{}_{}_{}_{}_{}",
                    program.network_id, program.service_id, program.event_id, start_at.seconds, start_at.nanos
                );
                if let Some(prog) = store.get(&id) {
                    Ok((ResponseStatus::AlreadyExists, prog.program.clone()))
                } else {
                    store.insert(id, StoredProgram::new(program.clone()));
                    Ok((ResponseStatus::Created, program))
                }
            }
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;

        let res = CreateProgramResponse {
            status: status.into(),
            program: Some(ret_program),
        };
        Ok(Response::new(res))
    }

    async fn get_program_metadata(
        &self,
        request: Request<GetProgramMetadataRequest>,
    ) -> Result<Response<GetProgramMetadataResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::invalid_argument(format!("Violation in program_id => {}", msg))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;
        if msg.key.is_empty() {
            return Err(Status::invalid_argument("Invalid value: key"));
        }
        if msg.key.len() > 255 {
            return Err(Status::invalid_argument("String too long: key"));
        }

        let id = program_store_key(&program_id);

        match PROGRAM_STORE.lock() {
            Ok(store) => match store.get(&id) {
                Some(sp) => {
                    println!("PID = {:?}, {:?}", program_id, &msg.key);

                    if let Some(value) = sp.metadata.get(&msg.key) {
                        Ok(Response::new(GetProgramMetadataResponse {
                            program_id: Some(program_id),
                            key: msg.key,
                            value: value.to_string(),
                        }))
                    } else {
                        Ok(Response::new(GetProgramMetadataResponse {
                            program_id: Some(program_id),
                            key: msg.key,
                            value: "".to_string(),
                        }))
                    }
                }
                None => Err(Status::not_found(format!("Program not found (id = {})", id))),
            },
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }
    }

    async fn update_program_metadata(
        &self,
        request: Request<UpdateProgramMetadataRequest>,
    ) -> Result<Response<UpdateProgramMetadataResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::failed_precondition(format!(
                    "Violation in program_id => {}",
                    msg
                ))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;
        if msg.key.is_empty() {
            return Err(Status::invalid_argument("Invalid value: key"));
        }
        if msg.key.len() > 255 {
            return Err(Status::invalid_argument("String too long: key"));
        }
        if msg.value.len() > 1 * 1024 * 1024 {
            return Err(Status::invalid_argument("String too long: value"));
        }

        let id = program_store_key(&program_id);

        match PROGRAM_STORE.lock() {
            Ok(mut store) => match store.get_mut(&id) {
                Some(sp) => {
                    println!("PID = {:?}, {:?} => {:?}", program_id, &msg.key, &msg.value);
                    sp.metadata.insert(msg.key, msg.value);
                    Ok(Response::new(UpdateProgramMetadataResponse {}))
                }
                None => Err(Status::not_found(format!("Program not found (id = {})", id))),
            },
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }
    }
}

#[derive(Debug, Default)]
struct VideoStorageService;

#[tonic::async_trait]
impl VideoStorageServiceTrait for VideoStorageService {
    async fn create_video(
        &self,
        request: Request<tonic::Streaming<CreateVideoRequest>>,
    ) -> Result<Response<CreateVideoResponse>, Status> {
        let mut stream = request.into_inner();

        let mut header = None::<VideoHeader>;
        while let Some(msg) = stream.next().await {
            let msg = msg?;
            let part = match msg.part {
                Some(part) => Ok(part),
                None => Err(Status::invalid_argument("Missing value: part")),
            }?;

            match header {
                Some(_) => match part {
                    VideoPart::Datagram(data) => {
                        // println!("offset = {}", data.offset);
                    }
                    _ => return Err(Status::invalid_argument("Invalid part: need datagram")),
                },
                None => match part {
                    VideoPart::Header(h) => {
                        println!("CreateVideo {:#?}", h);
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let program_service = ProgramService::default();
    let video_storage_service = VideoStorageService::default();

    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(ProgramServiceServer::with_interceptor(program_service, request_logger))
        .add_service(VideoStorageServiceServer::with_interceptor(
            video_storage_service,
            request_logger,
        ))
        .serve(addr)
        .await?;

    Ok(())
}

fn request_logger(req: Request<()>) -> Result<Request<()>, Status> {
    println!("Request => {:?}", req);
    Ok(req)
}

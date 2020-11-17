use tonic::{transport::Server, Request, Response, Status};

use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as ResponseStatus;
use dtvault_types::shibafu528::dtvault::central::program_service_server::{
    ProgramService as ProgramServiceTrait, ProgramServiceServer,
};
use dtvault_types::shibafu528::dtvault::central::{
    CreateProgramRequest, CreateProgramResponse, UpdateProgramMetadataRequest, UpdateProgramMetadataResponse,
};
use dtvault_types::shibafu528::dtvault::{Channel, Program, ProgramIdentity, Service};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

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

// TODO: ちゃんと保存する
static PROGRAM_STORE: Lazy<Mutex<HashMap<String, Program>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Default)]
struct ProgramService;

#[tonic::async_trait]
impl ProgramServiceTrait for ProgramService {
    async fn create_program(
        &self,
        request: Request<CreateProgramRequest>,
    ) -> Result<Response<CreateProgramResponse>, Status> {
        let msg = request.into_inner();
        let program = match msg.program {
            Some(p) => Ok(p),
            None => Err(Status::failed_precondition("Missing value: program".to_string())),
        }?;

        if program.service_id == 0 {
            return Err(Status::failed_precondition("Invalid value: service_id".to_string()));
        }
        if program.event_id == 0 {
            return Err(Status::failed_precondition("Invalid value: event_id".to_string()));
        }
        if program.start_at == None {
            return Err(Status::failed_precondition("Missing value: start_at".to_string()));
        }
        if program.name.is_empty() {
            return Err(Status::failed_precondition("Invalid value: name".to_string()));
        }
        if let Some(service) = &program.service {
            if let Err(msg) = validate_service(service) {
                return Err(Status::failed_precondition(format!("Violation in service => {}", msg)));
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
                    Ok((ResponseStatus::AlreadyExists, prog.clone()))
                } else {
                    store.insert(id, program.clone());
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
            None => Err(Status::failed_precondition("Missing value: program_id")),
        }?;
        if msg.key.is_empty() {
            return Err(Status::failed_precondition("Invalid value: key"));
        }
        if msg.key.len() > 255 {
            return Err(Status::failed_precondition("String too long: key"));
        }
        if msg.value.len() > 4 * 1024 * 1024 {
            return Err(Status::failed_precondition("String too long: value"));
        }

        println!("PID = {:?}, {:?} => {:?}", program_id, msg.key, msg.value);

        Ok(Response::new(UpdateProgramMetadataResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let service = ProgramService::default();

    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(ProgramServiceServer::with_interceptor(service, request_logger))
        .serve(addr)
        .await?;

    Ok(())
}

fn request_logger(req: Request<()>) -> Result<Request<()>, Status> {
    println!("Request => {:?}", req);
    Ok(req)
}

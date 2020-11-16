use tonic::{transport::Server, Request, Response, Status};

use dtvault_types::shibafu528::dtvault::central::program_service_server::{
    ProgramService as ProgramServiceTrait, ProgramServiceServer,
};
use dtvault_types::shibafu528::dtvault::central::{
    CreateProgramRequest, CreateProgramResponse, UpdateProgramMetadataRequest, UpdateProgramMetadataResponse,
};
use dtvault_types::shibafu528::dtvault::ProgramIdentity;

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

#[derive(Debug, Default)]
struct ProgramService;

#[tonic::async_trait]
impl ProgramServiceTrait for ProgramService {
    async fn create_program(
        &self,
        request: Request<CreateProgramRequest>,
    ) -> Result<Response<CreateProgramResponse>, Status> {
        Err(Status::unimplemented("not implemented yet"))
    }

    async fn update_program_metadata(
        &self,
        request: Request<UpdateProgramMetadataRequest>,
    ) -> Result<Response<UpdateProgramMetadataResponse>, Status> {
        println!("{:?}", request);

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
        .add_service(ProgramServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

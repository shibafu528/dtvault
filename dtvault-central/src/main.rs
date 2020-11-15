use tonic::{transport::Server, Request, Response, Status};

use dtvault_types::shibafu528::dtvault::central::program_service_server::{
    ProgramService as ProgramServiceTrait, ProgramServiceServer,
};
use dtvault_types::shibafu528::dtvault::central::{
    CreateProgramRequest, CreateProgramResponse, UpdateProgramMetadataRequest,
    UpdateProgramMetadataResponse,
};

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
        Err(Status::unimplemented("not implemented yet"))
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

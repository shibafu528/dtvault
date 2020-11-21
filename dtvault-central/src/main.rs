mod program;
mod video_storage;

use crate::program::ProgramService;
use crate::video_storage::VideoStorageService;
use dtvault_types::shibafu528::dtvault::central::program_service_server::ProgramServiceServer;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageServiceServer;
use tonic::{transport::Server, Request, Status};

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

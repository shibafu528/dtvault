mod program;
mod video_storage;

use crate::program::{ProgramService, ProgramStore};
use crate::video_storage::{FileSystem, VideoStorageService};
use dtvault_types::shibafu528::dtvault::central::program_service_server::ProgramServiceServer;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageServiceServer;
use envy::Error as EnvyError;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tonic::{transport::Server, Request, Status};

// 持ち方は後で変えるかも
#[derive(Deserialize, Debug)]
pub struct Config {
    data_dir: String,
    storage_dir: String,
}

impl Config {
    fn check_and_create_data_dir(&self) -> Result<(), std::io::Error> {
        let data_dir = Path::new(&self.data_dir);
        if !data_dir.is_dir() {
            std::fs::create_dir_all(data_dir)?;
        }
        let storage_dir = Path::new(&self.storage_dir);
        if !storage_dir.is_dir() {
            std::fs::create_dir_all(storage_dir)?;
        }
        Ok(())
    }

    pub fn programs_file_path(&self) -> PathBuf {
        PathBuf::from(self.data_dir.to_string()).join("programs.pb")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = envy::prefixed("DTVAULT_").from_env().unwrap_or_else(|err| match err {
        EnvyError::MissingValue(key) => panic!("Missing environment variable `DTVAULT_{}`", key.to_uppercase()),
        EnvyError::Custom(s) => panic!("{}", s),
    });
    config.check_and_create_data_dir()?;
    let config = Arc::new(config);

    let program_store = Arc::new(ProgramStore::new(config.clone())?);
    let program_service = ProgramService::new(program_store.clone());
    let storage = Arc::new(FileSystem::new(config.storage_dir.to_string()));
    let video_storage_service = VideoStorageService::new(program_store.clone(), storage.clone());

    let addr = "[::0]:50051".parse().unwrap();
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

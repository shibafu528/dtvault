mod config;
mod event;
mod program;
mod serde;
mod video_storage;

use crate::config::Config;
use crate::event::EventContext;
use crate::program::{ProgramService, ProgramStore};
use crate::video_storage::{FileSystem, IStorage, VideoStorageService};
use ::serde::Deserialize;
use dtvault_types::shibafu528::dtvault::central::program_service_server::ProgramServiceServer;
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_server::VideoStorageServiceServer;
use envy::Error as EnvyError;
use std::process::exit;
use std::sync::Arc;
use tonic::{transport::Server, Request, Status};

const ENV_PREFIX: &str = "DTVAULT_CENTRAL_";

#[derive(Deserialize, Debug)]
struct Env {
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env: Env = envy::prefixed(ENV_PREFIX).from_env().unwrap_or_else(|err| match err {
        EnvyError::MissingValue(key) => {
            eprintln!("Missing environment variable `{}{}`", ENV_PREFIX, key.to_uppercase());
            exit(1)
        }
        EnvyError::Custom(s) => panic!("{}", s),
    });
    let config_str = std::fs::read_to_string(env.config).unwrap();
    let config: Config = toml::from_str(&config_str).unwrap_or_else(|err| {
        eprintln!("Error in reading config file: {}", err);
        exit(1)
    });
    if let Err(err) = config.validate() {
        eprintln!("Error in config file: {}", err);
        exit(1)
    }
    let config = Arc::new(config);

    let (event_emitter, event_receiver) = event::make_event_channel();

    let program_store = Arc::new(ProgramStore::new(config.clone())?);
    let program_service = ProgramService::new(program_store.clone());

    let mut storages = Vec::<Arc<IStorage>>::new();
    for conf in &config.storages {
        match conf {
            config::Storage::FileSystem(fs) => {
                storages.push(Arc::new(FileSystem::new(fs.label.to_string(), fs.root_dir.to_string())))
            }
            config::Storage::Tempfile(tf) => {
                storages.push(Arc::new(video_storage::Tempfile::new(tf.label.to_string())))
            }
        }
    }
    let video_storage_service = VideoStorageService::new(
        config.clone(),
        program_store.clone(),
        storages.clone(),
        event_emitter.clone(),
    );

    let _event_join_handle = event::spawn_event_consumer(
        EventContext {
            config: config.clone(),
            program_store: program_store.clone(),
            storages,
        },
        event_receiver,
    );

    let addr = config.server.listen.parse().unwrap();
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

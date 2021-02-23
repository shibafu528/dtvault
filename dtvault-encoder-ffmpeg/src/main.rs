mod config;
mod encoder;

use crate::config::Config;
use crate::encoder::EncoderService;
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_server::EncoderServiceServer;
use envy::Error as EnvyError;
use serde::Deserialize;
use std::process::exit;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, Status};

const ENV_PREFIX: &str = "DTVAULT_ENCODER_FFMPEG_";

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
    println!("{} preset(s) loaded", config.presets.len());
    let config = Arc::new(config);

    let encoder_service = EncoderService::new(config.clone());

    let addr = config.listen.parse().unwrap();
    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(EncoderServiceServer::with_interceptor(encoder_service, request_logger))
        .serve(addr)
        .await?;

    Ok(())
}

fn request_logger(req: Request<()>) -> Result<Request<()>, Status> {
    println!("Request => {:?}", req);
    Ok(req)
}

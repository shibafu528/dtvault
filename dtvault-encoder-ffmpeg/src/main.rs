mod encoder;

use crate::encoder::EncoderService;
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_server::EncoderServiceServer;
use envy::Error as EnvyError;
use serde::Deserialize;
use tonic::transport::Server;
use tonic::{Request, Status};

const ENV_PREFIX: &str = "DTVAULT_ENCODER_FFMPEG_";

#[derive(Deserialize, Debug)]
struct Config {
    port: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = envy::prefixed(ENV_PREFIX).from_env().unwrap_or_else(|err| match err {
        EnvyError::MissingValue(key) => {
            panic!("Missing environment variable `{}{}`", ENV_PREFIX, key.to_uppercase())
        }
        EnvyError::Custom(s) => panic!("{}", s),
    });

    let encoder_service = EncoderService::new();

    let addr = format!("[::0]:{}", config.port).parse().unwrap();
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

mod chinachu;
mod record_with_raw;

use std::fs::File;
use std::io::{BufReader, Read};
use std::time::Instant;

use clap::{App, Arg};
use envy::Error;
use serde::Deserialize;
use serde_json::value::RawValue;
use tokio::sync::mpsc;
use tonic::transport::Uri;

use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as CreateProgramStatus;
use dtvault_types::shibafu528::dtvault::central::program_service_client::ProgramServiceClient;
use dtvault_types::shibafu528::dtvault::storage::create_video_request::{Datagram as VideoDatagram, Part as VideoPart};
use dtvault_types::shibafu528::dtvault::storage::video_storage_service_client::VideoStorageServiceClient;
use dtvault_types::shibafu528::dtvault::storage::CreateVideoRequest;

use crate::record_with_raw::RecordWithRaw;

#[derive(Deserialize, Debug)]
struct Config {
    central_addr: String,
    #[serde(default)]
    debug: bool,
}

struct Connection {
    program_client: ProgramServiceClient<tonic::transport::Channel>,
    video_storage_client: VideoStorageServiceClient<tonic::transport::Channel>,
}

#[derive(thiserror::Error, Debug)]
enum ConnectionError {
    #[error("Invalid URI: {0}")]
    InvalidUri(String),
    #[error(transparent)]
    TonicError(#[from] tonic::transport::Error),
}

impl Connection {
    async fn new(config: &Config) -> Result<Self, ConnectionError> {
        let central_url = config
            .central_addr
            .parse::<Uri>()
            .map_err(|_| ConnectionError::InvalidUri(config.central_addr.to_string()))?;

        let program_client = ProgramServiceClient::connect(central_url.clone()).await?;
        let video_storage_client = VideoStorageServiceClient::connect(central_url.clone()).await?;

        Ok(Connection {
            program_client,
            video_storage_client,
        })
    }
}

async fn send_to_central(
    config: &Config,
    connection: &mut Connection,
    record: &RecordWithRaw,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Program: {}", record.record.full_title);

    // Step 1. Send parsed program data
    let create_req = record.create_program_request().unwrap();
    if config.debug {
        println!("{:#?}", create_req);
    }
    print!("--> Send program... ");
    let create_res = connection.program_client.create_program(create_req).await?;
    if create_res.get_ref().status == CreateProgramStatus::AlreadyExists as i32 {
        println!("Already exists.");
    } else {
        println!("Done.");
    }

    // Step 2. Send raw program data
    let meta_req = record.update_program_metadata_request().unwrap();
    if config.debug {
        println!("{:#?}", meta_req);
    }
    print!("--> Send raw program... ");
    let _meta_res = connection.program_client.update_program_metadata(meta_req).await?;
    println!("Done.");

    // Step 3. Send M2TS video
    println!("--> Send video...: {}", record.record.recorded);
    let stream_begin = Instant::now();
    let stream = {
        let mut reader = BufReader::new(File::open(record.record.recorded.to_string())?);
        let header = record.video_header()?;
        let (mut tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let header_req = CreateVideoRequest {
                part: Some(VideoPart::Header(header)),
            };
            if let Err(e) = tx.send(header_req).await {
                eprintln!("[[Error in task!]] {}", e);
                return;
            }

            let mut sent: usize = 0;
            loop {
                let mut buffer = vec![0; 1024 * 1024];
                match reader.read(&mut buffer) {
                    Ok(size) => match size {
                        0 => break,
                        n => {
                            buffer.resize(n, 0);
                            let datagram = VideoDatagram {
                                offset: sent as u64,
                                payload: buffer,
                            };
                            let datagram_req = CreateVideoRequest {
                                part: Some(VideoPart::Datagram(datagram)),
                            };
                            if let Err(e) = tx.send(datagram_req).await {
                                eprintln!("[[Error in task!]] {}", e);
                                return;
                            };

                            sent += n;
                        }
                    },
                    Err(e) => {
                        eprintln!("[[Error in task!]] {}", e);
                        return;
                    }
                };
            }
        });
        rx
    };
    let _video_res = connection.video_storage_client.create_video(stream).await?;
    println!("    Done. ({} secs)", stream_begin.elapsed().as_secs_f32());

    Ok(())
}

async fn exec_send(config: &Config, json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let record = RecordWithRaw::from_str(json)?;
    let mut connection = Connection::new(config).await?;

    send_to_central(config, &mut connection, &record).await?;

    Ok(())
}

async fn exec_import(config: &Config, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let reader = BufReader::new(File::open(filename).unwrap_or_else(|_| panic!("failed to open: {}", filename)));
    let recorded: Vec<Box<RawValue>> = serde_json::from_reader(reader)?;

    let mut parsed: Vec<RecordWithRaw> = Vec::with_capacity(recorded.len());
    for raw_record in recorded {
        parsed.push(RecordWithRaw::from_str(raw_record.get())?);
    }
    parsed.sort_by(|rec1, rec2| rec1.record.id.cmp(&rec2.record.id));
    parsed.sort_by_key(|rec| rec.record.start);

    let mut connection = Connection::new(config).await?;
    for rec in parsed {
        match send_to_central(config, &mut connection, &rec).await {
            Ok(_) => println!("[  OK  ] {}", rec.record.full_title),
            Err(e) => println!("[FAILED] {}\n{}", rec.record.full_title, e),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config: Config = envy::prefixed("DTVAULT_").from_env().unwrap_or_else(|err| match err {
        Error::MissingValue(key) => panic!("Missing environment variable `{}`", key.to_uppercase()),
        Error::Custom(s) => panic!("{}", s),
    });

    let m = App::new("dtvault-collector-chinachu")
        .about("Send recorded MPEG2-TS file and program description to dtvault-central")
        .arg(
            Arg::with_name("import")
                .short("i")
                .long("import-from")
                .help("Import all recorded programs from recorded.json")
                .value_name("FILE")
                .takes_value(true),
        )
        .arg(Arg::with_name("debug").short("d").long("debug"))
        .arg(
            Arg::with_name("VIDEO")
                .help("Recorded MPEG2-TS file path (but not used)")
                .required(true)
                .conflicts_with("import"),
        )
        .arg(
            Arg::with_name("JSON")
                .help("Program JSON")
                .required(true)
                .conflicts_with("import"),
        )
        .get_matches();
    config.debug |= m.is_present("debug");

    if let Some(file) = m.value_of("import") {
        exec_import(&config, file).await
    } else {
        exec_send(&config, m.value_of("JSON").unwrap()).await
    }
}

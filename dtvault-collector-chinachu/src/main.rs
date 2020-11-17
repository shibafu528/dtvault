use crate::record_with_raw::RecordWithRaw;
use crate::recorded_program::RecordedProgram;
use clap::{App, AppSettings, Arg, ArgGroup, SubCommand};
use dtvault_types::shibafu528::dtvault::central::program_service_client::ProgramServiceClient;
use envy::Error;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fs::File;
use std::io::BufReader;
use tonic::transport::Uri;

mod program_id;
mod record_with_raw;
mod recorded_program;

#[derive(Deserialize, Debug)]
struct Config {
    central_addr: String,
    #[serde(default)]
    debug: bool,
}

async fn send_to_central(config: &Config, record: &RecordWithRaw) -> Result<(), Box<dyn std::error::Error>> {
    let mut program_client = ProgramServiceClient::connect(config.central_addr.parse::<Uri>()?).await?;

    // Step 1. Send parsed program data
    let create_req = record.create_program_request().unwrap();
    if config.debug {
        println!("{:#?}", create_req);
    }
    let _create_res = program_client.create_program(create_req).await?;

    // Step 2. Send raw program data
    let meta_req = record.update_program_metadata_request().unwrap();
    if config.debug {
        println!("{:#?}", meta_req);
    }
    let _meta_res = program_client.update_program_metadata(meta_req).await?;

    // Step 3. Send M2TS video
    // TODO

    Ok(())
}

async fn exec_send(config: &Config, json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let record = RecordWithRaw::from_str(json)?;

    send_to_central(config, &record).await?;

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

    for rec in parsed {
        match send_to_central(config, &rec).await {
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

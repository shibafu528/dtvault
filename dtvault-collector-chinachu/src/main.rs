use crate::record_with_raw::RecordWithRaw;
use crate::recorded_program::RecordedProgram;
use clap::{App, AppSettings, Arg, ArgGroup, SubCommand};
use envy::Error;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fs::File;
use std::io::BufReader;

mod program_id;
mod record_with_raw;
mod recorded_program;

#[derive(Deserialize, Debug)]
struct Config {
    central_addr: String,
}

fn send(config: &Config, json: &str) -> serde_json::Result<()> {
    let record = RecordWithRaw {
        record: serde_json::from_str(json)?,
        raw: json.to_string().into(),
    };

    let req = record.create_program_request().unwrap();
    println!("{:#?}", req);

    Ok(())
}

fn import(config: &Config, filename: &str) -> serde_json::Result<()> {
    let reader = BufReader::new(
        File::open(filename).unwrap_or_else(|_| panic!("failed to open: {}", filename)),
    );
    let recorded: Vec<Box<RawValue>> = serde_json::from_reader(reader)?;

    let mut parsed: Vec<RecordWithRaw> = recorded
        .into_iter()
        .filter_map(
            |raw| match serde_json::from_str::<RecordedProgram>(raw.get()) {
                Ok(record) => Some(RecordWithRaw {
                    record,
                    raw: raw.into(),
                }),
                _ => None,
            },
        )
        .collect();
    parsed.sort_by(|rec1, rec2| rec1.record.id.cmp(&rec2.record.id));
    parsed.sort_by_key(|rec| rec.record.start);

    for rec in parsed {
        rec.dbg();
    }

    Ok(())
}

fn main() {
    let config: Config = envy::prefixed("DTVAULT_")
        .from_env()
        .unwrap_or_else(|err| match err {
            Error::MissingValue(key) => {
                panic!("Missing environment variable `{}`", key.to_uppercase())
            }
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

    if let Some(file) = m.value_of("import") {
        import(&config, file).unwrap();
    } else {
        send(&config, m.value_of("JSON").unwrap()).unwrap();
    }
}

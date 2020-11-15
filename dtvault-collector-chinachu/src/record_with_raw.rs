use crate::program_id::ParseProgramIDError;
use crate::recorded_program::{MessageConversionError, RecordedProgram};
use dtvault_types::shibafu528::dtvault::central::CreateProgramRequest;
use dtvault_types::shibafu528::dtvault::Program;
use serde_json::value::RawValue;
use std::convert::TryInto;
use std::ops::Deref;

#[derive(Debug)]
pub enum RawJson {
    String(String),
    Serde(Box<RawValue>),
}

impl From<String> for RawJson {
    fn from(s: String) -> Self {
        RawJson::String(s)
    }
}

impl From<Box<RawValue>> for RawJson {
    fn from(v: Box<RawValue>) -> Self {
        RawJson::Serde(v)
    }
}

impl Deref for RawJson {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match &self {
            RawJson::String(s) => s,
            RawJson::Serde(s) => s.get(),
        }
    }
}

#[derive(Debug)]
pub struct RecordWithRaw {
    pub record: RecordedProgram,
    pub raw: RawJson,
}

impl RecordWithRaw {
    pub fn from_str(json: &str) -> Result<RecordWithRaw, serde_json::Error> {
        Ok(RecordWithRaw {
            record: serde_json::from_str(json)?,
            raw: json.to_string().into(),
        })
    }

    pub fn dbg(&self) {
        println!("{:?}", self.record);
        println!(
            "{} -> {}",
            self.record.id,
            self.record
                .program_id()
                .map(|id| format!("{:?}", id))
                .unwrap() //.unwrap_or_else(|e| format!("Parse error! {:?}", e))
        );
        println!(
            "raw {} bytes: {} ...",
            self.raw.len(),
            self.raw[..32].to_string()
        );
    }

    pub fn create_program_request(&self) -> Result<CreateProgramRequest, MessageConversionError> {
        let program = self.record.to_message()?;
        Ok(CreateProgramRequest {
            program: Some(program),
        })
    }
}

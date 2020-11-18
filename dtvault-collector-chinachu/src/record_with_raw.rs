use crate::recorded_program::{MessageConversionError, RecordedProgram};
use dtvault_types::shibafu528::dtvault::central::{CreateProgramRequest, UpdateProgramMetadataRequest};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Header as VideoHeader;
use serde_json::value::RawValue;
use std::ops::Deref;
use std::path::Path;

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

    pub fn create_program_request(&self) -> Result<CreateProgramRequest, MessageConversionError> {
        let program = self.record.to_message()?;
        Ok(CreateProgramRequest { program: Some(program) })
    }

    pub fn update_program_metadata_request(&self) -> Result<UpdateProgramMetadataRequest, MessageConversionError> {
        let id = self.record.to_identity()?;
        Ok(UpdateProgramMetadataRequest {
            program_id: Some(id),
            key: "chinachu_program_data".to_string(),
            value: self.raw.to_string(),
        })
    }

    pub fn video_header(&self) -> Result<VideoHeader, MessageConversionError> {
        let path = Path::new(&self.record.recorded);
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let mime_type = mime_guess::from_path(path)
            .first()
            .map(|m| m.essence_str().to_string())
            .unwrap_or_default();
        Ok(VideoHeader {
            provider_id: "dtvault-collector-chinachu".to_string(),
            program_id: Some(self.record.to_identity()?),
            total_length: path.metadata()?.len(),
            file_name,
            mime_type,
        })
    }
}

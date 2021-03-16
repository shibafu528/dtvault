use crate::chinachu::model::{MessageConversionError, RecordedProgram};
use dtvault_types::shibafu528::dtvault::central::{CreateProgramRequest, UpdateProgramMetadataRequest};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Header as VideoHeader;
use std::path::Path;

const PROGRAM_METADATA_KEY: &str = "chinachu_program_data";
const VIDEO_PROVIDER_ID: &str = "dtvault-collector-chinachu";

#[derive(Debug)]
pub struct RecordWithRaw {
    pub record: RecordedProgram,
    pub raw_json: String,
}

impl RecordWithRaw {
    pub fn from_str(json: &str) -> Result<RecordWithRaw, serde_json::Error> {
        Ok(RecordWithRaw {
            record: serde_json::from_str(json)?,
            raw_json: json.to_string(),
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
            key: PROGRAM_METADATA_KEY.to_string(),
            value: self.raw_json.to_string(),
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
            provider_id: VIDEO_PROVIDER_ID.to_string(),
            program_id: Some(self.record.to_identity()?),
            total_length: path.metadata()?.len(),
            file_name,
            mime_type,
        })
    }
}

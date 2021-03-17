use super::Program as StoredProgram;
use crate::program::prost_convert::ToDateTimeExt;
use crate::program::{MessageConversionError, Persistence};
use chrono::{DateTime, Utc};
use dtvault_types::shibafu528::dtvault::central::PersistProgramKey;
use dtvault_types::shibafu528::dtvault::{Program, ProgramIdentity};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProgramKey {
    start_at: DateTime<Utc>,
    network_id: u16,
    service_id: u16,
    event_id: u16,
}

impl ProgramKey {
    pub fn from_program(program: &Program) -> Self {
        ProgramKey {
            start_at: program.start_at.as_ref().unwrap().to_utc(),
            network_id: program.network_id as u16,
            service_id: program.service_id as u16,
            event_id: program.event_id as u16,
        }
    }

    pub fn from_stored_program(program: &StoredProgram) -> Self {
        ProgramKey {
            start_at: program.start_at,
            network_id: program.network_id as u16,
            service_id: program.service_id as u16,
            event_id: program.event_id as u16,
        }
    }

    pub fn from_program_id(program_id: &ProgramIdentity) -> Self {
        ProgramKey {
            start_at: program_id.start_at.as_ref().unwrap().to_utc(),
            network_id: program_id.network_id as u16,
            service_id: program_id.service_id as u16,
            event_id: program_id.event_id as u16,
        }
    }

    pub fn exchangeable(&self) -> ProgramIdentity {
        ProgramIdentity {
            start_at: Some(prost_types::Timestamp {
                seconds: self.start_at.timestamp(),
                nanos: self.start_at.timestamp_subsec_nanos() as i32,
            }),
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            event_id: self.event_id as u32,
        }
    }
}

impl fmt::Display for ProgramKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{network_id={}, service_id={}, event_id={}, start_at={}}}",
            self.network_id, self.service_id, self.event_id, self.start_at
        )
    }
}

impl Persistence<PersistProgramKey> for ProgramKey {
    fn from_persisted(persisted: PersistProgramKey) -> Result<Self, MessageConversionError> {
        Ok(ProgramKey {
            start_at: persisted.start_at.unwrap().to_utc(),
            network_id: persisted.network_id as u16,
            service_id: persisted.service_id as u16,
            event_id: persisted.event_id as u16,
        })
    }

    fn persist(&self) -> PersistProgramKey {
        PersistProgramKey {
            start_at: Some(prost_types::Timestamp {
                seconds: self.start_at.timestamp(),
                nanos: self.start_at.timestamp_subsec_nanos() as i32,
            }),
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            event_id: self.event_id as u32,
        }
    }
}

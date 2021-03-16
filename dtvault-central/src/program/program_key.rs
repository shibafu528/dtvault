use super::Program as StoredProgram;
use dtvault_types::shibafu528::dtvault::{Program, ProgramIdentity};
use std::fmt;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct ProgramKey {
    start_at: std::time::Duration,
    network_id: u16,
    service_id: u16,
    event_id: u16,
}

impl ProgramKey {
    pub fn from_program(program: &Program) -> Self {
        ProgramKey {
            start_at: program
                .start_at
                .as_ref()
                .map(|v| std::time::Duration::new(v.seconds as u64, v.nanos as u32))
                .unwrap(),
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
            start_at: program_id
                .start_at
                .as_ref()
                .map(|v| std::time::Duration::new(v.seconds as u64, v.nanos as u32))
                .unwrap(),
            network_id: program_id.network_id as u16,
            service_id: program_id.service_id as u16,
            event_id: program_id.event_id as u16,
        }
    }

    pub fn exchangeable(&self) -> ProgramIdentity {
        let start_at = prost_types::Duration::from(self.start_at);
        ProgramIdentity {
            start_at: Some(prost_types::Timestamp {
                seconds: start_at.seconds,
                nanos: start_at.nanos,
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
            self.network_id,
            self.service_id,
            self.event_id,
            self.start_at.as_secs_f64()
        )
    }
}

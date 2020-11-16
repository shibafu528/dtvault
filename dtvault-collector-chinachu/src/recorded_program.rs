use crate::program_id::{ParseProgramIDError, ProgramID};
use dtvault_types::shibafu528::dtvault::{
    Channel as ChannelPb, ChannelType, ExtendedEvent, Program, ProgramIdentity, Service,
};
use prost_types::{Duration, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordedProgram {
    pub id: String,
    pub start: u64,
    pub seconds: u64,
    pub title: String,
    pub full_title: String,
    pub detail: String, // beta: gammaにおけるdescription相当の内容, gamma: description+extra
    pub description: Option<String>,
    pub channel: Channel,
    pub extra: Option<Map<String, Value>>,
    pub recorded: String,
}

#[derive(thiserror::Error, Debug)]
pub enum MessageConversionError {
    #[error(transparent)]
    ParseProgramIDError(#[from] ParseProgramIDError),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Unexpected type of `{}`: {}", .name, .value)]
    UnexpectedType { name: String, value: String },
}

impl RecordedProgram {
    pub fn program_id(&self) -> Result<ProgramID, ParseProgramIDError> {
        self.id.parse()
    }

    pub fn to_message(&self) -> Result<Program, MessageConversionError> {
        let identity = self.to_identity()?;
        Ok(Program {
            network_id: identity.network_id,
            service_id: identity.service_id,
            event_id: identity.event_id,
            start_at: identity.start_at,
            duration: Some(Duration::from(std::time::Duration::from_secs(self.seconds))),
            name: self.title.clone(),
            description: self.short_description().clone(),
            extended: self.extra_to_extented_event()?,
            service: Some(self.channel.to_message()?),
        })
    }

    pub fn to_identity(&self) -> Result<ProgramIdentity, MessageConversionError> {
        let program_id = self.program_id()?;
        let start_at = Duration::from(std::time::Duration::from_millis(self.start));
        Ok(ProgramIdentity {
            network_id: program_id.nid.into(),
            service_id: program_id.sid.into(),
            event_id: program_id.eid.into(),
            start_at: Some(Timestamp {
                seconds: start_at.seconds,
                nanos: start_at.nanos,
            }),
        })
    }

    fn short_description(&self) -> &String {
        let option = self.description.as_ref();
        let detail = &self.detail;
        option.unwrap_or(detail)
    }

    fn extra_to_extented_event(&self) -> Result<Vec<ExtendedEvent>, MessageConversionError> {
        match self.extra.as_ref() {
            Some(extra) => {
                let mut vec = Vec::with_capacity(extra.len());
                for (k, v) in extra {
                    let key = k.clone();
                    let value = match v {
                        Value::String(str) => Ok(str.clone()),
                        _ => Err(MessageConversionError::UnexpectedType {
                            name: format!("extra.{}", key),
                            value: v.to_string(),
                        }),
                    }?;
                    vec.push(ExtendedEvent { key, value })
                }
                Ok(vec)
            }
            None => Ok(vec![]),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    #[serde(rename = "type")]
    pub channel_type: String,
    pub id: String,
    pub channel: String,
    pub name: String,
    pub nid: Option<u16>,
    pub sid: Value,
}

impl Channel {
    fn to_message(&self) -> Result<Service, MessageConversionError> {
        Ok(Service {
            network_id: self.nid.unwrap_or(0).into(),
            service_id: match &self.sid {
                Value::Number(num) => num.as_u64().unwrap() as u32,
                Value::String(str) => str.parse()?,
                _ => {
                    return Err(MessageConversionError::UnexpectedType {
                        name: "service_id".to_string(),
                        value: self.sid.to_string(),
                    })
                }
            },
            name: self.name.clone(),
            channel: Some(ChannelPb {
                channel_type: match self.channel_type.as_ref() {
                    "GR" => ChannelType::Gr,
                    "BS" => ChannelType::Bs,
                    "CS" => ChannelType::Cs,
                    "SKY" => ChannelType::Sky,
                    _ => ChannelType::ChannelUnknown,
                }
                .into(),
                channel: self.channel.clone(),
                name: self.name.clone(),
            }),
        })
    }
}

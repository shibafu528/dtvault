use crate::program::prost_convert::ToDurationExt;
use dtvault_types::shibafu528::dtvault as types;
use dtvault_types::shibafu528::dtvault::central::persist_program::ExtendedEvent as PersistExtendedEvent;
use dtvault_types::shibafu528::dtvault::central::{PersistChannel, PersistProgram, PersistService, PersistVideo};
use dtvault_types::shibafu528::dtvault::storage::create_video_request::Header as VideoHeader;
use mime::Mime;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum MessageConversionError {
    #[error("Missing required field `{0}`")]
    MissingRequiredField(String),
    #[error(transparent)]
    ParseUuidError(#[from] uuid::Error),
    #[error(transparent)]
    ParseMimeError(#[from] mime::FromStrError),
}

pub trait Persistence<T> {
    fn from_persisted(persisted: T) -> Result<Self, MessageConversionError>
    where
        Self: Sized;
    fn persist(&self) -> T;
}

#[derive(FromPrimitive, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize)]
pub enum ChannelType {
    GR = 1,
    BS = 2,
    CS = 3,
    Sky = 4,
}

#[derive(Clone, Serialize)]
pub struct Channel {
    channel_type: ChannelType,
    channel: String,
    name: String,
}

impl Channel {
    pub fn from_exchanged(channel: types::Channel) -> Result<Self, MessageConversionError> {
        Ok(Channel {
            channel_type: match types::ChannelType::from_i32(channel.channel_type) {
                Some(types::ChannelType::Gr) => Ok(ChannelType::GR),
                Some(types::ChannelType::Bs) => Ok(ChannelType::BS),
                Some(types::ChannelType::Cs) => Ok(ChannelType::CS),
                Some(types::ChannelType::Sky) => Ok(ChannelType::Sky),
                Some(_) | None => Err(MessageConversionError::MissingRequiredField("channel_type".to_string())),
            }?,
            channel: channel.channel,
            name: channel.name,
        })
    }

    pub fn exchangeable(&self) -> types::Channel {
        types::Channel {
            channel_type: self.channel_type as i32,
            channel: self.channel.clone(),
            name: self.name.clone(),
        }
    }
}

impl Persistence<PersistChannel> for Channel {
    fn from_persisted(persisted: PersistChannel) -> Result<Self, MessageConversionError> {
        Ok(Channel {
            channel_type: match ChannelType::from_i32(persisted.channel_type) {
                Some(v) => Ok(v),
                None => Err(MessageConversionError::MissingRequiredField("channel_type".to_string())),
            }?,
            channel: persisted.channel,
            name: persisted.name,
        })
    }

    fn persist(&self) -> PersistChannel {
        PersistChannel {
            channel_type: self.channel_type.clone() as i32,
            channel: self.channel.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Service {
    network_id: u16,
    service_id: u16,
    name: String,
    channel: Option<Channel>,
}

impl Service {
    pub fn from_exchanged(service: types::Service) -> Result<Self, MessageConversionError> {
        Ok(Service {
            network_id: service.network_id as u16,
            service_id: service.service_id as u16,
            name: service.name,
            channel: match service.channel {
                Some(ch) => Some(Channel::from_exchanged(ch)?),
                None => None,
            },
        })
    }

    pub fn exchangeable(&self) -> types::Service {
        types::Service {
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            name: self.name.clone(),
            channel: self.channel.as_ref().map(|ch| ch.exchangeable()),
        }
    }
}

impl Persistence<PersistService> for Service {
    fn from_persisted(persisted: PersistService) -> Result<Self, MessageConversionError> {
        Ok(Service {
            network_id: persisted.network_id as u16,
            service_id: persisted.service_id as u16,
            name: persisted.name,
            channel: match persisted.channel {
                Some(ch) => Some(Channel::from_persisted(ch)?),
                None => None,
            },
        })
    }

    fn persist(&self) -> PersistService {
        PersistService {
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            name: self.name.clone(),
            channel: self.channel.as_ref().map(|ch| ch.persist()),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Program {
    #[serde(with = "crate::serde::uuid")]
    id: Uuid,
    pub network_id: u16,
    pub service_id: u16,
    pub event_id: u16,
    pub start_at: Duration,
    duration: Duration,
    name: String,
    description: String,
    extended: Vec<ExtendedEvent>,
    service: Option<Service>,
    #[serde(skip)]
    metadata: HashMap<String, String>,
    #[serde(skip)]
    video_ids: Vec<Uuid>,
}

impl Program {
    pub fn from_exchanged(program: types::Program) -> Result<Self, MessageConversionError> {
        let start_at = program
            .start_at
            .ok_or_else(|| MessageConversionError::MissingRequiredField("start_at".to_string()))?;
        let duration = program
            .duration
            .ok_or_else(|| MessageConversionError::MissingRequiredField("duration".to_string()))?;

        Ok(Program {
            id: Uuid::new_v4(),
            network_id: program.network_id as u16,
            service_id: program.service_id as u16,
            event_id: program.event_id as u16,
            start_at: start_at.to_duration(),
            duration: duration.to_duration(),
            name: program.name,
            description: program.description,
            extended: program
                .extended
                .into_iter()
                .map(|e| ExtendedEvent::from_exchanged(e))
                .collect(),
            service: match program.service {
                Some(service) => Some(Service::from_exchanged(service)?),
                None => None,
            },
            metadata: HashMap::new(),
            video_ids: Vec::new(),
        })
    }

    pub fn exchangeable(&self) -> types::Program {
        types::Program {
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            event_id: self.event_id as u32,
            start_at: Some(prost_types::Timestamp {
                seconds: self.start_at.as_secs() as i64,
                nanos: self.start_at.subsec_nanos() as i32,
            }),
            duration: Some(self.duration.clone().into()),
            name: self.name.clone(),
            description: self.description.clone(),
            extended: self.extended.iter().map(|e| e.exchangeable()).collect(),
            service: match &self.service {
                Some(service) => Some(service.exchangeable()),
                None => None,
            },
        }
    }

    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.metadata
    }

    pub fn video_ids(&self) -> &Vec<Uuid> {
        &self.video_ids
    }

    pub fn video_ids_mut(&mut self) -> &mut Vec<Uuid> {
        &mut self.video_ids
    }
}

impl Persistence<PersistProgram> for Program {
    fn from_persisted(persisted: PersistProgram) -> Result<Self, MessageConversionError> {
        let start_at = persisted
            .start_at
            .ok_or_else(|| MessageConversionError::MissingRequiredField("start_at".to_string()))?;
        let duration = persisted
            .duration
            .ok_or_else(|| MessageConversionError::MissingRequiredField("duration".to_string()))?;

        Ok(Program {
            id: Uuid::parse_str(&persisted.id)?,
            network_id: persisted.network_id as u16,
            service_id: persisted.service_id as u16,
            event_id: persisted.event_id as u16,
            start_at: start_at.to_duration(),
            duration: duration.to_duration(),
            name: persisted.name,
            description: persisted.description,
            extended: persisted
                .extended
                .into_iter()
                .map(|e| ExtendedEvent::from_persisted(e).unwrap())
                .collect(),
            service: match persisted.service {
                Some(service) => Some(Service::from_persisted(service)?),
                None => None,
            },
            metadata: persisted.metadata,
            video_ids: persisted
                .video_ids
                .into_iter()
                .map(|v| Uuid::parse_str(&v).unwrap())
                .collect(),
        })
    }

    fn persist(&self) -> PersistProgram {
        PersistProgram {
            id: self
                .id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            network_id: self.network_id as u32,
            service_id: self.service_id as u32,
            event_id: self.event_id as u32,
            start_at: Some(prost_types::Timestamp {
                seconds: self.start_at.as_secs() as i64,
                nanos: self.start_at.subsec_nanos() as i32,
            }),
            duration: Some(self.duration.into()),
            name: self.name.clone(),
            description: self.description.clone(),
            extended: self.extended.iter().map(|e| e.persist()).collect(),
            service: self.service.as_ref().map(|s| s.persist()),
            metadata: self.metadata.clone(),
            video_ids: self
                .video_ids
                .iter()
                .map(|id| id.to_hyphenated().encode_lower(&mut Uuid::encode_buffer()).to_string())
                .collect(),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ExtendedEvent {
    key: String,
    value: String,
}

impl ExtendedEvent {
    pub fn from_exchanged(event: types::ExtendedEvent) -> Self {
        ExtendedEvent {
            key: event.key,
            value: event.value,
        }
    }

    pub fn exchangeable(&self) -> types::ExtendedEvent {
        types::ExtendedEvent {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl Persistence<PersistExtendedEvent> for ExtendedEvent {
    fn from_persisted(persisted: PersistExtendedEvent) -> Result<Self, MessageConversionError> {
        Ok(ExtendedEvent {
            key: persisted.key,
            value: persisted.value,
        })
    }

    fn persist(&self) -> PersistExtendedEvent {
        PersistExtendedEvent {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Video {
    #[serde(with = "crate::serde::uuid")]
    pub id: Uuid,
    pub provider_id: String,
    #[serde(with = "crate::serde::uuid")]
    program_id: Uuid,
    total_length: u64,
    pub file_name: String,
    original_file_name: String,
    #[serde(with = "crate::serde::mime")]
    mime_type: Mime,
    // TODO: 複数ストレージちゃんとやる時には考え直す
    #[serde(with = "crate::serde::uuid")]
    storage_id: Uuid,
    storage_prefix: String,
}

impl Video {
    pub fn from_exchanged(program: &Program, video_header: VideoHeader) -> Self {
        Video {
            id: Uuid::new_v4(),
            provider_id: video_header.provider_id.clone(),
            program_id: program.id,
            total_length: video_header.total_length,
            file_name: video_header.file_name.clone(),
            original_file_name: video_header.file_name.clone(),
            mime_type: video_header.mime_type.parse().unwrap(),
            storage_id: Uuid::nil(),
            storage_prefix: "".to_string(),
        }
    }

    pub fn exchangeable(&self) -> types::Video {
        types::Video {
            video_id: self
                .id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            provider_id: self.provider_id.clone(),
            program_id: None, // TODO: これどうしよ
            total_length: self.total_length,
            file_name: self.file_name.clone(),
            mime_type: self.mime_type.essence_str().to_string(),
            storage_id: self
                .storage_id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            prefix: self.storage_prefix.clone(),
        }
    }
}

impl Persistence<PersistVideo> for Video {
    fn from_persisted(persisted: PersistVideo) -> Result<Self, MessageConversionError> {
        Ok(Video {
            id: Uuid::parse_str(&persisted.video_id)?,
            provider_id: persisted.provider_id,
            program_id: Uuid::parse_str(&persisted.program_id)?,
            total_length: persisted.total_length,
            file_name: persisted.file_name,
            original_file_name: persisted.original_file_name,
            mime_type: persisted.mime_type.parse()?,
            storage_id: Uuid::parse_str(&persisted.storage_id)?,
            storage_prefix: persisted.storage_prefix,
        })
    }

    fn persist(&self) -> PersistVideo {
        PersistVideo {
            video_id: self
                .id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            provider_id: self.provider_id.clone(),
            program_id: self
                .program_id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            total_length: self.total_length,
            file_name: self.file_name.clone(),
            original_file_name: self.original_file_name.clone(),
            mime_type: self.mime_type.essence_str().to_string(),
            storage_id: self
                .storage_id
                .to_hyphenated()
                .encode_lower(&mut Uuid::encode_buffer())
                .to_string(),
            storage_prefix: self.storage_prefix.clone(),
        }
    }
}

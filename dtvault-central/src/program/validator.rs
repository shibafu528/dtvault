use dtvault_types::shibafu528::dtvault::{Channel, ProgramIdentity, Service};

pub fn validate_channel(value: &Channel) -> Result<&Channel, String> {
    if value.channel.is_empty() {
        return Err("Invalid value: channel".to_string());
    }
    if value.name.is_empty() {
        return Err("Invalid value: name".to_string());
    }

    Ok(value)
}

pub fn validate_service(value: &Service) -> Result<&Service, String> {
    if value.service_id == 0 {
        return Err("Invalid value: service_id".to_string());
    }
    if let Some(channel) = &value.channel {
        if let Err(msg) = validate_channel(channel) {
            return Err(format!("Violation in channel => {}", msg));
        }
    }

    Ok(value)
}

pub fn validate_program_id(value: &ProgramIdentity) -> Result<&ProgramIdentity, String> {
    if value.service_id == 0 {
        return Err("Invalid value: service_id".to_string());
    }
    if value.event_id == 0 {
        return Err("Invalid value: event_id".to_string());
    }
    if value.start_at == None {
        return Err("Missing value: start_at".to_string());
    }

    Ok(value)
}

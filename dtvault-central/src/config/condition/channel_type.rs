use super::Matcher;
use crate::program::ChannelType as ChannelTypeEnum;
use serde::{Deserialize, Deserializer};

#[derive(Debug)]
pub struct ChannelType {
    raw_value: String,
    values: Vec<ChannelTypeEnum>,
    invalid_values: Vec<String>,
}

impl ChannelType {
    fn new(value: String) -> Self {
        if value.is_empty() {
            return ChannelType {
                raw_value: value,
                values: vec![],
                invalid_values: vec![],
            };
        }

        let mut values = vec![];
        let mut invalid_values = vec![];
        for v in value.split(",") {
            let v = v.trim().to_uppercase();
            match v.as_str() {
                "GR" => values.push(ChannelTypeEnum::GR),
                "BS" => values.push(ChannelTypeEnum::BS),
                "CS" => values.push(ChannelTypeEnum::CS),
                "SKY" => values.push(ChannelTypeEnum::Sky),
                _ => invalid_values.push(v),
            }
        }

        ChannelType {
            raw_value: value,
            values,
            invalid_values,
        }
    }
}

impl<'de> Deserialize<'de> for ChannelType {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(ChannelType::new(String::deserialize(deserializer)?))
    }
}

impl Default for ChannelType {
    fn default() -> Self {
        ChannelType::new("".to_string())
    }
}

impl Matcher<ChannelTypeEnum> for ChannelType {
    fn validate(&self) -> Result<(), String> {
        todo!()
    }

    fn matches(&self, input: &ChannelTypeEnum) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let c = ChannelType::default();
        assert_eq!("", c.raw_value);
        assert!(c.values.is_empty());
    }
}

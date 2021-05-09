use super::Matcher;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::fmt::{Debug, Display};
use std::str::FromStr;

#[derive(Debug)]
pub struct LiteralOrRegex<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> {
    raw_value: String,
    value: LiteralOrRegexValue<T, E>,
}

#[derive(Debug)]
enum LiteralOrRegexValue<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> {
    Empty,
    Literal(T),
    InvalidLiteral(T::Err),
    Regex(Regex),
    InvalidRegex(regex::Error),
}

impl<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> LiteralOrRegex<T, E> {
    fn new(value: String) -> Self {
        if value.is_empty() {
            return LiteralOrRegex {
                raw_value: value,
                value: LiteralOrRegexValue::Empty,
            };
        }

        if value.starts_with("/") && value.ends_with("/") {
            let pattern = &value[1..value.len() - 1];
            match Regex::new(&pattern) {
                Ok(re) => LiteralOrRegex {
                    raw_value: value,
                    value: LiteralOrRegexValue::Regex(re),
                },
                Err(e) => LiteralOrRegex {
                    raw_value: value,
                    value: LiteralOrRegexValue::InvalidRegex(e),
                },
            }
        } else {
            let raw_value = value.clone();
            match value.parse() {
                Ok(v) => LiteralOrRegex {
                    raw_value,
                    value: LiteralOrRegexValue::Literal(v),
                },
                Err(e) => LiteralOrRegex {
                    raw_value,
                    value: LiteralOrRegexValue::InvalidLiteral(e),
                },
            }
        }
    }
}

impl<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> Matcher<T> for LiteralOrRegex<T, E> {
    fn validate(&self) -> Result<(), String> {
        match &self.value {
            LiteralOrRegexValue::InvalidLiteral(e) => Err(format!("invalid literal: {}", e)),
            LiteralOrRegexValue::InvalidRegex(e) => Err(format!("invalid regular expression: {}", e)),
            _ => Ok(()),
        }
    }

    fn matches(&self, input: &T) -> bool {
        match &self.value {
            LiteralOrRegexValue::Literal(lit) => *lit == *input,
            LiteralOrRegexValue::Regex(rg) => rg.is_match(&input.to_string()),
            _ => false,
        }
    }
}

impl<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> Default for LiteralOrRegex<T, E> {
    fn default() -> Self {
        LiteralOrRegex::new("".to_string())
    }
}

impl<'de, T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> Deserialize<'de> for LiteralOrRegex<T, E> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(LiteralOrRegex::new(String::deserialize(deserializer)?))
    }
}

pub type StringOrRegex = LiteralOrRegex<String, <String as FromStr>::Err>;
pub type Int32OrRegex = LiteralOrRegex<i32, <i32 as FromStr>::Err>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_default() {
        let c = StringOrRegex::default();
        match c.validate() {
            Ok(_) => {}
            Err(_) => assert!(false),
        }
        match &c.value {
            LiteralOrRegexValue::Empty => {}
            _ => assert!(false),
        }
    }

    #[test]
    fn test_int32_default() {
        let c = Int32OrRegex::default();
        match c.validate() {
            Ok(_) => {}
            Err(_) => assert!(false),
        }
        match &c.value {
            LiteralOrRegexValue::Empty => {}
            _ => assert!(false),
        }
    }

    #[test]
    fn test_string_literal_match() {
        let c = StringOrRegex::new("vis".to_string());
        c.validate().unwrap();
        assert!(c.matches(&"vis".to_string()));
        assert!(c.matches(&"television".to_string()));
    }

    #[test]
    fn test_int32_literal_match() {
        let c = Int32OrRegex::new("123".to_string());
        c.validate().unwrap();
        assert!(c.matches(&123));
        assert!(!c.matches(&112345));
    }
}

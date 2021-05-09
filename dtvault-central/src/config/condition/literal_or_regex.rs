use super::Matcher;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::fmt::{Debug, Display};
use std::str::FromStr;

#[derive(Debug)]
enum LiteralOrRegexValue<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> {
    Empty,
    Literal(T),
    InvalidLiteral(T::Err),
    Regex(Regex),
    InvalidRegex(regex::Error),
}

impl<T: FromStr<Err = E> + ToString + Clone + Eq, E: Debug + Display> LiteralOrRegexValue<T, E> {
    fn parse(value: &str) -> Self {
        if value.is_empty() {
            return LiteralOrRegexValue::Empty;
        }

        if value.starts_with("/") && value.ends_with("/") {
            let pattern = &value[1..value.len() - 1];
            match Regex::new(&pattern) {
                Ok(re) => LiteralOrRegexValue::Regex(re),
                Err(e) => LiteralOrRegexValue::InvalidRegex(e),
            }
        } else {
            match value.parse() {
                Ok(v) => LiteralOrRegexValue::Literal(v),
                Err(e) => LiteralOrRegexValue::InvalidLiteral(e),
            }
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            LiteralOrRegexValue::InvalidLiteral(e) => Err(format!("invalid literal: {}", e)),
            LiteralOrRegexValue::InvalidRegex(e) => Err(format!("invalid regular expression: {}", e)),
            _ => Ok(()),
        }
    }
}

macro_rules! defmatcher {
    ($id:ident, $t:ty) => {
        #[derive(Debug)]
        pub struct $id {
            raw_value: String,
            value: LiteralOrRegexValue<$t, <$t as FromStr>::Err>,
        }

        impl $id {
            fn new(value: String) -> Self {
                let parsed_value = LiteralOrRegexValue::parse(&value);
                $id {
                    raw_value: value,
                    value: parsed_value,
                }
            }
        }

        impl Default for $id {
            fn default() -> Self {
                Self::new("".to_string())
            }
        }

        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(Self::new(String::deserialize(deserializer)?))
            }
        }
    };
}

defmatcher!(StringOrRegex, String);
impl Matcher<String> for StringOrRegex {
    fn validate(&self) -> Result<(), String> {
        self.value.validate()
    }

    fn matches(&self, input: &String) -> bool {
        match &self.value {
            LiteralOrRegexValue::Literal(lit) => input.contains(lit),
            LiteralOrRegexValue::Regex(rg) => rg.is_match(&input.to_string()),
            _ => false,
        }
    }
}

defmatcher!(Int32OrRegex, i32);
impl Matcher<i32> for Int32OrRegex {
    fn validate(&self) -> Result<(), String> {
        self.value.validate()
    }

    fn matches(&self, input: &i32) -> bool {
        match &self.value {
            LiteralOrRegexValue::Literal(lit) => *lit == *input,
            LiteralOrRegexValue::Regex(rg) => rg.is_match(&input.to_string()),
            _ => false,
        }
    }
}

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

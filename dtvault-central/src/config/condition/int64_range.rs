use super::Matcher;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer};

static RANGE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:(?P<value>\d+)|(?P<op1>[<>]=?)\s*(?P<value1>\d+)|(?P<value2>\d+)\s*(?P<op2>[<>]=?)|(?P<range_begin>\d+)\s*(?P<op_range>\.\.\.?)\s*(?P<range_end>\d+))$",
    )
        .unwrap()
});

#[derive(Debug)]
pub struct Int64Range {
    raw_value: String,
    min: i64,
    max: i64,
    error: Option<String>,
}

impl Int64Range {
    fn new(value: String) -> Self {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Int64Range {
                raw_value: value,
                min: i64::min_value(),
                max: i64::max_value(),
                error: None,
            };
        }

        match RANGE_PATTERN.captures(&trimmed) {
            Some(captures) => {
                if let Some(v) = captures.name("value") {
                    let v = v.as_str().parse().unwrap();
                    Int64Range {
                        raw_value: value,
                        min: v,
                        max: v,
                        error: None,
                    }
                } else if let Some(op) = captures.name("op1") {
                    let v = captures.name("value1").unwrap().as_str().parse().unwrap();
                    match op.as_str() {
                        "<" => Int64Range {
                            raw_value: value,
                            min: i64::min_value(),
                            max: v - 1,
                            error: None,
                        },
                        "<=" => Int64Range {
                            raw_value: value,
                            min: i64::min_value(),
                            max: v,
                            error: None,
                        },
                        ">=" => Int64Range {
                            raw_value: value,
                            min: v,
                            max: i64::max_value(),
                            error: None,
                        },
                        ">" => Int64Range {
                            raw_value: value,
                            min: v + 1,
                            max: i64::max_value(),
                            error: None,
                        },
                        _ => panic!(),
                    }
                } else if let Some(op) = captures.name("op2") {
                    let v = captures.name("value2").unwrap().as_str().parse().unwrap();
                    match op.as_str() {
                        "<" => Int64Range {
                            raw_value: value,
                            min: v + 1,
                            max: i64::max_value(),
                            error: None,
                        },
                        "<=" => Int64Range {
                            raw_value: value,
                            min: v,
                            max: i64::max_value(),
                            error: None,
                        },
                        ">=" => Int64Range {
                            raw_value: value,
                            min: i64::min_value(),
                            max: v,
                            error: None,
                        },
                        ">" => Int64Range {
                            raw_value: value,
                            min: i64::min_value(),
                            max: v - 1,
                            error: None,
                        },
                        _ => panic!(),
                    }
                } else {
                    let op = captures.name("op_range").unwrap().as_str();
                    let inclusive = op == "..";

                    let min = captures.name("range_begin").unwrap().as_str().parse().unwrap();
                    let end = captures.name("range_end").unwrap().as_str().parse().unwrap();
                    let max = if inclusive { end } else { end - 1 };

                    if min <= max {
                        Int64Range {
                            raw_value: value,
                            min,
                            max,
                            error: None,
                        }
                    } else {
                        Int64Range {
                            raw_value: value,
                            min: max,
                            max: min,
                            error: None,
                        }
                    }
                }
            }
            None => Int64Range {
                raw_value: value,
                min: i64::min_value(),
                max: i64::max_value(),
                error: Some("invalid value".to_string()),
            },
        }
    }
}

impl<'de> Deserialize<'de> for Int64Range {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Int64Range::new(String::deserialize(deserializer)?))
    }
}

impl Default for Int64Range {
    fn default() -> Self {
        Int64Range::new("".to_string())
    }
}

impl Matcher<i64> for Int64Range {
    fn validate(&self) -> Result<(), String> {
        self.error.as_ref().map_or(Ok(()), |e| Err(e.clone()))
    }

    fn matches(&self, input: &i64) -> bool {
        self.min <= *input && *input <= self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let r = Int64Range::default();
        assert_eq!("", r.raw_value);
        assert_eq!(i64::min_value(), r.min);
        assert_eq!(i64::max_value(), r.max);
    }

    #[test]
    fn test_pre_op_lt() {
        let r = Int64Range::new("< 128".to_string());
        assert_eq!("< 128", r.raw_value);
        assert_eq!(i64::min_value(), r.min);
        assert_eq!(127, r.max);
    }

    #[test]
    fn test_pre_op_lg() {
        let r = Int64Range::new("<= 128".to_string());
        assert_eq!("<= 128", r.raw_value);
        assert_eq!(i64::min_value(), r.min);
        assert_eq!(128, r.max);
    }

    #[test]
    fn test_pre_op_gt() {
        let r = Int64Range::new("> 128".to_string());
        assert_eq!("> 128", r.raw_value);
        assert_eq!(129, r.min);
        assert_eq!(i64::max_value(), r.max);
    }

    #[test]
    fn test_pre_op_ge() {
        let r = Int64Range::new(">= 128".to_string());
        assert_eq!(">= 128", r.raw_value);
        assert_eq!(128, r.min);
        assert_eq!(i64::max_value(), r.max);
    }

    #[test]
    fn test_post_op_lt() {
        let r = Int64Range::new("128 <".to_string());
        assert_eq!("128 <", r.raw_value);
        assert_eq!(129, r.min);
        assert_eq!(i64::max_value(), r.max);
    }

    #[test]
    fn test_post_op_lg() {
        let r = Int64Range::new("128 <=".to_string());
        assert_eq!("128 <=", r.raw_value);
        assert_eq!(128, r.min);
        assert_eq!(i64::max_value(), r.max);
    }

    #[test]
    fn test_post_op_gt() {
        let r = Int64Range::new("128 >".to_string());
        assert_eq!("128 >", r.raw_value);
        assert_eq!(i64::min_value(), r.min);
        assert_eq!(127, r.max);
    }

    #[test]
    fn test_post_op_ge() {
        let r = Int64Range::new("128 >=".to_string());
        assert_eq!("128 >=", r.raw_value);
        assert_eq!(i64::min_value(), r.min);
        assert_eq!(128, r.max);
    }

    #[test]
    fn test_inclusive_range() {
        let r = Int64Range::new("2..64".to_string());
        assert_eq!("2..64", r.raw_value);
        assert_eq!(2, r.min);
        assert_eq!(64, r.max);
    }

    #[test]
    fn test_exclusive_range() {
        let r = Int64Range::new("2...64".to_string());
        assert_eq!("2...64", r.raw_value);
        assert_eq!(2, r.min);
        assert_eq!(63, r.max);
    }

    #[test]
    fn test_reverse_inclusive_range() {
        let r = Int64Range::new("64..2".to_string());
        assert_eq!("64..2", r.raw_value);
        assert_eq!(2, r.min);
        assert_eq!(64, r.max);
    }

    #[test]
    fn test_reverse_exclusive_range() {
        let r = Int64Range::new("64...2".to_string());
        assert_eq!("64...2", r.raw_value);
        assert_eq!(1, r.min);
        assert_eq!(64, r.max);
    }
}

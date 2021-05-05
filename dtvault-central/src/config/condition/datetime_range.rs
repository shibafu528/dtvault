use super::Matcher;
use chrono::{DateTime, Duration, Local, TimeZone};
use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer};

const DATETIME: &str =
    r"(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})(?:[T ](?P<hour>\d{2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?)?";
static VALUE_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(formatcp!(r"^{DATETIME}$")).unwrap());
static PRE_OP_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(formatcp!(r"^(?P<op>[<>]=?)\s*{DATETIME}$")).unwrap());
static POST_OP_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(formatcp!(r"^{DATETIME}\s*(?P<op>[<>]=?)$")).unwrap());

static RANGE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    let dt_replace_pattern = Regex::new(r"year|month|day|hour|minute|second").unwrap();
    let datetime_begin = dt_replace_pattern.replace_all(DATETIME, "${0}1");
    let datetime_end = dt_replace_pattern.replace_all(DATETIME, "${0}2");
    Regex::new(&format!(
        r"^{begin}\s*(?P<op_range>\.\.\.?)\s*{end}",
        begin = datetime_begin,
        end = datetime_end
    ))
    .unwrap()
});

const MIN_DATETIME: Lazy<DateTime<Local>> = Lazy::new(|| Local.ymd(2000, 1, 1).and_hms(0, 0, 0));
const MAX_DATETIME: Lazy<DateTime<Local>> =
    Lazy::new(|| Local.ymd(2100, 1, 1).and_hms(0, 0, 0) - Duration::nanoseconds(1));

#[derive(Debug)]
pub struct DateTimeRange {
    raw_value: String,
    min: DateTime<Local>,
    max: DateTime<Local>,
    error: Option<String>,
}

impl DateTimeRange {
    fn new(value: String) -> Self {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return DateTimeRange {
                raw_value: value,
                min: *MIN_DATETIME,
                max: *MAX_DATETIME,
                error: None,
            };
        }

        fn parse_datetime(captures: &regex::Captures, suffix: &str) -> DateTime<Local> {
            let year = captures
                .name(&format!("year{}", suffix))
                .unwrap()
                .as_str()
                .parse()
                .unwrap();
            let month = captures
                .name(&format!("month{}", suffix))
                .unwrap()
                .as_str()
                .parse()
                .unwrap();
            let day = captures
                .name(&format!("day{}", suffix))
                .unwrap()
                .as_str()
                .parse()
                .unwrap();
            let hour = captures.name(&format!("hour{}", suffix));
            let minute = captures.name(&format!("minute{}", suffix));
            let second = captures.name(&format!("second{}", suffix));

            let date = Local.ymd(year, month, day);
            if let Some(hour) = hour {
                let hour = hour.as_str().parse().unwrap();
                let minute = minute.unwrap().as_str().parse().unwrap();
                let second = second.map_or(0, |s| s.as_str().parse().unwrap());
                date.and_hms(hour, minute, second)
            } else {
                date.and_hms(0, 0, 0)
            }
        }

        if let Some(captures) = VALUE_PATTERN.captures(&trimmed) {
            let dt = parse_datetime(&captures, "");
            return DateTimeRange {
                raw_value: value,
                min: dt,
                max: dt,
                error: None,
            };
        }

        if let Some(captures) = PRE_OP_PATTERN.captures(&trimmed) {
            let op = captures.name("op").unwrap().as_str();
            let dt = parse_datetime(&captures, "");
            return match op {
                "<" => DateTimeRange {
                    raw_value: value,
                    min: *MIN_DATETIME,
                    max: dt - Duration::seconds(1),
                    error: None,
                },
                "<=" => DateTimeRange {
                    raw_value: value,
                    min: *MIN_DATETIME,
                    max: dt,
                    error: None,
                },
                ">=" => DateTimeRange {
                    raw_value: value,
                    min: dt,
                    max: *MAX_DATETIME,
                    error: None,
                },
                ">" => DateTimeRange {
                    raw_value: value,
                    min: dt + Duration::seconds(1),
                    max: *MAX_DATETIME,
                    error: None,
                },
                _ => panic!(),
            };
        }

        if let Some(captures) = POST_OP_PATTERN.captures(&trimmed) {
            let op = captures.name("op").unwrap().as_str();
            let dt = parse_datetime(&captures, "");
            return match op {
                "<" => DateTimeRange {
                    raw_value: value,
                    min: dt + Duration::seconds(1),
                    max: *MAX_DATETIME,
                    error: None,
                },
                "<=" => DateTimeRange {
                    raw_value: value,
                    min: dt,
                    max: *MAX_DATETIME,
                    error: None,
                },
                ">=" => DateTimeRange {
                    raw_value: value,
                    min: *MIN_DATETIME,
                    max: dt,
                    error: None,
                },
                ">" => DateTimeRange {
                    raw_value: value,
                    min: *MIN_DATETIME,
                    max: dt - Duration::seconds(1),
                    error: None,
                },
                _ => panic!(),
            };
        }

        if let Some(captures) = RANGE_PATTERN.captures(&trimmed) {
            let op = captures.name("op_range").unwrap().as_str();
            let inclusive = op == "..";

            let min = parse_datetime(&captures, "1");
            let end = parse_datetime(&captures, "2");
            let max = if inclusive { end } else { end - Duration::seconds(1) };

            return if min <= max {
                DateTimeRange {
                    raw_value: value,
                    min,
                    max,
                    error: None,
                }
            } else {
                DateTimeRange {
                    raw_value: value,
                    min: max,
                    max: min,
                    error: None,
                }
            };
        }

        DateTimeRange {
            raw_value: value,
            min: *MIN_DATETIME,
            max: *MAX_DATETIME,
            error: Some("invalid value".to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for DateTimeRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DateTimeRange::new(String::deserialize(deserializer)?))
    }
}

impl Default for DateTimeRange {
    fn default() -> Self {
        DateTimeRange::new("".to_string())
    }
}

impl Matcher<DateTime<Local>> for DateTimeRange {
    fn validate(&self) -> Result<(), String> {
        todo!()
    }

    fn matches(&self, input: &DateTime<Local>) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let r = DateTimeRange::default();
        assert_eq!("", r.raw_value);
        assert_eq!(*MIN_DATETIME, r.min);
        assert_eq!(*MAX_DATETIME, r.max);
    }

    #[test]
    fn test_value_ymd() {
        let v = "2020-02-03";
        let r = DateTimeRange::new(v.to_string());
        assert_eq!(v, r.raw_value);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(0, 0, 0), r.min);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(0, 0, 0), r.max);
    }

    #[test]
    fn test_value_ymd_hm() {
        let v = "2020-02-03 12:34";
        let r = DateTimeRange::new(v.to_string());
        assert_eq!(v, r.raw_value);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 0), r.min);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 0), r.max);
    }

    #[test]
    fn test_value_ymd_hms() {
        let v = "2020-02-03 12:34:56";
        let r = DateTimeRange::new(v.to_string());
        assert_eq!(v, r.raw_value);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 56), r.min);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 56), r.max);
    }

    #[test]
    fn test_pre_op_lt() {
        let v = "< 2020-02-03 12:34:56";
        let r = DateTimeRange::new(v.to_string());
        assert_eq!(v, r.raw_value);
        assert_eq!(*MIN_DATETIME, r.min);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 55), r.max);
    }

    #[test]
    fn test_inclusive_range() {
        let v = "2020-02-03 12:34:56..2021-12-31T01:02:03";
        let r = DateTimeRange::new(v.to_string());
        assert_eq!(v, r.raw_value);
        assert_eq!(Local.ymd(2020, 2, 3).and_hms(12, 34, 56), r.min);
        assert_eq!(Local.ymd(2021, 12, 31).and_hms(1, 2, 3), r.max);
    }
}

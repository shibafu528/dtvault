mod channel_type;
mod datetime_range;
mod int64_range;
mod literal_or_regex;

use self::channel_type::ChannelType;
use self::datetime_range::DateTimeRange;
use self::int64_range::Int64Range;
use self::literal_or_regex::{Int32OrRegex, StringOrRegex};
use crate::program::{Program, Video};
use num_traits::ToPrimitive;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::marker::PhantomData;

trait Matcher<T: Eq> {
    fn validate(&self) -> Result<(), String>;
    fn matches(&self, input: &T) -> bool;
}

trait BoundMatcher: Send + Sync {
    fn validate(&self) -> Result<(), String>;
    fn matches(&self, program: &Program, video: &Video) -> bool;
}

struct MatcherWithThunk<T, M, Thunk>
where
    T: Eq,
    M: Send + Sync + Matcher<T>,
    Thunk: Send + Sync + Fn(&Program, &Video, &M) -> bool,
{
    matcher: M,
    thunk: Thunk,
    _marker: PhantomData<fn() -> T>,
}

impl<T, M, Thunk> BoundMatcher for MatcherWithThunk<T, M, Thunk>
where
    T: Eq,
    M: Send + Sync + Matcher<T>,
    Thunk: Send + Sync + Fn(&Program, &Video, &M) -> bool,
{
    fn validate(&self) -> Result<(), String> {
        self.matcher.validate()
    }

    fn matches(&self, program: &Program, video: &Video) -> bool {
        (self.thunk)(program, video, &self.matcher)
    }
}

#[derive(Default)]
pub struct Condition {
    matchers: MatcherVec,
}

impl Condition {
    pub fn validate(&self) -> Result<(), String> {
        for matcher in &self.matchers {
            matcher.validate()?;
        }
        Ok(())
    }

    pub fn matches(&self, program: &Program, video: &Video) -> bool {
        for matcher in &self.matchers {
            if !matcher.matches(program, video) {
                return false;
            }
        }
        true
    }
}

impl fmt::Debug for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Condition ({} matchers)", self.matchers.len())
    }
}

type MatcherVec = Vec<Box<dyn BoundMatcher>>;

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let matchers = deserializer.deserialize_map(ConditionVisitor)?;
        Ok(Condition { matchers })
    }
}

struct ConditionVisitor;

macro_rules! parse_conditions {
    (from $access:expr; $($key:ident: $t:ty => $fn:expr$(;)*)+) => {{
        const KEYS: &'static [&'static str] = &[
            $(stringify!($key),)+
        ];

        let mut matchers = MatcherVec::new();

        while let Some(key) = $access.next_key()? {
            match key {
                $(
                    stringify!($key) => {
                        let value = $access.next_value::<$t>()?;
                        let bound = MatcherWithThunk {
                            matcher: value,
                            thunk: $fn,
                            _marker: PhantomData,
                        };
                        matchers.push(Box::new(bound));
                    }
                )+
                _ => return Err(serde::de::Error::unknown_field(key, KEYS)),
            }
        }

        matchers
    }}
}

impl<'de> Visitor<'de> for ConditionVisitor {
    type Value = MatcherVec;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        todo!()
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, <A as MapAccess<'de>>::Error>
    where
        A: MapAccess<'de>,
    {
        let matchers = parse_conditions! {
            from map;

            title: StringOrRegex => |program, _video, matcher| { matcher.matches(&program.name) };
            description: StringOrRegex => |program, _video, matcher| { matcher.matches(&program.description) };
            network_id: Int32OrRegex => |program, _video, matcher| { matcher.matches(&program.network_id.into()) };
            service_id: Int32OrRegex => |program, _video, matcher| { matcher.matches(&program.service_id.into()) };
            event_id: Int32OrRegex => |program, _video, matcher| { matcher.matches(&program.event_id.into()) };
            service_name: StringOrRegex => |program, _video, matcher| {
                match &program.service {
                    Some(s) => matcher.matches(&s.name),
                    None => false,
                }
            };
            channel_name: StringOrRegex => |program, _video, matcher| {
                if let Some(s) = &program.service {
                    if let Some(c) = &s.channel {
                        return matcher.matches(&c.name);
                    }
                }
                false
            };
            channel_type: ChannelType => |program, _video, matcher| {
                if let Some(s) = &program.service {
                    if let Some(c) = &s.channel {
                        return matcher.matches(&c.channel_type);
                    }
                }
                false
            };
            start_at: DateTimeRange => |program, _video, matcher| { matcher.matches(&program.start_at.with_timezone(&chrono::Local)) };
            video_total_length: Int64Range => |_program, video, matcher| {
                match video.total_length.to_i64() {
                    Some(v) => matcher.matches(&v),
                    None => false,
                }
            };
            video_mime_type: StringOrRegex => |_program, video, matcher| { matcher.matches(&video.mime_type.essence_str().to_string()) };
            video_provider_id: StringOrRegex => |_program, video, matcher| { matcher.matches(&video.provider_id) };
        };

        Ok(matchers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let input = r#"
            title = "hogehoge"
            description = "/^dtv/"
        "#;

        let cond: Condition = toml::from_str(input).unwrap();
        assert_eq!(2, cond.matchers.len());
    }
}

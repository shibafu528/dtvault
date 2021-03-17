pub trait ToDurationExt {
    fn to_duration(&self) -> std::time::Duration;
}

impl ToDurationExt for prost_types::Timestamp {
    fn to_duration(&self) -> std::time::Duration {
        std::time::Duration::new(self.seconds as u64, self.nanos as u32)
    }
}

impl ToDurationExt for prost_types::Duration {
    fn to_duration(&self) -> std::time::Duration {
        std::time::Duration::new(self.seconds as u64, self.nanos as u32)
    }
}

pub trait ToNaiveDateTimeExt {
    fn to_naive_utc(&self) -> chrono::NaiveDateTime;
}

impl ToNaiveDateTimeExt for prost_types::Timestamp {
    fn to_naive_utc(&self) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::from_timestamp(self.seconds, self.nanos as u32)
    }
}

pub trait ToDateTimeExt {
    fn to_utc(&self) -> chrono::DateTime<chrono::Utc>;
}

impl ToDateTimeExt for prost_types::Timestamp {
    fn to_utc(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_utc(self.to_naive_utc(), chrono::offset::Utc)
    }
}

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

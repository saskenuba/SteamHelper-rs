use chrono::{NaiveDate, TimeZone, Utc};

/// Represents a globally unique identifier within the Steam network.
/// Guaranteed to be unique across all racks and servers for a given Steam universe.

#[derive(Debug, Copy, Clone)]
struct GlobalID(u64);

impl GlobalID {
    fn sequence_count(&self) -> u64 {
        self.0 & 0xFFFFF
    }

    fn start_time_seconds(self) -> u64 {
        (self.0 >> 20) & 0x3FFFFFFF
    }

    fn start_time(self) -> NaiveDate {
        Utc.ymd(2005, 1, 1).naive_utc()
    }

    fn process_id(self) -> u64 {
        (self.0 >> 50) & 0xF
    }

    fn box_id(self) -> u64 {
        (self.0 >> 54) & 0x3FF
    }
}

#[derive(Debug, Copy, Clone)]
struct JobId(pub(crate) GlobalID);

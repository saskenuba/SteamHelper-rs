use super::Result;
use bytes::{BigEndian, ByteOrder};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct Time(pub(crate) u64);

impl Time {
    /// Creates a Time struct with the current local Unix time in seconds, plus
    /// the given offset.
    pub fn now(offset: Option<u64>) -> Result<Time> {
        let offset = offset.unwrap_or(0);
        let unix_time_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        Ok(Time(offset + unix_time_secs))
    }

    /// TODO: Add doc
    pub fn offset() -> Result<u64> {
        unimplemented!()
    }

    /// TODO: Add doc
    pub fn with_offset() -> Result<Time> {
        unimplemented!()
    }

    pub(crate) fn as_padded_buffer<'a>(&self, interval: Option<u64>) -> Vec<u8> {
        let interval = interval.unwrap_or(1);
        let mut buffer = vec![0; 8];

        BigEndian::write_u32(&mut buffer[4..], (self.0 / interval) as u32);
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_returns_seconds() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(Time::now(None).unwrap().0, now_seconds);
    }

    #[test]
    fn new_returns_seconds_with_offset() {
        let offset = 100;
        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(Time::now(Some(offset)).unwrap().0, now_seconds + offset);
    }

    fn as_padded_buffer_without_interval() {
        let seconds = 1572580000;
        let time = Time(seconds);
        let mut buffer = [0; 8];
        BigEndian::write_u32(&mut buffer[4..], time.0 as u32);

        assert_eq!(time.as_padded_buffer(None), buffer);
    }

    fn as_padded_buffer_with_interval() {
        let seconds = 1572580000;
        let interval = 30;
        let time = Time(seconds);
        let mut buffer = [0; 8];
        BigEndian::write_u32(&mut buffer[4..], (time.0 / 30) as u32);

        assert_eq!(time.as_padded_buffer(Some(30)), buffer);
    }
}

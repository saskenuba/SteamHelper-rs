use std::time::{SystemTime,SystemTimeError,UNIX_EPOCH};

#[derive(Debug)]
pub struct Time(u64);

impl Time {
    /// Creates a Time struct with the current local Unix time in seconds, plus
    /// the given offset.
    pub fn now(offset: Option<u64>) -> Result<Time, SystemTimeError> {
        let offset = offset.unwrap_or(0);
        let unix_time_secs = SystemTime::now().duration_since(UNIX_EPOCH)?
            .as_secs();

        Ok(Time(offset + unix_time_secs))
    }

    pub(crate) fn time(&self) -> u64 {
        self.0
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_returns_seconds() {
        use std::time::{SystemTime,UNIX_EPOCH};

        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_eq!(Time::now(None).unwrap().0, now_seconds);
    }

    #[test]
    fn new_returns_seconds_with_offset() {
        let offset = 100;
        let now_seconds = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_eq!(Time::now(Some(offset)).unwrap().0, now_seconds + offset);
    }

}

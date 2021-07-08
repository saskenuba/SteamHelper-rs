use std::fmt::Formatter;
use std::time::{SystemTime, UNIX_EPOCH};

use byteorder::{BigEndian, ByteOrder};

use super::{
    error::{SteamApiError, TotpError},
    steam_api::SteamApiResponse,
    Result,
};

/// Struct for working with TOTP time values.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Time(pub u64);

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Time {
    /// Creates a Time struct with the current local Unix time in seconds, plus
    /// the given offset.
    pub fn now(offset: Option<u64>) -> Result<Time> {
        let now_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let offset = offset.unwrap_or(0);

        Ok(Time(now_secs + offset))
    }

    /// Queries the Steam servers for their time, then subtracts our local time
    /// from it to get our offset.
    ///
    /// The offset is how many seconds we are _behind_ Steam. You can pass
    /// this value to `Time::now()` as-is with no math involved.
    ///
    /// # Example
    ///
    /// ```
    /// # use steam_totp::{Time};
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let offset = Time::offset().await?;
    /// let time = Time::now(Some(offset));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn offset() -> Result<u64> {
        let client = reqwest::Client::new();
        let response = {
            let res = client
                .post("http://api.steampowered.com/ITwoFactorService/QueryTime/v1/")
                .header(reqwest::header::CONTENT_LENGTH, 0)
                .send()
                .await?;

            if res.status() != reqwest::StatusCode::OK {
                return Err(TotpError::from(SteamApiError::BadStatusCode(res)));
            }

            res.json::<SteamApiResponse>().await?
        };

        let server_time = match response.response.server_time.parse::<u64>() {
            Ok(x) => x,
            Err(_) => {
                return Err(TotpError::from(SteamApiError::ParseServerTime(response)));
            }
        };

        let now_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let offset = server_time.saturating_sub(now_secs);

        Ok(offset)
    }

    /// Returns a `Time` value with computed offset from Steam servers.
    pub async fn with_offset() -> Result<Time> {
        Time::now(Some(Time::offset().await?))
    }

    pub(crate) fn as_padded_buffer(&self, interval: Option<u64>) -> Vec<u8> {
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

    #[test]
    fn as_padded_buffer_without_interval() {
        let seconds = 1572580000;
        let time = Time(seconds);
        let mut buffer = [0; 8];
        BigEndian::write_u32(&mut buffer[4..], time.0 as u32);

        assert_eq!(time.as_padded_buffer(None), buffer);
    }

    #[test]
    fn as_padded_buffer_with_interval() {
        let seconds = 1572580000;
        let interval = 30;
        let time = Time(seconds);
        let mut buffer = [0; 8];
        BigEndian::write_u32(&mut buffer[4..], (time.0 / interval) as u32);

        assert_eq!(time.as_padded_buffer(Some(30)), buffer);
    }
}

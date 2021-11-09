//! Allows to estimate the end of the trade lock, i.e, the time you can trade again after trading an item.
//!
//! The alternative way to do this is to call the inventory endpoint and get the `cache_expiration`.
//!
//! Can be enabled by adding the snippet below in your Cargo.toml:
//! ```toml
//! steam-trading = { version = "*", features = ["time"] }
//! ```

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};

pub const ONE_HOUR_SECONDS: i64 = 3600;
pub const ONE_WEEK_SECONDS: i64 = ONE_HOUR_SECONDS * 24 * 7;

// Steam "Midnight" is on 10:00 PST/GMT-8 or 18:00 UTC/GMT.
const STEAM_MIDNIGHT_OFFSET_UTC_SECONDS: i64 = ONE_HOUR_SECONDS * 18;
const PST_TO_UTC_OFFSET_SECONDS: i64 = ONE_HOUR_SECONDS * 8;

/// Adds the Steam Midnight offset to a completed trade time epoch
fn trade_time_with_offset(trade_complete_time_epoch: i64) -> DateTime<Utc> {
    let trade_utc = Utc.timestamp(trade_complete_time_epoch, 0);

    trade_utc + Duration::seconds(STEAM_MIDNIGHT_OFFSET_UTC_SECONDS)
}

pub fn estimate_tradelock_end(trade_completed_on_epoch: i64, trade_lock_duration_seconds: i64) -> NaiveDateTime {
    let trade_with_offset = trade_time_with_offset(trade_completed_on_epoch);
    let trade_lock_duration = Duration::seconds(trade_lock_duration_seconds);

    let end_date = trade_with_offset + trade_lock_duration;
    let tradelock_end_datetime = end_date.date().and_hms(0, 0, 0) + Duration::seconds(PST_TO_UTC_OFFSET_SECONDS);
    tradelock_end_datetime.naive_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade_complete_time_sample() -> i64 {
        1603998438
    }

    fn expected_tradelock_end() -> i64 {
        1604649600
    }

    #[test]
    fn t_estimate() {
        let estimated = estimate_tradelock_end(trade_complete_time_sample(), ONE_WEEK_SECONDS);
        assert_eq!(estimated.timestamp(), expected_tradelock_end());
    }
}

//! Module containing deserializable structs for Steam API responses.

use serde::Deserialize;

/// Response from the Steam API.
#[derive(Deserialize, Debug)]
pub struct SteamApiResponse {
    pub response: TimeQuery,
}

/// A Steam API `ITwoFactorService/QueryTime` response.
#[derive(Deserialize, Debug)]
pub struct TimeQuery {
    pub server_time: String,
}

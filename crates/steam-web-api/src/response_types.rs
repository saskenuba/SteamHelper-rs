//! This module contains responses for some endpoints, to be used with `ExecutorResponse` trait.
//! If you can't find it your method deserialized response, feel free to contribute and add it here!

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Base response for GetPlayerBans endpoint.
pub struct GetPlayerBansResponseBase {
    pub players: Vec<PlayerBans>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlayerBans {
    #[serde(rename = "SteamId")]
    pub steam_id: String,
    #[serde(rename = "CommunityBanned")]
    pub community_banned: bool,
    #[serde(rename = "VACBanned")]
    pub vacbanned: bool,
    #[serde(rename = "NumberOfVACBans")]
    pub number_of_vac_bans: i64,
    #[serde(rename = "DaysSinceLastBan")]
    pub days_since_last_ban: i64,
    #[serde(rename = "NumberOfGameBans")]
    pub number_of_game_bans: i64,
    #[serde(rename = "EconomyBan")]
    pub economy_ban: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Base response for GetPlayerSummaries endpoint.
pub struct GetPlayerSummariesResponseBase {
    pub response: GetPlayerSummariesPlayers,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPlayerSummariesPlayers {
    pub players: Vec<PlayerSummary>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSummary {
    pub steamid: String,
    pub communityvisibilitystate: i64,
    pub profilestate: i64,
    pub personaname: String,
    pub profileurl: String,
    pub avatar: String,
    pub avatarmedium: String,
    pub avatarfull: String,
    pub personastate: i64,
    pub realname: Option<String>,
    pub primaryclanid: String,
    pub timecreated: i64,
    pub personastateflags: i64,
    pub loccountrycode: Option<String>,
    pub locstatecode: Option<String>,
    pub loccityid: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Base response for GetCMList endpoint.
pub struct GetCMListResponseBase {
    pub response: GetCMListServerLists,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GetCMListServerLists {
    pub serverlist: Vec<String>,
    pub serverlist_websockets: Vec<String>,
    pub result: i64,
    pub message: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
/// Base response for GetTradeHoldDurations
pub struct GetTradeHoldDurationsResponseBase {
    pub response: GetCMListServerLists,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GetTradeHoldDurations {
    pub my_escrow: EscrowData,
    pub their_escrow: EscrowData,
    pub both_escrow: EscrowData,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EscrowData {
    pub escrow_end_duration_seconds: i64,
    pub escrow_end_date: Option<i64>,
    pub escrow_end_date_rfc3339: Option<String>,
}

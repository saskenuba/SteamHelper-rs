//! This API is not final
#[macro_use]
extern crate arrayref;
#[macro_use]
extern crate enum_primitive;

pub mod cmserver;
pub mod tcpconnection;
pub mod api;
pub mod messages;

struct SteamCMClient {
    /// steam_id of client
    steam_id: i32,
    ///
    session_id: i32,
}

// CM stands for CONTENT MANAGER

// We can query cm0.steampowered.com to get IP of the
// content manager server
// but only for fallback

// https://steamcommunity.com/dev/apikey
// api.steampowered.com
// get CM ip list
// https://api.steampowered.com/ISteamDirectory/GetCMList/v1/\?key={API_KEY}&cellid={STEAM_ID}

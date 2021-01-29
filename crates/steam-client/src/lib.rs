#![allow(dead_code)]
#![warn(missing_docs, missing_doc_code_examples)]
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_import_braces,
    unused_qualifications
)]

#[macro_use]
extern crate log;

use std::sync::Arc;

use lazy_static::lazy_static;

use steam_web_api::SteamAPI;

pub mod client;
mod content_manager;
pub mod config;
pub mod connection;
pub mod errors;
pub mod handlers;
pub mod messages;
pub(crate) mod utils;

lazy_static! {
    /// Internal Steam web API client
    pub(crate) static ref API_CLIENT: Arc<SteamAPI> = Arc::new(SteamAPI::new("1"));
}

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

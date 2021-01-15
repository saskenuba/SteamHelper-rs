//! This API is not final

#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![feature(associated_type_defaults)]
#![feature(box_syntax)]

#[macro_use]
extern crate arrayref;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate steam_language_gen_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate enum_dispatch;
/*#[macro_use]
extern crate tracing;
#[macro_use]
extern crate tracing_futures;*/


mod cmserver;
mod encrypted_connection;
pub mod config;
pub mod connection;
pub mod handlers;
pub mod messages;
pub mod steamclient;
pub mod webapi;

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

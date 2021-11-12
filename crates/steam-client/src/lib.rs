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
use tappet::SteamAPI;

pub mod client;
pub mod config;
pub mod connection;
mod content_manager;
pub mod errors;
pub mod handlers;
pub mod messages;
pub(crate) mod utils;

lazy_static! {
    /// Internal Steam web API client
    pub(crate) static ref API_CLIENT: Arc<SteamAPI> = Arc::new(SteamAPI::new("1"));
}

struct SteamCMClient {
    steam_id: i32,
    session_id: i32,
}

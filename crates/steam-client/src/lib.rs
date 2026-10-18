//! Client of the Steam CM (connection manager) protocol, the one the Steam client speaks.
//!
//! See [`client::SteamClient`] to connect and log on, and [`auth`] to log on to an account.
//!
//! Once logged on, [`spawn`](client::SteamClient::spawn) the client, as a bot should: a task then sends the
//! heartbeats, reads what Steam sends, and connects again when the connection is lost (see [`handle`]). Its handle
//! clones and calls concurrently; its [`Events`](handle::Events) are what Steam sends that no call waits for; its
//! [`SteamTask`](handle::SteamTask) ends once the client stopped, logged off. Without a task, keep awaiting the
//! client's methods, such as [`next_event`](client::SteamClient::next_event), or Steam drops it within a few minutes.
//!
//! Service methods are named by marker types ([`service::ServiceMethod`]): the crate's own in [`service::methods`],
//! which `call_service_method(request)` finds from the request, and any other with a marker type of yours and
//! `call::<Method>(request)`. Notifications Steam sends likewise ([`service::notifications`]).
//!
//! A bot that answers its friends' messages. Not tried live: account logons ask for the new Steam chat
//! (`chat_mode = 2`, as node-steam-user does), and the bot sets its persona online, which node-steam-user says
//! friend messages need; whether Steam then sends them to this crate is unverified.
//!
//! ```no_run
//! use steam_client::auth::RefreshToken;
//! use steam_client::client::SteamClient;
//! use steam_client::service::notifications::IncomingMessage;
//! use steam_language_gen::generated::enums::EPersonaState;
//! use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_SendMessage_Request;
//!
//! # async fn run(refresh_token: RefreshToken) -> Result<(), steam_client::errors::ClientError> {
//! let client = SteamClient::connect()
//!     .await?
//!     .login_with_refresh_token("account", refresh_token)
//!     .await?;
//! let (handle, mut events, task) = client.spawn();
//! // offline until set otherwise; set again after each reconnect
//! handle.set_persona_state(EPersonaState::Online)?;
//! // until the client stops: Steam logged it off for good (`Event::LoggedOff`, the last event), or `log_off`
//! while let Some(event) = events.recv().await {
//!     let Some(Ok(incoming)) = event.notification::<IncomingMessage>() else {
//!         continue;
//!     };
//!     let mut reply = CFriendMessages_SendMessage_Request::new();
//!     reply.set_steamid(incoming.steamid_friend());
//!     reply.set_chat_entry_type(1);
//!     reply.set_message(format!("You said: {}", incoming.message()));
//!     // in a task of its own, so the events keep being read meanwhile
//!     let handle = handle.clone();
//!     tokio::spawn(async move { handle.call_service_method(reply).await });
//! }
//! println!("Stopped: {:?}", task.await);
//! # Ok(())
//! # }
//! ```
//!
//! A game's coordinator, such as CS2's, talks to an account that plays the game: see [`gc`]. With the `cs2` feature,
//! `gc::cs2::Cs2` inspects CS2 items (pattern, float, stickers).

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

#[doc(hidden)]
pub mod __private;
pub mod auth;
pub mod client;
pub mod config;
pub mod connection;
mod content_manager;
pub mod errors;
pub mod events;
pub mod gc;
pub mod handle;
pub mod handlers;
pub mod messages;
mod protocol;
pub mod service;
pub(crate) mod utils;

lazy_static! {
    /// Internal Steam web API client
    pub(crate) static ref API_CLIENT: Arc<SteamAPI> = Arc::new(SteamAPI::new("1"));
}

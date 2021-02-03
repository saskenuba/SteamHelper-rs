//! Handle events through [PacketMessage] matching.
//!
//! For each kind of message, we have `Events`, that are incoming messages received through Steam3 Network.
//! And also the handlers, which can send your desired messages at any time.
//!
//! Events interest is registered on the [DispatcherMap], so we know that you are interested on hearing it.
//! They can be further inspected by selecting the proper variant.
//!
//! Register interest on:
//! SteamUser::LoggedOn
//!
//! Maybe you want to log off after log in:
//!
//! cx.log_off();
//! // or?
//! SteamUser::log_off(cx);
//!
//! Perhaps cx holds the call handler more information. So we can't really log in two times.. maybe?
//! But this only for the states that the user is interested in..
//!
//! ```rust
//! ```

// how to handle handler state efficiently?
// certainly there are handlers that depend on others current state.
// this needs to check out if it is really relevant.

use std::collections::HashMap;

use steam_protobuf::Message;
use tokio::sync::mpsc::UnboundedSender;

use crate::connection::DynBytes;
use crate::messages::packet::PacketMessage;

// we try to keep the same nomenclature as SteamKit2
pub mod steam_friends;
pub mod steam_user;

#[derive(Debug, Copy, Clone, Hash, PartialEq)]
/// Each message(EMsg) received maps to a certain kind of event.
/// If we are interested on it, we register the SteamEvent.
pub enum SteamEvents {
    SteamFriends,
    SteamUser,
}

trait HandlerKind {
    /// Each handler must implement a dispatch map, to connect emsgs to callbacks
    /// Find EMsg on dispatch map, and execute related function callback
    fn handle_msg(packet_message: PacketMessage) {}
}

// handles related to friends coming online etc

struct SourceId(u64);

/// Steam client is able to send messages at any time, listen to messages and react to them.
///
///
/// Should provide a hot path for quick lookup if user is listening for one specific emsg.
/// This could be done after the dispatch map is initiated.
///
/// Each Handler could have a mapping of [EMsg] that it listens for.
pub struct DispatcherMap {
    /// User is interested in listening for the followings events
    /// and will answer with a proper callback
    interested: HashMap<SteamEvents, String>,

    // we may need a source/target hashmap to track messages
    tracked_protobuf_messages: HashMap<u64, Box<dyn Message>>,

    sender: UnboundedSender<Vec<u8>>,
}

impl DispatcherMap {
    pub fn new() -> Self {
        Self {
            interested: Default::default(),
            tracked_protobuf_messages: Default::default(),
            sender: (),
        }
    }

    pub fn register_interest() {}
}

trait Dispatcher {}

//! Handle events through [PacketMessage] matching.

use std::collections::HashMap;

use steam_language_gen::generated::enums::EMsg;

use crate::messages::packetmessage::PacketMessage;

// we try to keep the same nomenclature as SteamKit2
pub mod steam_client;
pub mod steam_friends;

pub enum SteamEvents {
    SteamFriends,
    SteamUser,
    SteamClient,
}


trait HandlerKind {
    /// Each handler must implement a dispatch map, to connect emsgs to callbacks
    /// Find EMsg on dispatch map, and execute related function callback
    fn handle_msg(packet_message: PacketMessage) {}
}


/// handles related to friends coming online etc
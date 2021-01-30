//! Handle events through [PacketMessage] matching.

use crate::messages::packet::PacketMessage;

// we try to keep the same nomenclature as SteamKit2
pub mod steam_client;
pub mod steam_friends;

#[derive(Debug, Copy, Clone)]
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

// handles related to friends coming online etc

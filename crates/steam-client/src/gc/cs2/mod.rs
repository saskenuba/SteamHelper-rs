//! Counter-Strike 2 over its game coordinator, through a [`Cs2`] session: inspect links and the item details
//! (pattern, float, stickers) the game coordinator gives for them, players' profiles and ranks, matches and their
//! share codes, and the account's inventory.

mod client;
mod inventory;
mod item;
mod link;
mod matches;
mod profile;
mod share_code;

pub use client::*;
pub use inventory::Inventory;
pub use inventory::InventoryItem;
pub use inventory::InventoryItemError;
pub use item::*;
pub use link::*;
pub use matches::*;
pub use profile::*;
pub use share_code::*;

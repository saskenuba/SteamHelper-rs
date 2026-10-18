//! Counter-Strike 2 over its game coordinator: inspect links, and the item details (pattern, float, stickers) the
//! game coordinator gives for them, through a [`Cs2`] session.

mod client;
mod item;
mod link;

pub use client::*;
pub use item::*;
pub use link::*;

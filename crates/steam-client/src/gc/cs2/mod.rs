//! Counter-Strike 2 over its game coordinator, through a [`Cs2`] session: inspect links and the item details
//! (pattern, float, stickers) the game coordinator gives for them, players' profiles and ranks, matches and their
//! share codes, and the account's inventory.
//!
//! A session runs over a [spawned](crate::client::SteamClient::spawn) client, logged on to an account that owns CS2
//! (the free license). [`Cs2::connect`] plays CS2 and greets the game coordinator until it welcomes the account; the
//! welcome carries the inventory, which the session then keeps as the game coordinator changes it. Requests go one at
//! a time, [`REQUEST_COOLDOWN`] (1.1 s) apart: the game coordinator drops those that come faster. What no request
//! waits for comes from [`Cs2::next_event`].
//!
//! ```no_run
//! # async fn run(handle: steam_client::handle::SteamHandle<steam_client::client::User>)
//! # -> Result<(), Box<dyn std::error::Error>> {
//! use steam_client::gc::cs2::Cs2;
//! use steam_client::gc::cs2::Cs2Event;
//! use steam_client::gc::cs2::ShareCode;
//! use steamid_parser::SteamID;
//!
//! let mut cs2 = Cs2::connect(&handle).await?;
//! let link = concat!(
//!     "steam://rungame/730/76561202255233023/+csgo_econ_action_preview",
//!     "%20S76561198084749846A698323590D7935523998312483177",
//! );
//! let item = cs2.inspect_str(link).await?;
//! println!("pattern {:?}, float {:?}", item.pattern(), item.float());
//!
//! // a friend of the account who plays CS2
//! let friend = SteamID::from_steam64(76561198084749846);
//! let profile = cs2.player_profile(friend).await?;
//! let ranks: Vec<_> = profile.ranks().collect();
//! println!("level {:?}, ranks {:?}", profile.level(), ranks);
//!
//! let code: ShareCode = "CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN".parse()?;
//! for info in cs2.match_info(&code).await?.matches() {
//!     println!("{:?}: {:?}", info.map(), info.score());
//!     println!("demo {:?}", info.demo_url());
//! }
//!
//! let inventory = cs2.inventory();
//! println!("{} items", inventory.len());
//! if let Some(skin) = inventory.iter().find(|item| item.paint_index().is_some()) {
//!     println!("float {:?}, pattern {:?}", skin.float(), skin.pattern());
//! }
//!
//! loop {
//!     match cs2.next_event().await? {
//!         Cs2Event::ItemAcquired(item) => println!("Acquired {}", item.id()),
//!         Cs2Event::ItemRemoved(item) => println!("Removed {}", item.id()),
//!         Cs2Event::SessionLost => println!("Lost the session, greeting again"),
//!         Cs2Event::Welcomed => println!("Welcomed again: {} items", cs2.inventory().len()),
//!         _ => {}
//!     }
//! }
//! # }
//! ```
//!
//! Inspect links that embed their item, in hex, decode offline, without a session: [`EmbeddedItem::details`], or
//! [`Cs2::inspect`], which then sends nothing. The game coordinator answers profiles of the account's friends only,
//! node-globaloffensive says.
//!
//! The crate's examples `cs2_inspect` (inspect links) and `cs2_profile` (profiles, matches, the inventory and the
//! events) run these with a saved refresh token: see `examples/README.md`.
//!
//! Not covered: the inventories of other accounts (list them from Steam's web inventory, then inspect each item's
//! link), the items inside storage units (loading a unit isn't built: the [`Inventory`] has those the game
//! coordinator sent unasked), and whatever changes the account, such as moving items in or out of storage units,
//! name tags or stickers.

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

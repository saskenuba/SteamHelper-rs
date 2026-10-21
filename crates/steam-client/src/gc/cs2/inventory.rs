//! The account's CS2 inventory, as the game coordinator keeps it in the account's shared-object cache: sent whole in
//! its welcome, then changed object by object.

use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use std::mem;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bytes::Bytes;
use steam_protobuf::protobuf;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobuf::MessageField;
use steam_protobuf::protobufs::csgo::base_gcmessages::CSOEconItem;
use steam_protobuf::protobufs::csgo::base_gcmessages::CSOEconItemAttribute;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientWelcome;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOCacheSubscribed;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOCacheSubscriptionCheck;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOCacheSubscriptionRefresh;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOCacheUnsubscribed;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOIDOwner;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOMultipleObjects;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgSOSingleObject;
use steam_protobuf::protobufs::csgo::gcsystemmsgs::ESOMsg;
use steamid_parser::SteamID;
use thiserror::Error;

use crate::gc::cs2::StatTrak;
use crate::gc::cs2::Sticker;
use crate::gc::GcMessage;

/// The shared objects that are items (`CSOEconItem`); the others (account data, presets…) aren't kept.
const ITEM_TYPE: i32 = 1;

pub(crate) const SO_CREATE: u32 = ESOMsg::k_ESOMsg_Create as u32;
pub(crate) const SO_UPDATE: u32 = ESOMsg::k_ESOMsg_Update as u32;
pub(crate) const SO_DESTROY: u32 = ESOMsg::k_ESOMsg_Destroy as u32;
pub(crate) const SO_CACHE_SUBSCRIBED: u32 = ESOMsg::k_ESOMsg_CacheSubscribed as u32;
pub(crate) const SO_UPDATE_MULTIPLE: u32 = ESOMsg::k_ESOMsg_UpdateMultiple as u32;
pub(crate) const SO_CACHE_UNSUBSCRIBED: u32 = ESOMsg::k_ESOMsg_CacheUnsubscribed as u32;
pub(crate) const SO_CACHE_SUBSCRIPTION_CHECK: u32 = ESOMsg::k_ESOMsg_CacheSubscriptionCheck as u32;
pub(crate) const SO_CACHE_SUBSCRIPTION_REFRESH: u32 = ESOMsg::k_ESOMsg_CacheSubscriptionRefresh as u32;

/// The item attributes read, as node-globaloffensive (v3.3.0, `handlers.js` `_processSOEconItem`) reads them. Each
/// value is 4 little-endian bytes: a float or an integer, depending on the attribute.
mod attribute {
    /// The skin (paint kit), a float.
    pub(super) const PAINT_INDEX: u32 = 6;
    /// The pattern (paint seed), a float.
    pub(super) const PAINT_SEED: u32 = 7;
    /// The float (paint wear), a float.
    pub(super) const PAINT_WEAR: u32 = 8;
    /// When the item can be traded, a Unix time.
    pub(super) const TRADABLE_AFTER: u32 = 75;
    /// The StatTrak count.
    pub(super) const KILL_EATER_VALUE: u32 = 80;
    /// What the StatTrak counts.
    pub(super) const KILL_EATER_SCORE_TYPE: u32 = 81;
    /// The name tag's name, a `CAttribute_String`.
    pub(super) const CUSTOM_NAME: u32 = 111;
    /// Sticker `n`'s id is `STICKER_ID + 4 n`, its wear, scale and rotation (floats) the next three, for `n` < 6.
    pub(super) const STICKER_ID: u32 = 113;
    /// The quest.
    pub(super) const QUEST_ID: u32 = 168;
    /// How many items a storage unit holds.
    pub(super) const STORAGE_UNIT_COUNT: u32 = 270;
    /// The storage unit an item is in: the low 32 bits of its id, then the high ones.
    pub(super) const STORAGE_UNIT_LOW: u32 = 272;
    pub(super) const STORAGE_UNIT_HIGH: u32 = 273;
    /// Sticker `n`'s offset: x at `STICKER_OFFSET + 2 n`, y next (floats).
    pub(super) const STICKER_OFFSET: u32 = 278;
    /// Sticker `n`'s actual slot is at `STICKER_SCHEMA + n`, when set (node: "as of the 2024-02-06 update").
    pub(super) const STICKER_SCHEMA: u32 = 290;
}
/// How many stickers an item has room for.
const STICKER_SLOTS: u32 = 6;
/// The def index of a storage unit.
const STORAGE_UNIT: u32 = 1201;
/// In an item's `inventory`: the item is new, not acknowledged yet; then the low 16 bits aren't its position.
const NEW_ITEM: u32 = 1 << 30;

/// The account's CS2 inventory: its items by id, as the game coordinator last said.
///
/// Items in storage units count too when the game coordinator sent them (it may, unasked): tell them by
/// [`InventoryItem::storage_unit`].
///
/// ```no_run
/// # fn run(cs2: &mut steam_client::gc::cs2::Cs2) -> Result<(), steam_client::errors::ClientError> {
/// // none while it may be old
/// let Some(inventory) = cs2.inventory()? else {
///     return Ok(());
/// };
/// println!("{} items", inventory.len());
/// // the AK-47s (def index 7)
/// for item in inventory.iter().filter(|item| item.def_index() == 7) {
///     println!("{}: float {:?}", item.id(), item.float());
/// }
/// // the storage units, and those of their items the game coordinator sent
/// for unit in inventory.iter() {
///     if let Some(count) = unit.storage_unit_count() {
///         let sent = inventory.in_storage_unit(unit.id()).count();
///         println!("{:?}: {} items, {} sent", unit.custom_name(), count, sent);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inventory {
    items: BTreeMap<u64, InventoryItem>,
}

impl Inventory {
    /// The item with this id.
    pub fn get(&self, id: u64) -> Option<&InventoryItem> {
        self.items.get(&id)
    }

    /// The items, by id.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &InventoryItem> + '_ {
        self.items.values()
    }

    /// How many items there are.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The items of the storage unit with this id, among those the game coordinator sent.
    pub fn in_storage_unit(&self, storage_unit: u64) -> impl Iterator<Item = &InventoryItem> + '_ {
        self.iter()
            .filter(move |item| item.storage_unit() == Some(storage_unit))
    }
}

/// The account's inventory as [`Cs2`](crate::gc::cs2::Cs2) keeps it: the game coordinator's copy while it is known to
/// be current, and nothing otherwise. Never an old copy taken for a current one.
///
/// The copy is current from a cache of the account (`CacheSubscribed`, alone or in a welcome) until anything says it
/// may not be: the session ended, messages were dropped or couldn't be read (an item among them too), the game
/// coordinator unsubscribed the cache or holds another version of it, or welcomed the account again without it.
/// Changes that come meanwhile aren't applied: the next cache replaces the copy whole, and what it changed comes out as
/// changes.
#[derive(Debug, Default)]
pub(crate) struct KeptInventory {
    /// The items: the current ones while `current`, else the last ones known, which the next cache is compared with.
    items: Inventory,
    current: bool,
    /// Whether a cache came before: the first one changes nothing.
    loaded_before: bool,
    /// The version of the account's cache the items are at, when the game coordinator said.
    version: Option<u64>,
}

impl KeptInventory {
    /// The items, if they are the game coordinator's current ones as far as the messages read tell.
    pub(crate) fn current(&self) -> Option<&Inventory> {
        self.current.then_some(&self.items)
    }

    /// Forgets that the items are current. True if they were.
    pub(crate) fn invalidate(&mut self) -> bool {
        self.version = None;
        mem::replace(&mut self.current, false)
    }

    /// Reads a welcome, which starts a new session: the items are the account's cache it carries. Without one, they
    /// aren't current any more; when the welcome lists the account's cache as up to date, which it can't be since
    /// the hello names no version, asks for it.
    pub(crate) fn welcome(&mut self, welcome: &CMsgClientWelcome, owner: SteamID) -> Vec<Change> {
        if let Some(changes) = self.load(&welcome.outofdate_subscribed_caches, owner) {
            return changes;
        }
        let mut changes = Vec::new();
        if self.invalidate() {
            changes.push(Change::Unavailable);
        }
        if welcome
            .uptodate_subscribed_caches
            .iter()
            .any(|check| owned_by(&check.owner_soid, owner))
        {
            changes.push(Change::Refresh);
        }
        changes
    }

    /// Applies a shared-object message of the game coordinator: what changed, or `None` if it isn't one or isn't
    /// about `owner`'s cache. Without a session (`session_on` false), the account's are read as nothing: the next
    /// welcome brings the cache.
    pub(crate) fn apply(&mut self, message: &GcMessage, owner: SteamID, session_on: bool) -> Option<Vec<Change>> {
        let body = message.body();
        let parsed = match message.msg_type() {
            SO_CREATE | SO_UPDATE | SO_DESTROY => CMsgSOSingleObject::parse_from_tokio_bytes(body).map(|object| {
                if !owned_by(&object.owner_soid, owner) {
                    return None;
                }
                let is_item = object.type_id() == ITEM_TYPE;
                if !self.current || !session_on {
                    // other objects stay the caller's
                    return is_item.then(Vec::new);
                }
                // any object of the cache moves its version
                self.version = object.version.or(self.version);
                if !is_item {
                    return None;
                }
                let data = object.object_data.unwrap_or_default();
                let change = match message.msg_type() {
                    SO_DESTROY => item_id(&data).map(|id| self.items.destroy(id)),
                    // the game coordinator creates what it updates, and the other way around
                    _ => read(&data).map(|item| self.items.put(item)),
                };
                match change {
                    Some(change) => Some(change.into_iter().collect()),
                    // the copy may hold it as it was
                    None => Some(self.stale()),
                }
            }),
            SO_UPDATE_MULTIPLE => CMsgSOMultipleObjects::parse_from_tokio_bytes(body).map(|objects| {
                if !owned_by(&objects.owner_soid, owner) {
                    return None;
                }
                let items: Vec<_> = objects
                    .objects_modified
                    .iter()
                    .filter(|object| object.type_id() == ITEM_TYPE)
                    .collect();
                if !self.current || !session_on {
                    return (!items.is_empty()).then(Vec::new);
                }
                self.version = objects.version.or(self.version);
                if items.is_empty() {
                    return None;
                }
                let read: Option<Vec<_>> = items
                    .into_iter()
                    .map(|object| object.object_data.as_ref().and_then(read))
                    .collect();
                match read {
                    Some(items) => Some(items.into_iter().filter_map(|item| self.items.put(item)).collect()),
                    // the copy may hold one it can't read as it was
                    None => Some(self.stale()),
                }
            }),
            SO_CACHE_SUBSCRIBED => CMsgSOCacheSubscribed::parse_from_tokio_bytes(body).map(|cache| {
                if !session_on {
                    return owned_by(&cache.owner_soid, owner).then(Vec::new);
                }
                self.load([&cache], owner)
            }),
            SO_CACHE_UNSUBSCRIBED => CMsgSOCacheUnsubscribed::parse_from_tokio_bytes(body).map(|cache| {
                if !owned_by(&cache.owner_soid, owner) {
                    return None;
                }
                Some(if self.invalidate() {
                    vec![Change::Unavailable]
                } else {
                    Vec::new()
                })
            }),
            SO_CACHE_SUBSCRIPTION_CHECK => CMsgSOCacheSubscriptionCheck::parse_from_tokio_bytes(body).map(|check| {
                if !owned_by(&check.owner_soid, owner) {
                    return None;
                }
                if !session_on || (self.current && check.version.is_some() && check.version == self.version) {
                    return Some(Vec::new());
                }
                Some(self.stale())
            }),
            _ => return None,
        };
        parsed.unwrap_or_else(|error| {
            warn!("Unreadable shared object from CS2's game coordinator: {}", error);
            // whose it is can't be told: it may have changed the copy, or been the cache
            session_on.then(|| self.stale())
        })
    }

    /// The copy may be old: it isn't current until the game coordinator sends the cache again, which it is asked for.
    fn stale(&mut self) -> Vec<Change> {
        let mut changes = Vec::new();
        if self.invalidate() {
            changes.push(Change::Unavailable);
        }
        changes.push(Change::Refresh);
        changes
    }

    /// Replaces the items with those the caches of `owner` among `caches` hold, none if they hold none: a cache is the
    /// whole of it. What changed since the last items known, then [`Change::Loaded`]; `None` if no cache is `owner`'s.
    fn load<'a>(
        &mut self,
        caches: impl IntoIterator<Item = &'a CMsgSOCacheSubscribed>,
        owner: SteamID,
    ) -> Option<Vec<Change>> {
        let mut loaded = None;
        for cache in caches.into_iter().filter(|cache| owned_by(&cache.owner_soid, owner)) {
            let (items, version) = loaded.get_or_insert_with(|| (BTreeMap::new(), None));
            *version = cache.version.or(*version);
            for objects in cache.objects.iter().filter(|objects| objects.type_id() == ITEM_TYPE) {
                items.extend(
                    objects
                        .object_data
                        .iter()
                        .filter_map(read)
                        .map(|item| (item.id(), item)),
                );
            }
        }
        let (items, version) = loaded?;
        let mut changes = if mem::replace(&mut self.loaded_before, true) {
            changes_between(&self.items.items, &items)
        } else {
            Vec::new()
        };
        changes.push(Change::Loaded);
        self.items.items = items;
        self.current = true;
        self.version = version;
        Some(changes)
    }
}

impl Inventory {
    /// Adds `item`, or replaces the one with its id.
    fn put(&mut self, item: InventoryItem) -> Option<Change> {
        let new = item.clone();
        Some(match self.items.insert(item.id(), item) {
            Some(old) if old == new => return None,
            Some(old) => Change::Changed { old, new },
            None => Change::Acquired(new),
        })
    }

    /// Removes the item with `id`.
    fn destroy(&mut self, id: u64) -> Option<Change> {
        let removed = self.items.remove(&id);
        if removed.is_none() {
            debug!("CS2's game coordinator removed an item the inventory doesn't have");
        }
        removed.map(Change::Removed)
    }
}

/// What changed from `old` to `new`, by item id.
fn changes_between(old: &BTreeMap<u64, InventoryItem>, new: &BTreeMap<u64, InventoryItem>) -> Vec<Change> {
    let removed = old
        .iter()
        .filter(|(id, _)| !new.contains_key(id))
        .map(|(_, item)| Change::Removed(item.clone()));
    let acquired_or_changed = new.iter().filter_map(|(id, item)| match old.get(id) {
        None => Some(Change::Acquired(item.clone())),
        Some(before) if before != item => Some(Change::Changed {
            old: before.clone(),
            new: item.clone(),
        }),
        Some(_) => None,
    });
    removed.chain(acquired_or_changed).collect()
}

/// How the inventory changed, or what it needs.
#[derive(Debug)]
pub(crate) enum Change {
    /// A cache replaced it whole; the changes it brought came before.
    Loaded,
    /// It isn't current any more.
    Unavailable,
    /// The account's cache should be asked for again (`CacheSubscriptionRefresh`).
    Refresh,
    Acquired(InventoryItem),
    Changed {
        old: InventoryItem,
        new: InventoryItem,
    },
    Removed(InventoryItem),
}

/// Asks the game coordinator for `owner`'s cache again.
pub(crate) fn refresh(owner: SteamID) -> CMsgSOCacheSubscriptionRefresh {
    let mut refresh = CMsgSOCacheSubscriptionRefresh::new();
    let mut soid = CMsgSOIDOwner::new();
    soid.set_type(1);
    soid.set_id(owner.to_steam64());
    refresh.owner_soid = Some(soid).into();
    refresh
}

/// Whether a shared object (or cache) is `owner`'s: it says so, or names no owner.
fn owned_by(soid: &MessageField<CMsgSOIDOwner>, owner: SteamID) -> bool {
    soid.as_ref()
        .and_then(|soid| soid.id)
        .is_none_or(|id| id == owner.to_steam64())
}

/// Reads an item of the cache, or skips it with a warning.
fn read(data: &Bytes) -> Option<InventoryItem> {
    CSOEconItem::parse_from_tokio_bytes(data)
        .map_err(InventoryItemError::from)
        .and_then(InventoryItem::try_from)
        .map_err(|error| warn!("Skipping an item of CS2's game coordinator: {}", error))
        .ok()
}

/// The id of a serialized `CSOEconItem`, if it can be read.
fn item_id(data: &Bytes) -> Option<u64> {
    CSOEconItem::parse_from_tokio_bytes(data)
        .map(|item| item.id())
        .map_err(|error| warn!("Unreadable item from CS2's game coordinator: {}", error))
        .ok()
}

/// An item of the account's inventory: a `CSOEconItem`, read as what a skin is.
///
/// Read from the item's attributes as node-globaloffensive does; what the methods don't give is in
/// [`raw`](Self::raw) and [`attribute_bytes`](Self::attribute_bytes).
#[derive(Clone, PartialEq)]
pub struct InventoryItem {
    /// Has an id and a def index that aren't 0. Boxed: the events that carry items stay small.
    proto: Box<CSOEconItem>,
}

impl InventoryItem {
    /// Reads a serialized `CSOEconItem`.
    pub fn from_bytes(data: &[u8]) -> Result<Self, InventoryItemError> {
        Self::try_from(CSOEconItem::parse_from_bytes(data)?)
    }

    /// The item's id: its asset id in the inventory.
    pub fn id(&self) -> u64 {
        self.proto.id()
    }

    /// What the item is: the weapon, knife, gloves, sticker, case… as numbered in the game's `items_game.txt`.
    pub fn def_index(&self) -> u32 {
        self.proto.def_index()
    }

    /// The skin (paint kit). `None` for an item without one.
    pub fn paint_index(&self) -> Option<u32> {
        self.float_attribute(attribute::PAINT_INDEX)
            .filter(|index| index.is_finite() && *index >= 1.0)
            .map(|index| index as u32)
    }

    /// The pattern (paint seed), from 0 to 1000. `None` for an item without one.
    pub fn pattern(&self) -> Option<u32> {
        self.float_attribute(attribute::PAINT_SEED)
            .filter(|seed| seed.is_finite() && *seed >= 0.0)
            .map(|seed| seed as u32)
    }

    /// The float (paint wear), from 0 to 1. `None` for an item without one.
    pub fn float(&self) -> Option<f32> {
        self.float_attribute(attribute::PAINT_WEAR)
    }

    /// The quality, such as 4 for a plain item, 9 for StatTrak, 12 for Souvenir and 3 for a ★ knife or gloves.
    pub fn quality(&self) -> Option<u32> {
        self.proto.quality
    }

    /// The rarity, such as 1 for Consumer Grade up to 6 for Covert.
    pub fn rarity(&self) -> Option<u32> {
        self.proto.rarity
    }

    /// How the item came to be, such as 0 for a drop or 8 for a case opening.
    pub fn origin(&self) -> Option<u32> {
        self.proto.origin
    }

    /// How many the item counts for, for those that stack.
    pub fn quantity(&self) -> Option<u32> {
        self.proto.quantity
    }

    /// The StatTrak counter, if the item has one.
    pub fn stat_trak(&self) -> Option<StatTrak> {
        Some(StatTrak {
            score_type: self.u32_attribute(attribute::KILL_EATER_SCORE_TYPE).unwrap_or(0),
            value: self.u32_attribute(attribute::KILL_EATER_VALUE)?,
        })
    }

    /// The name tag's name.
    pub fn custom_name(&self) -> Option<&str> {
        self.proto
            .custom_name
            .as_deref()
            .or_else(|| attribute_string(self.attribute_bytes(attribute::CUSTOM_NAME)?))
            .filter(|name| !name.is_empty())
    }

    /// The stickers on the item, or the patches on an agent.
    pub fn stickers(&self) -> impl Iterator<Item = Sticker> + '_ {
        (0..STICKER_SLOTS).filter_map(move |slot| {
            let float = |attribute| self.float_attribute(attribute);
            let first = attribute::STICKER_ID + 4 * slot;
            let offset = attribute::STICKER_OFFSET + 2 * slot;
            Some(Sticker {
                slot: self.u32_attribute(attribute::STICKER_SCHEMA + slot).unwrap_or(slot),
                id: self.u32_attribute(first)?,
                wear: float(first + 1),
                scale: float(first + 2),
                rotation: float(first + 3),
                offset_x: float(offset),
                offset_y: float(offset + 1),
                offset_z: None,
            })
        })
    }

    /// When the item can be traded (it may have passed).
    pub fn tradable_after(&self) -> Option<SystemTime> {
        self.u32_attribute(attribute::TRADABLE_AFTER)
            .filter(|&time| time != 0)
            .map(|time| UNIX_EPOCH + Duration::from_secs(u64::from(time)))
    }

    /// The item's quest, for those that have one.
    pub fn quest_id(&self) -> Option<u32> {
        self.u32_attribute(attribute::QUEST_ID)
    }

    /// The storage unit the item is in, if it's in one.
    pub fn storage_unit(&self) -> Option<u64> {
        let low = self.u32_attribute(attribute::STORAGE_UNIT_LOW)?;
        let high = self.u32_attribute(attribute::STORAGE_UNIT_HIGH)?;
        Some(u64::from(high) << 32 | u64::from(low))
    }

    /// How many items the storage unit holds, if the item is one.
    pub fn storage_unit_count(&self) -> Option<u32> {
        (self.def_index() == STORAGE_UNIT).then(|| self.u32_attribute(attribute::STORAGE_UNIT_COUNT).unwrap_or(0))
    }

    /// Where the item is in the inventory, from 1. `None` while the item is new: not acknowledged yet.
    pub fn position(&self) -> Option<u16> {
        let inventory = self.proto.inventory?;
        (inventory & NEW_ITEM == 0).then_some(inventory as u16)
    }

    /// Whether the item is new: not acknowledged yet.
    pub fn is_new(&self) -> bool {
        self.proto.inventory() & NEW_ITEM != 0
    }

    /// The account whose item it is, when the game coordinator says.
    pub fn owner(&self) -> Option<SteamID> {
        self.proto
            .account_id
            .filter(|&id| id != 0)
            .map(SteamID::from_account_id)
    }

    /// The value of the item's attribute with this def index (as numbered in `items_game.txt`), as sent: often 4
    /// little-endian bytes of a float or an integer.
    pub fn attribute_bytes(&self, def_index: u32) -> Option<&[u8]> {
        self.attribute(def_index)?.value_bytes.as_deref()
    }

    /// The message the game coordinator gave.
    pub fn raw(&self) -> &CSOEconItem {
        &self.proto
    }

    /// The message the game coordinator gave.
    pub fn into_proto(self) -> CSOEconItem {
        *self.proto
    }

    fn attribute(&self, def_index: u32) -> Option<&CSOEconItemAttribute> {
        self.proto
            .attribute
            .iter()
            .find(|attribute| attribute.def_index() == def_index)
    }

    /// A 32-bit value: the first 4 bytes of `value_bytes`, or `value` when there are no bytes.
    fn u32_attribute(&self, def_index: u32) -> Option<u32> {
        let attribute = self.attribute(def_index)?;
        match attribute.value_bytes.as_deref() {
            Some(&[a, b, c, d, ..]) => Some(u32::from_le_bytes([a, b, c, d])),
            Some(_) => None,
            None => attribute.value,
        }
    }

    fn float_attribute(&self, def_index: u32) -> Option<f32> {
        self.u32_attribute(def_index).map(f32::from_bits)
    }
}

/// Fails when the message names no item: it has no id or no def index, or 0.
impl TryFrom<CSOEconItem> for InventoryItem {
    type Error = InventoryItemError;

    fn try_from(proto: CSOEconItem) -> Result<Self, Self::Error> {
        if proto.id() == 0 {
            return Err(InventoryItemError::NoId);
        }
        if proto.def_index() == 0 {
            return Err(InventoryItemError::NoDefIndex);
        }
        Ok(Self { proto: Box::new(proto) })
    }
}

/// Debug shows what the methods give.
impl fmt::Debug for InventoryItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InventoryItem")
            .field("id", &self.id())
            .field("def_index", &self.def_index())
            .field("paint_index", &self.paint_index())
            .field("pattern", &self.pattern())
            .field("float", &self.float())
            .field("quality", &self.quality())
            .field("rarity", &self.rarity())
            .field("stat_trak", &self.stat_trak())
            .field("custom_name", &self.custom_name())
            .field("stickers", &self.stickers().collect::<Vec<_>>())
            .field("storage_unit", &self.storage_unit())
            .field("storage_unit_count", &self.storage_unit_count())
            .field("position", &self.position())
            .finish()
    }
}

/// The string of a `CAttribute_String` (field 1, length-delimited), without copying it.
fn attribute_string(bytes: &[u8]) -> Option<&str> {
    let rest = bytes.strip_prefix(&[0x0A])?;
    let (length, rest) = varint(rest)?;
    let string = rest.get(..usize::try_from(length).ok()?)?;
    std::str::from_utf8(string).ok()
}

fn varint(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let mut value = 0;
    for (index, &byte) in bytes.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7F) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, &bytes[index + 1..]));
        }
    }
    None
}

/// Why a message isn't an inventory item.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InventoryItemError {
    /// The data isn't a `CSOEconItem`.
    #[error("The item could not be decoded: {0}")]
    Decode(#[from] protobuf::Error),
    /// The item has no id.
    #[error("The item has no id.")]
    NoId,
    /// The item has no def index.
    #[error("The item has no def index.")]
    NoDefIndex,
}

#[cfg(test)]
pub(crate) mod tests {
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CAttribute_String;
    use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::cmsg_socache_subscribed::SubscribedType;
    use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::cmsg_somultiple_objects::SingleObject;

    use super::*;

    pub(crate) fn attribute(def_index: u32, value: &[u8]) -> CSOEconItemAttribute {
        let mut attribute = CSOEconItemAttribute::new();
        attribute.set_def_index(def_index);
        attribute.set_value_bytes(Bytes::copy_from_slice(value));
        attribute
    }

    fn float(def_index: u32, value: f32) -> CSOEconItemAttribute {
        attribute(def_index, &value.to_le_bytes())
    }

    fn int(def_index: u32, value: u32) -> CSOEconItemAttribute {
        attribute(def_index, &value.to_le_bytes())
    }

    fn name(name: &str) -> CSOEconItemAttribute {
        let mut string = CAttribute_String::new();
        string.set_value(name.to_string());
        attribute(attribute::CUSTOM_NAME, &string.write_to_bytes().unwrap())
    }

    /// An AK-47 Case Hardened as the inventory has it: paint 44, pattern 661, float 0.2048, a sticker.
    pub(crate) fn item(id: u64) -> CSOEconItem {
        let mut item = CSOEconItem::new();
        item.set_id(id);
        item.set_account_id(124484118);
        item.set_def_index(7);
        item.set_inventory(3);
        item.set_quality(4);
        item.set_rarity(5);
        item.attribute = vec![
            float(attribute::PAINT_INDEX, 44.0),
            float(attribute::PAINT_SEED, 661.0),
            float(attribute::PAINT_WEAR, 0.204_828_55),
            int(attribute::STICKER_ID, 5032),
            float(attribute::STICKER_ID + 1, 0.5),
        ];
        item
    }

    pub(crate) fn object(item: &CSOEconItem) -> Bytes {
        item.write_to_bytes().unwrap().into()
    }

    /// The account's cache, holding `items`, and some account data (type 7).
    pub(crate) fn cache(owner: Option<SteamID>, items: &[CSOEconItem]) -> CMsgSOCacheSubscribed {
        let mut objects = SubscribedType::new();
        objects.set_type_id(ITEM_TYPE);
        objects.object_data = items.iter().map(object).collect();
        let mut other = SubscribedType::new();
        other.set_type_id(7);
        other.object_data = vec![Bytes::from_static(b"\x08\x01")];
        let mut cache = CMsgSOCacheSubscribed::new();
        cache.objects = vec![other, objects];
        if let Some(owner) = owner {
            let mut soid = CMsgSOIDOwner::new();
            soid.set_type(1);
            soid.set_id(owner.to_steam64());
            cache.owner_soid = Some(soid).into();
        }
        cache
    }

    fn owner() -> SteamID {
        SteamID::from_account_id(124484118)
    }

    #[test]
    fn reads_an_item() {
        let mut proto = item(25);
        proto.attribute.extend([
            int(attribute::KILL_EATER_VALUE, 1337),
            int(attribute::TRADABLE_AFTER, 1_700_000_000),
            name("Ayy lmao"),
            // a sticker in slot 2 that the game shows in slot 4, scraped, moved, the old way: `value`
            int(attribute::STICKER_ID + 8, 76),
            int(attribute::STICKER_SCHEMA + 2, 4),
            float(attribute::STICKER_ID + 8 + 1, 0.25),
            float(attribute::STICKER_ID + 8 + 2, 1.5),
            float(attribute::STICKER_ID + 8 + 3, 90.0),
            float(attribute::STICKER_OFFSET + 4, 0.125),
            {
                let mut offset = CSOEconItemAttribute::new();
                offset.set_def_index(attribute::STICKER_OFFSET + 5);
                offset.set_value((-0.25f32).to_bits());
                offset
            },
        ]);
        let item = InventoryItem::try_from(proto).unwrap();
        assert_eq!(item.id(), 25);
        assert_eq!(item.def_index(), 7);
        assert_eq!(item.paint_index(), Some(44));
        assert_eq!(item.pattern(), Some(661));
        assert!((item.float().unwrap() - 0.204_828_55).abs() < 1e-7);
        assert_eq!((item.quality(), item.rarity(), item.origin()), (Some(4), Some(5), None));
        let stat_trak = item.stat_trak().unwrap();
        assert_eq!((stat_trak.score_type, stat_trak.value), (0, 1337));
        assert_eq!(item.custom_name(), Some("Ayy lmao"));
        assert_eq!(
            item.tradable_after(),
            Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000))
        );
        let stickers: Vec<_> = item.stickers().collect();
        assert_eq!(stickers.len(), 2);
        assert_eq!(
            (stickers[0].slot, stickers[0].id, stickers[0].wear, stickers[0].scale),
            (0, 5032, Some(0.5), None)
        );
        let moved = stickers[1];
        assert_eq!((moved.slot, moved.id), (4, 76));
        assert_eq!(
            (moved.wear, moved.scale, moved.rotation),
            (Some(0.25), Some(1.5), Some(90.0))
        );
        assert_eq!(
            (moved.offset_x, moved.offset_y, moved.offset_z),
            (Some(0.125), Some(-0.25), None)
        );
        assert_eq!(item.position(), Some(3));
        assert!(!item.is_new());
        assert_eq!(item.owner(), Some(owner()));
        assert_eq!(item.storage_unit(), None);
        assert_eq!(item.storage_unit_count(), None);
        assert_eq!(
            item.attribute_bytes(attribute::PAINT_INDEX),
            Some(&44f32.to_le_bytes()[..])
        );
        assert_eq!(item.attribute_bytes(1), None);
        assert!(format!("{:?}", item).contains("custom_name: Some(\"Ayy lmao\")"));
    }

    #[test]
    fn reads_storage_units_and_what_they_hold() {
        let mut unit = CSOEconItem::new();
        unit.set_id(30);
        unit.set_def_index(STORAGE_UNIT);
        // new: no position
        unit.set_inventory(NEW_ITEM | 7);
        let empty = InventoryItem::try_from(unit.clone()).unwrap();
        assert_eq!(empty.storage_unit_count(), Some(0));
        assert_eq!(empty.position(), None);
        assert!(empty.is_new());
        unit.attribute.push(int(attribute::STORAGE_UNIT_COUNT, 12));
        unit.set_custom_name("Knives".to_string());
        let unit = InventoryItem::try_from(unit).unwrap();
        assert_eq!(unit.storage_unit_count(), Some(12));
        assert_eq!(unit.custom_name(), Some("Knives"));

        let mut stored = item(0x1_0000_0002);
        let unit_id: u64 = 0x0000_0005_0000_0009;
        stored.attribute.push(int(attribute::STORAGE_UNIT_LOW, unit_id as u32));
        stored
            .attribute
            .push(int(attribute::STORAGE_UNIT_HIGH, (unit_id >> 32) as u32));
        let stored = InventoryItem::try_from(stored).unwrap();
        assert_eq!(stored.storage_unit(), Some(unit_id));
        let mut kept = KeptInventory::default();
        assert!(kept
            .load([&cache(None, &[stored.raw().clone(), item(1)])], owner())
            .is_some());
        let inventory = kept.current().unwrap();
        let held: Vec<_> = inventory.in_storage_unit(unit_id).map(InventoryItem::id).collect();
        assert_eq!(held, [0x1_0000_0002]);
    }

    #[test]
    fn rejects_what_names_no_item() {
        let mut nameless = item(1);
        nameless.clear_id();
        assert!(matches!(
            InventoryItem::try_from(nameless),
            Err(InventoryItemError::NoId)
        ));
        let mut nameless = item(1);
        nameless.set_def_index(0);
        assert!(matches!(
            InventoryItem::try_from(nameless),
            Err(InventoryItemError::NoDefIndex)
        ));
        assert!(matches!(
            InventoryItem::from_bytes(b"\x0a"),
            Err(InventoryItemError::Decode(_))
        ));
        assert_eq!(InventoryItem::from_bytes(&object(&item(1))).unwrap().id(), 1);
    }

    #[test]
    fn reads_odd_attributes_as_unset() {
        let mut proto = item(1);
        proto.attribute = vec![
            // too short
            attribute(attribute::PAINT_SEED, &[1, 2]),
            float(attribute::PAINT_INDEX, f32::NAN),
            // not a `CAttribute_String`
            attribute(attribute::CUSTOM_NAME, b"\x12\x01a"),
        ];
        let item = InventoryItem::try_from(proto).unwrap();
        assert_eq!(item.pattern(), None);
        assert_eq!(item.paint_index(), None);
        assert_eq!(item.float(), None);
        assert_eq!(item.custom_name(), None);
        assert_eq!(item.stat_trak(), None);
        assert_eq!(item.stickers().count(), 0);
    }

    #[test]
    fn reads_the_strings_of_attributes() {
        assert_eq!(attribute_string(b"\x0a\x03abc"), Some("abc"));
        assert_eq!(attribute_string(b"\x0a\x00"), Some(""));
        // cut short, not UTF-8, no length
        assert_eq!(attribute_string(b"\x0a\x04abc"), None);
        assert_eq!(attribute_string(b"\x0a\x01\xff"), None);
        assert_eq!(attribute_string(b"\x0a\x80"), None);
        let long = "x".repeat(300);
        let mut string = CAttribute_String::new();
        string.set_value(long.clone());
        assert_eq!(attribute_string(&string.write_to_bytes().unwrap()), Some(long.as_str()));
    }

    fn ids(kept: &KeptInventory) -> Vec<u64> {
        kept.current().unwrap().iter().map(InventoryItem::id).collect()
    }

    #[test]
    fn loads_the_owners_items_only() {
        let mut inventory = KeptInventory::default();
        let someone = SteamID::from_account_id(5);
        assert!(inventory.load([&cache(Some(someone), &[item(1)])], owner()).is_none());
        assert!(inventory.load(None, owner()).is_none());
        assert!(inventory.current().is_none());

        let mut broken = item(3);
        broken.clear_def_index();
        let caches = [
            cache(Some(someone), &[item(1)]),
            cache(Some(owner()), &[item(2), broken, item(4)]),
        ];
        // the first cache changes nothing: it is the start
        assert!(matches!(
            inventory.load(&caches, owner()).as_deref(),
            Some([Change::Loaded])
        ));
        assert_eq!(ids(&inventory), [2, 4]);
        let current = inventory.current().unwrap();
        assert_eq!(current.len(), 2);
        assert!(current.get(2).is_some() && current.get(1).is_none());

        // a cache is the whole of it: one without items is an empty inventory
        let mut empty = cache(None, &[]);
        empty.objects.retain(|objects| objects.type_id() != ITEM_TYPE);
        match inventory.load([&empty], owner()).as_deref() {
            Some([Change::Removed(two), Change::Removed(four), Change::Loaded]) => {
                assert_eq!((two.id(), four.id()), (2, 4));
            }
            changes => panic!("unexpected changes: {:?}", changes),
        }
        assert!(ids(&inventory).is_empty());
        // and one with none, the same
        assert!(inventory.load([&cache(None, &[item(1)])], owner()).is_some());
        assert!(inventory.load([&cache(None, &[])], owner()).is_some());
        assert!(ids(&inventory).is_empty());
    }

    #[test]
    fn tells_what_changed_since_the_last_items_known() {
        let mut inventory = KeptInventory::default();
        let mut moved = item(2);
        inventory.load([&cache(None, &[item(1), moved.clone()])], owner());
        assert!(inventory.invalidate());
        assert!(!inventory.invalidate());
        assert!(inventory.current().is_none());
        // changes while it isn't current aren't applied
        let mut created = CMsgSOSingleObject::new();
        created.set_type_id(ITEM_TYPE);
        created.set_object_data(object_of(&item(9)));
        assert!(inventory
            .apply(&message(SO_CREATE, &created), owner(), true)
            .unwrap()
            .is_empty());
        moved.set_inventory(5);
        match inventory.load([&cache(None, &[moved, item(3)])], owner()).as_deref() {
            Some(
                [Change::Removed(removed), Change::Changed { old, new }, Change::Acquired(acquired), Change::Loaded],
            ) => {
                assert_eq!(removed.id(), 1);
                assert_eq!((old.position(), new.position()), (Some(3), Some(5)));
                assert_eq!(acquired.id(), 3);
            }
            changes => panic!("unexpected changes: {:?}", changes),
        }
        assert_eq!(ids(&inventory), [2, 3]);
    }

    #[test]
    fn takes_updates_of_several_objects() {
        let mut objects = CMsgSOMultipleObjects::new();
        let mut changed = item(1);
        changed.set_inventory(9);
        for (type_id, item) in [(ITEM_TYPE, &changed), (7, &item(2)), (ITEM_TYPE, &item(3))] {
            let mut object = SingleObject::new();
            object.set_type_id(type_id);
            object.set_object_data(object_of(item));
            objects.objects_modified.push(object);
        }
        let mut inventory = KeptInventory::default();
        inventory.load([&cache(None, &[item(1)])], owner());
        let changes = inventory
            .apply(&message(SO_UPDATE_MULTIPLE, &objects), owner(), true)
            .unwrap();
        match changes.as_slice() {
            [Change::Changed { old, new }, Change::Acquired(acquired)] => {
                assert_eq!((old.position(), new.position()), (Some(3), Some(9)));
                assert_eq!(acquired.id(), 3);
            }
            changes => panic!("unexpected changes: {:?}", changes),
        }
        // the same again changes nothing
        assert!(inventory
            .apply(&message(SO_UPDATE_MULTIPLE, &objects), owner(), true)
            .unwrap()
            .is_empty());
        // objects that aren't items aren't the inventory's
        objects.objects_modified.retain(|object| object.type_id() != ITEM_TYPE);
        assert!(inventory
            .apply(&message(SO_UPDATE_MULTIPLE, &objects), owner(), true)
            .is_none());
        // without a session, nothing is applied
        assert!(inventory.invalidate());
        let mut objects = CMsgSOMultipleObjects::new();
        let mut object = SingleObject::new();
        object.set_type_id(ITEM_TYPE);
        object.set_object_data(object_of(&item(4)));
        objects.objects_modified.push(object);
        inventory.load([&cache(None, &[item(1)])], owner());
        assert!(inventory
            .apply(&message(SO_UPDATE_MULTIPLE, &objects), owner(), false)
            .unwrap()
            .is_empty());
        assert_eq!(ids(&inventory), [1]);
    }

    fn object_of(item: &CSOEconItem) -> Bytes {
        object(item)
    }

    fn message<M: Message>(msg_type: u32, body: &M) -> GcMessage {
        use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
        let payload =
            crate::gc::tests::proto_payload(msg_type, &CMsgProtoBufHeader::new(), &body.write_to_bytes().unwrap());
        crate::gc::tests::raw(crate::gc::tests::from_gc(730, msg_type | 0x8000_0000, payload))
            .gc_message()
            .unwrap()
            .unwrap()
    }
}

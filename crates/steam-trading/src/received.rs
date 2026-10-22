//! Where the items of an accepted offer went, found through CS2's game coordinator (feature `cs2`).
//!
//! Since ~2026-04 Steam's trade history has no `new_assetid` for CS2 trades. The item already has its new id right
//! after the accept, and the game coordinator's item id is the web asset id (seen live 2026-10-20/21), so the new ids
//! are those the game coordinator's inventory gained. Telling which received item is which comes from what a trade
//! keeps: the pattern and wear (the web prints the game coordinator's 32-bit float in full) and the kind (`classid`).
//! Spec: `docs/specs/2026-10-21-received-items.md`.
//!
//! Two ways in: [`accept_and_locate`] accepts and waits for what the inventory gains over a snapshot taken right
//! before; [`find_and_locate`], for an offer the partner accepted earlier, looks once at what the inventory has beyond
//! the ids the caller knows, with the fingerprints the caller kept. Both place items with [`locate`].

use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;

use futures_timer::Delay;
use steam_client::gc::cs2::Cs2;
use tappet::response_types::CEcon_Asset;
use tracing::debug;
use tracing::warn;

use crate::Inventory;
use crate::InventoryContext;
use crate::InventoryItem;
use crate::TradeError;

/// How long to wait for the received items by default.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the game coordinator's inventory is looked at while waiting. Local state: no request.
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Reads of our own web inventory for the kinds of the new items after an accept, at most: Steam serves it from a
/// cache.
const KIND_READS: usize = 3;
/// Between two of those reads.
const KIND_READ_INTERVAL: Duration = Duration::from_secs(2);
/// Where our own web inventory is read for kinds once the partner accepted: the trade-protected context, where received
/// items land, then the tradable one for those whose protection ended.
const PARTNER_ACCEPT_KIND_READS: [InventoryContext; 2] = [InventoryContext::CS2_TRADE_PROTECTED, InventoryContext::CS2];

/// An offer accepted by
/// [`SteamTradeManager::accept_offer_and_find_items`](crate::SteamTradeManager::accept_offer_and_find_items), and
/// where its items went.
#[derive(Debug, Clone)]
pub struct AcceptedOffer {
    /// The trade's id, as [`SteamTradeManager::accept_offer`](crate::SteamTradeManager::accept_offer) returns it:
    /// `None` when Steam asked for a mobile confirmation, whose answer has no id.
    pub tradeid: Option<i64>,
    /// One per item received, in the offer's order. Entries of the offer that couldn't be read are left out (see
    /// `has_malformed_items()` on the offer).
    pub received: Vec<ReceivedItem>,
}

/// An item received, and where it went.
#[derive(Debug, Clone)]
pub struct ReceivedItem {
    /// The item as the offer listed it: its `assetid` is the partner's, from before the trade.
    pub offered: CEcon_Asset,
    pub located: Located,
}

/// How a received item was found in the game coordinator's inventory, with its new asset id.
///
/// After [`SteamTradeManager::accept_offer_and_find_items`](crate::SteamTradeManager::accept_offer_and_find_items),
/// items that came from elsewhere while waiting (a drop, another trade) can be taken for received ones in
/// [`Located::Only`] and [`Located::Guessed`].
/// [`SteamTradeManager::find_received_items`](crate::SteamTradeManager::find_received_items) can't tell which new items
/// are the trade's, so it places an item without a [`Fingerprint`] only on a new item read to be of its kind: never
/// [`Located::Only`], and [`Located::Guessed`] only among new items of its kind.
///
/// An item with a fingerprint is only ever placed on a new item that matches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Located {
    /// The only one left: one received item not found otherwise, and one new item.
    Only(u64),
    /// Pattern and wear (as `f32` bits) equal the partner's copy's [`Fingerprint`]: read from the partner's web
    /// inventory before the accept, or given by the caller.
    PatternAndWear(u64),
    /// The only new item of its kind (`classid`), from our own web inventory.
    Kind(u64),
    /// Not told apart from others of its kind, or whose kind couldn't be read: the set of new ids is right, which of
    /// them is this item may not be.
    Guessed(u64),
    /// Nothing new for it: before the timeout after an accept, or in the inventory as it is when the partner accepted;
    /// there, for an item without a fingerprint, no new item read to be of its kind. Also items of other games than
    /// CS2, which the game coordinator doesn't hold.
    NotFound,
}

impl Located {
    /// The item's new asset id, if it was found.
    pub fn assetid(&self) -> Option<u64> {
        match *self {
            Located::Only(id) | Located::PatternAndWear(id) | Located::Kind(id) | Located::Guessed(id) => Some(id),
            Located::NotFound => None,
        }
    }
}

/// What the game coordinator tells about an item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GcItem {
    pub(crate) id: u64,
    pub(crate) pattern: Option<u32>,
    pub(crate) float: Option<f32>,
}

/// The game coordinator's inventory of the account, read without taking anyone's events.
pub(crate) trait ItemSource {
    /// The ids of the items, `None` while the inventory isn't current.
    fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError>;

    /// The items whose id isn't in `known`, in id order, `None` while the inventory isn't current. Items in storage
    /// units are left out: no trade puts an item there.
    fn items_not_in(&mut self, known: &HashSet<u64>) -> Result<Option<Vec<GcItem>>, TradeError>;
}

/// `Cs2::inventory` reads the messages that came (they become the caller's events, as during any request) and gives
/// the local copy: no request, nothing taken from the caller.
impl ItemSource for Cs2 {
    fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError> {
        let inventory = self.inventory().map_err(TradeError::GameCoordinatorError)?;
        Ok(inventory.map(|inventory| inventory.iter().map(|item| item.id()).collect()))
    }

    fn items_not_in(&mut self, known: &HashSet<u64>) -> Result<Option<Vec<GcItem>>, TradeError> {
        let inventory = self.inventory().map_err(TradeError::GameCoordinatorError)?;
        Ok(inventory.map(|inventory| {
            inventory
                .iter()
                .filter(|item| !known.contains(&item.id()) && item.storage_unit().is_none())
                .map(|item| GcItem {
                    id: item.id(),
                    pattern: item.pattern(),
                    float: item.float(),
                })
                .collect()
        }))
    }
}

/// What a trade keeps of an item and tells copies of one kind apart: its pattern and wear. Commodities (cases,
/// graffiti) have neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint {
    pattern: i64,
    /// The web's wear as the game coordinator's `f32`: the web prints that float in full, so the bits are equal.
    wear_bits: u32,
}

impl Fingerprint {
    /// From the pattern ("Pattern Template") and the wear's bits as `f32`: `(wear as f32).to_bits()` of the web's
    /// "Wear Rating", or the game coordinator's float's.
    pub const fn new(pattern: i64, wear_bits: u32) -> Self {
        Self { pattern, wear_bits }
    }

    /// An item's, from its web inventory: `None` without a pattern or a wear.
    pub fn of(item: &InventoryItem<'_>) -> Option<Self> {
        Some(Self::new(item.pattern()?, (item.wear()? as f32).to_bits()))
    }

    pub const fn pattern(&self) -> i64 {
        self.pattern
    }

    pub const fn wear_bits(&self) -> u32 {
        self.wear_bits
    }

    fn matches(&self, item: &GcItem) -> bool {
        item.pattern.map(i64::from) == Some(self.pattern) && item.float.map(f32::to_bits) == Some(self.wear_bits)
    }
}

/// A received item, as the matching sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wanted {
    classid: i64,
    /// Whether it is a CS2 item: the game coordinator holds no other.
    cs2: bool,
    /// The partner's copy's, when known.
    fingerprint: Option<Fingerprint>,
    /// Whether it may only be placed on a new item read to be of its kind: without a fingerprint, when the new items
    /// may not all be the trade's.
    kind_required: bool,
}

/// Which items the game coordinator's inventory gained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewItems {
    /// Right after our accept: the trade's, so an item may also be placed by elimination or among new items of
    /// unknown kind.
    OfTheTrade,
    /// Those not known to the caller once the partner accepted: some may have come another way.
    MaybeOthers,
}

impl Wanted {
    fn new(offered: &CEcon_Asset, fingerprint: Option<Fingerprint>, new_items: NewItems) -> Self {
        Self {
            classid: offered.classid,
            cs2: offered.appid == i64::from(InventoryContext::CS2.appid()),
            fingerprint,
            kind_required: fingerprint.is_none() && new_items == NewItems::MaybeOthers,
        }
    }

    /// Whether `item` may be this one: with a fingerprint, only an item that matches it.
    fn could_be(&self, item: &GcItem) -> bool {
        match self.fingerprint {
            Some(fingerprint) => fingerprint.matches(item),
            None => true,
        }
    }
}

/// What the game coordinator alone tells: the items it places, and the new items left.
struct Placed {
    located: Vec<Option<Located>>,
    /// In id order.
    free: Vec<GcItem>,
}

impl Placed {
    fn open(&self) -> impl Iterator<Item = usize> + '_ {
        self.located
            .iter()
            .enumerate()
            .filter(|(_, located)| located.is_none())
            .map(|(index, _)| index)
    }

    fn take(&mut self, wanted: usize, free: usize, how: fn(u64) -> Located) {
        let item = self.free.remove(free);
        self.located[wanted] = Some(how(item.id));
    }
}

/// [`Located::PatternAndWear`], [`Located::Only`], and [`Located::NotFound`] when no new item is left.
fn place_by_game_coordinator(wanted: &[Wanted], new: &[GcItem]) -> Placed {
    let mut free = new.to_vec();
    free.sort_by_key(|item| item.id);
    let mut placed = Placed {
        located: wanted
            .iter()
            .map(|wanted| if wanted.cs2 { None } else { Some(Located::NotFound) })
            .collect(),
        free,
    };

    for (index, item) in wanted.iter().enumerate() {
        let fingerprint = match item.fingerprint {
            Some(fingerprint) if placed.located[index].is_none() => fingerprint,
            _ => continue,
        };
        // Identical copies (never seen for skins): which is which can't be told.
        if wanted
            .iter()
            .filter(|other| other.fingerprint == Some(fingerprint))
            .count()
            > 1
        {
            continue;
        }
        let mut matching = placed
            .free
            .iter()
            .enumerate()
            .filter(|(_, new)| fingerprint.matches(new))
            .map(|(free, _)| free);
        let only_match = match (matching.next(), matching.next()) {
            (Some(free), None) => Some(free),
            _ => None,
        };
        if let Some(free) = only_match {
            placed.take(index, free, Located::PatternAndWear);
        }
    }

    let open: Vec<usize> = placed.open().collect();
    if placed.free.is_empty() {
        for index in open {
            placed.located[index] = Some(Located::NotFound);
        }
    } else if let ([index], [new]) = (open.as_slice(), placed.free.as_slice()) {
        if wanted[*index].could_be(new) && !wanted[*index].kind_required {
            placed.take(*index, 0, Located::Only);
        }
    }
    placed
}

/// The new items whose kind (`classid`) [`locate`] needs, from our own web inventory: none when the game
/// coordinator placed every item.
pub(crate) fn ids_needing_kinds(wanted: &[Wanted], new: &[GcItem]) -> Vec<u64> {
    let placed = place_by_game_coordinator(wanted, new);
    let open: Vec<usize> = placed.open().collect();
    placed
        .free
        .iter()
        .filter(|item| open.iter().any(|&index| wanted[index].could_be(item)))
        .map(|item| item.id)
        .collect()
}

/// Where each of `wanted` (the received items, in the offer's order) went among `new` (the items the game
/// coordinator's inventory gained). `kinds` holds the `classid` of new items, for those [`ids_needing_kinds`] names
/// and our web inventory listed.
///
/// In order: [`Located::PatternAndWear`] and [`Located::Only`]; then by kind, [`Located::Kind`] for one received
/// item and one new item of a kind with no new item of unknown kind that could be it, else [`Located::Guessed`] in
/// the offer's order and the ids' order; then [`Located::Guessed`] among new items of unknown kind; the rest
/// [`Located::NotFound`]. An item with a fingerprint is only ever paired with a new item that matches it; one whose
/// kind is required, only with a new item of its kind (no [`Located::Only`], no guess among unknown kinds).
pub(crate) fn locate(wanted: &[Wanted], new: &[GcItem], kinds: &HashMap<u64, i64>) -> Vec<Located> {
    let mut placed = place_by_game_coordinator(wanted, new);

    let open: Vec<usize> = placed.open().collect();
    let mut classids: Vec<i64> = open.iter().map(|&index| wanted[index].classid).collect();
    classids.sort_unstable();
    classids.dedup();
    for classid in classids {
        let of_kind: Vec<usize> = placed
            .open()
            .filter(|&index| wanted[index].classid == classid)
            .collect();
        let unknown_could_be = |index: usize, placed: &Placed| {
            placed
                .free
                .iter()
                .any(|item| !kinds.contains_key(&item.id) && wanted[index].could_be(item))
        };
        let new_of_kind = |index: usize, placed: &Placed| -> Vec<usize> {
            placed
                .free
                .iter()
                .enumerate()
                .filter(|(_, item)| kinds.get(&item.id) == Some(&classid) && wanted[index].could_be(item))
                .map(|(free, _)| free)
                .collect()
        };

        if let [index] = of_kind.as_slice() {
            if let [free] = new_of_kind(*index, &placed).as_slice() {
                let how = if unknown_could_be(*index, &placed) {
                    Located::Guessed
                } else {
                    Located::Kind
                };
                placed.take(*index, *free, how);
                continue;
            }
        }
        for index in of_kind {
            if let Some(&free) = new_of_kind(index, &placed).first() {
                placed.take(index, free, Located::Guessed);
            }
        }
    }

    let open: Vec<usize> = placed.open().collect();
    for index in open {
        let unknown = placed.free.iter().position(|item| {
            !wanted[index].kind_required && !kinds.contains_key(&item.id) && wanted[index].could_be(item)
        });
        match unknown {
            Some(free) => placed.take(index, free, Located::Guessed),
            None => placed.located[index] = Some(Located::NotFound),
        }
    }

    placed
        .located
        .into_iter()
        .map(|located| located.unwrap_or(Located::NotFound))
        .collect()
}

/// How long to wait for the new items, and how often to look.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wait {
    pub(crate) timeout: Duration,
    pub(crate) poll: Duration,
    pub(crate) kind_read_interval: Duration,
}

impl Wait {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            poll: POLL_INTERVAL,
            kind_read_interval: KIND_READ_INTERVAL,
        }
    }
}

/// The items `source` gained over `before`, once there are `expected` of them or `timeout` passed: then those of the
/// last current inventory. While the inventory isn't current, waits for the game coordinator to send it again.
pub(crate) async fn wait_for_new_items<S: ItemSource>(
    source: &mut S,
    before: &HashSet<u64>,
    expected: usize,
    wait: Wait,
) -> Result<Vec<GcItem>, TradeError> {
    let deadline = Instant::now() + wait.timeout;
    let mut latest = Vec::new();
    loop {
        match source.items_not_in(before)? {
            Some(new) => latest = new,
            None => debug!("The game coordinator's inventory isn't current; waiting for it."),
        }
        if latest.len() >= expected || Instant::now() >= deadline {
            return Ok(latest);
        }
        Delay::new(wait.poll).await;
    }
}

/// The steps of `accept_offer_and_find_items` once the offer was looked up, each request given as a closure so that
/// tests can stand in for Steam: `partner_inventory(context)`, `accept()`, and `own_inventory()` (CS2's
/// trade-protected context).
pub(crate) async fn accept_and_locate<S, P, PF, A, AF, O, OF>(
    tradeoffer_id: u64,
    received: Vec<CEcon_Asset>,
    source: &mut S,
    mut partner_inventory: P,
    accept: A,
    mut own_inventory: O,
    wait: Wait,
) -> Result<AcceptedOffer, TradeError>
where
    S: ItemSource,
    P: FnMut(InventoryContext) -> PF,
    PF: Future<Output = Result<Inventory, TradeError>>,
    A: FnOnce() -> AF,
    AF: Future<Output = Result<Option<i64>, TradeError>>,
    O: FnMut() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    if source.item_ids()?.is_none() {
        return Err(TradeError::Cs2InventoryUnavailable);
    }

    let fingerprints = partner_fingerprints(&received, &mut partner_inventory).await;
    let wanted: Vec<Wanted> = received
        .iter()
        .map(|offered| {
            Wanted::new(
                offered,
                fingerprints.get(&offered.assetid).copied(),
                NewItems::OfTheTrade,
            )
        })
        .collect();

    // Right before the accept: an item that comes from elsewhere earlier isn't taken for a received one.
    let before = source.item_ids()?.ok_or(TradeError::Cs2InventoryUnavailable)?;
    let tradeid = accept().await?;

    let located = locate_after_accept(&wanted, &before, source, &mut own_inventory, wait)
        .await
        .map_err(|error| TradeError::AcceptedButItemsNotFound {
            tradeoffer_id,
            tradeid,
            source: Box::new(error),
        })?;

    Ok(AcceptedOffer {
        tradeid,
        received: received_items(received, located),
    })
}

/// Waits for the new items and places the received ones among them. Fails only if the game coordinator does: a web
/// inventory that can't be read is logged, and its items are [`Located::Guessed`].
async fn locate_after_accept<S, O, OF>(
    wanted: &[Wanted],
    before: &HashSet<u64>,
    source: &mut S,
    own_inventory: &mut O,
    wait: Wait,
) -> Result<Vec<Located>, TradeError>
where
    S: ItemSource,
    O: FnMut() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let expected = wanted.iter().filter(|wanted| wanted.cs2).count();
    let new = if expected == 0 {
        Vec::new()
    } else {
        wait_for_new_items(source, before, expected, wait).await?
    };
    let reads = [InventoryContext::CS2_TRADE_PROTECTED; KIND_READS];
    Ok(place(wanted, &new, &mut |_| own_inventory(), &reads, wait.kind_read_interval).await)
}

/// The steps of `find_received_items`: where `received` (an accepted offer's items to receive) went among the items
/// of `source` not in `known`, looking at it once, with the partner's `fingerprints` by asset id.
/// `own_inventory(context)` reads one of our CS2 contexts, each once at most.
///
/// Fails with [`TradeError::Cs2InventoryUnavailable`] while the inventory isn't current, unless no CS2 item is to be
/// found; otherwise only if the game coordinator does.
pub(crate) async fn find_and_locate<S, O, OF>(
    received: Vec<CEcon_Asset>,
    known: &HashSet<u64>,
    fingerprints: &HashMap<u64, Fingerprint>,
    source: &mut S,
    mut own_inventory: O,
) -> Result<Vec<ReceivedItem>, TradeError>
where
    S: ItemSource,
    O: FnMut(InventoryContext) -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let wanted: Vec<Wanted> = received
        .iter()
        .map(|offered| {
            let fingerprint = u64::try_from(offered.assetid)
                .ok()
                .and_then(|assetid| fingerprints.get(&assetid));
            Wanted::new(offered, fingerprint.copied(), NewItems::MaybeOthers)
        })
        .collect();
    let new = if wanted.iter().any(|wanted| wanted.cs2) {
        source.items_not_in(known)?.ok_or(TradeError::Cs2InventoryUnavailable)?
    } else {
        Vec::new()
    };
    let located = place(
        &wanted,
        &new,
        &mut own_inventory,
        &PARTNER_ACCEPT_KIND_READS,
        Duration::ZERO,
    )
    .await;
    Ok(received_items(received, located))
}

/// [`locate`], with the kinds it needs read from our own web inventory: in `kind_reads`, one context per read.
async fn place<O, OF>(
    wanted: &[Wanted],
    new: &[GcItem],
    own_inventory: &mut O,
    kind_reads: &[InventoryContext],
    kind_read_interval: Duration,
) -> Vec<Located>
where
    O: FnMut(InventoryContext) -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let needing = ids_needing_kinds(wanted, new);
    let kinds = if needing.is_empty() {
        HashMap::new()
    } else {
        kinds_of(&needing, own_inventory, kind_reads, kind_read_interval).await
    };
    locate(wanted, new, &kinds)
}

/// Each of `received` with where it went.
fn received_items(received: Vec<CEcon_Asset>, located: Vec<Located>) -> Vec<ReceivedItem> {
    received
        .into_iter()
        .zip(located)
        .map(|(offered, located)| ReceivedItem { offered, located })
        .collect()
}

/// The pattern and wear of the partner's copies, by the partner's asset id: only for items whose kind repeats among
/// the received ones, from one read per context. A failed read is logged: its items get none.
async fn partner_fingerprints<P, PF>(received: &[CEcon_Asset], partner_inventory: &mut P) -> HashMap<i64, Fingerprint>
where
    P: FnMut(InventoryContext) -> PF,
    PF: Future<Output = Result<Inventory, TradeError>>,
{
    let cs2: Vec<&CEcon_Asset> = received
        .iter()
        .filter(|offered| offered.appid == i64::from(InventoryContext::CS2.appid()))
        .collect();
    let repeated: Vec<&CEcon_Asset> = cs2
        .iter()
        .copied()
        .filter(|offered| cs2.iter().filter(|other| other.classid == offered.classid).count() > 1)
        .collect();
    let mut contexts: Vec<i64> = repeated.iter().map(|offered| offered.contextid).collect();
    contexts.sort_unstable();
    contexts.dedup();

    let mut fingerprints = HashMap::new();
    for contextid in contexts {
        let context = match u32::try_from(contextid) {
            Ok(contextid) => InventoryContext::new(InventoryContext::CS2.appid(), contextid),
            Err(_) => continue,
        };
        let inventory = match partner_inventory(context).await {
            Ok(inventory) => inventory,
            Err(error) => {
                warn!(
                    "Could not read the partner's inventory (context {}), telling received items apart without it: {}",
                    contextid, error
                );
                continue;
            }
        };
        for page in inventory.pages() {
            for item in page.items() {
                let wanted = repeated
                    .iter()
                    .any(|offered| offered.assetid == item.assetid && offered.contextid == contextid);
                if let (true, Some(fingerprint)) = (wanted, Fingerprint::of(&item)) {
                    fingerprints.insert(item.assetid, fingerprint);
                }
            }
        }
    }
    fingerprints
}

/// The `classid` of the items `ids` from our own web inventory, read in each context of `reads` in turn while some are
/// missing (Steam serves it from a cache, and a context may not hold them). A failed read is logged and ends the reads.
async fn kinds_of<O, OF>(
    ids: &[u64],
    own_inventory: &mut O,
    reads: &[InventoryContext],
    interval: Duration,
) -> HashMap<u64, i64>
where
    O: FnMut(InventoryContext) -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let mut kinds = HashMap::new();
    for (read, &context) in (1..).zip(reads) {
        if read > 1 {
            Delay::new(interval).await;
        }
        match own_inventory(context).await {
            Ok(inventory) => {
                for page in inventory.pages() {
                    for item in page.items() {
                        if let Ok(id) = u64::try_from(item.assetid) {
                            if ids.contains(&id) {
                                kinds.insert(id, item.classid);
                            }
                        }
                    }
                }
            }
            Err(error) => {
                warn!(
                    "Could not read our inventory for the kinds of the received items: {}",
                    error
                );
                break;
            }
        }
        if kinds.len() == ids.len() {
            break;
        }
        debug!(
            "Our web inventory lists {} of {} new items (read {} of {}).",
            kinds.len(),
            ids.len(),
            read,
            reads.len()
        );
    }
    kinds
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;

    use futures::executor::block_on;
    use futures::future::ready;

    use super::*;

    const AUG: i64 = 241575744;
    const CASE: i64 = 4717330486;
    const MP7: i64 = 7993036706;

    fn fingerprint(pattern: i64, wear: f32) -> Option<Fingerprint> {
        Some(Fingerprint {
            pattern,
            wear_bits: wear.to_bits(),
        })
    }

    fn wanted(classid: i64, fingerprint: Option<Fingerprint>) -> Wanted {
        Wanted {
            classid,
            cs2: true,
            fingerprint,
            kind_required: false,
        }
    }

    fn skin(id: u64, pattern: u32, float: f32) -> GcItem {
        GcItem {
            id,
            pattern: Some(pattern),
            float: Some(float),
        }
    }

    fn commodity(id: u64) -> GcItem {
        GcItem {
            id,
            pattern: None,
            float: None,
        }
    }

    fn no_kinds() -> HashMap<u64, i64> {
        HashMap::new()
    }

    fn kinds(kinds: &[(u64, i64)]) -> HashMap<u64, i64> {
        kinds.iter().copied().collect()
    }

    #[test]
    fn only() {
        let wanted = [wanted(AUG, None)];
        let new = [skin(54238728503, 302, 0.372_153_76)];
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(locate(&wanted, &new, &no_kinds()), vec![Located::Only(54238728503)]);
    }

    #[test]
    fn only_is_not_an_item_that_contradicts_the_fingerprint() {
        let wanted = [wanted(AUG, fingerprint(302, 0.372_153_76))];
        let elsewhere = [skin(9, 1, 0.5)];
        assert!(ids_needing_kinds(&wanted, &elsewhere).is_empty());
        assert_eq!(locate(&wanted, &elsewhere, &no_kinds()), vec![Located::NotFound]);
    }

    #[test]
    fn web_wear_matches_the_game_coordinator_float_bit_for_bit() {
        // B's AUG, live 2026-10-21: the web's "0.372153759002685547" and the game coordinator's 0x3ebe8af0
        let fingerprint = Fingerprint {
            pattern: 302,
            wear_bits: ("0.372153759002685547".parse::<f64>().unwrap() as f32).to_bits(),
        };
        assert!(fingerprint.matches(&skin(1, 302, f32::from_bits(0x3ebe_8af0))));
        assert!(!fingerprint.matches(&skin(1, 303, f32::from_bits(0x3ebe_8af0))));
        assert!(!fingerprint.matches(&skin(1, 302, f32::from_bits(0x3ebe_8af1))));
        assert!(!fingerprint.matches(&commodity(1)));
    }

    #[test]
    fn pattern_and_wear() {
        let wanted = [
            wanted(MP7, fingerprint(355, 0.066_143_81)),
            wanted(MP7, fingerprint(12, 0.2)),
        ];
        let new = [skin(20, 12, 0.2), skin(10, 355, 0.066_143_81)];
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::PatternAndWear(10), Located::PatternAndWear(20)]
        );
    }

    #[test]
    fn pattern_and_wear_ignores_items_from_elsewhere() {
        let wanted = [
            wanted(MP7, fingerprint(355, 0.066_143_81)),
            wanted(MP7, fingerprint(12, 0.2)),
        ];
        let new = [
            skin(5, 1, 0.9),
            skin(20, 12, 0.2),
            skin(10, 355, 0.066_143_81),
            commodity(7),
        ];
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::PatternAndWear(10), Located::PatternAndWear(20)]
        );
    }

    #[test]
    fn identical_copies_are_guessed() {
        let wanted = [wanted(MP7, fingerprint(1, 0.5)), wanted(MP7, fingerprint(1, 0.5))];
        let new = [skin(30, 1, 0.5), skin(20, 1, 0.5)];
        assert_eq!(ids_needing_kinds(&wanted, &new), vec![20, 30]);
        let kinds = kinds(&[(20, MP7), (30, MP7)]);
        assert_eq!(
            locate(&wanted, &new, &kinds),
            vec![Located::Guessed(20), Located::Guessed(30)]
        );
    }

    #[test]
    fn kind() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        let new = [commodity(2), skin(1, 302, 0.37)];
        assert_eq!(ids_needing_kinds(&wanted, &new), vec![1, 2]);
        let kinds = kinds(&[(1, AUG), (2, CASE)]);
        assert_eq!(locate(&wanted, &new, &kinds), vec![Located::Kind(1), Located::Kind(2)]);
    }

    #[test]
    fn kind_ignores_items_of_other_kinds_from_elsewhere() {
        let wanted = [wanted(AUG, None)];
        let new = [skin(1, 302, 0.37), skin(2, 7, 0.1)];
        assert_eq!(ids_needing_kinds(&wanted, &new), vec![1, 2]);
        let kinds = kinds(&[(1, AUG), (2, MP7)]);
        assert_eq!(locate(&wanted, &new, &kinds), vec![Located::Kind(1)]);
    }

    #[test]
    fn kind_is_guessed_while_an_item_of_unknown_kind_could_be_it() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        let new = [skin(1, 302, 0.37), commodity(2), commodity(3)];
        // 3 isn't listed (yet) on the web
        let kinds = kinds(&[(1, AUG), (2, CASE)]);
        assert_eq!(
            locate(&wanted, &new, &kinds),
            vec![Located::Guessed(1), Located::Guessed(2)]
        );
    }

    #[test]
    fn commodities_of_one_kind_are_guessed() {
        let wanted = [wanted(CASE, None), wanted(CASE, None), wanted(AUG, None)];
        let new = [commodity(12), commodity(11), skin(13, 302, 0.37)];
        let kinds = kinds(&[(11, CASE), (12, CASE), (13, AUG)]);
        assert_eq!(
            locate(&wanted, &new, &kinds),
            vec![Located::Guessed(11), Located::Guessed(12), Located::Kind(13)]
        );
    }

    #[test]
    fn a_tie_of_one_received_and_two_new_of_its_kind_is_guessed() {
        let wanted = [wanted(CASE, None), wanted(AUG, None)];
        // one case came from elsewhere
        let new = [commodity(11), commodity(12), skin(13, 302, 0.37)];
        let kinds = kinds(&[(11, CASE), (12, CASE), (13, AUG)]);
        assert_eq!(
            locate(&wanted, &new, &kinds),
            vec![Located::Guessed(11), Located::Kind(13)]
        );
    }

    #[test]
    fn a_guess_among_unknown_kinds_skips_items_of_other_kinds() {
        let wanted = [wanted(AUG, None)];
        let new = [skin(1, 7, 0.1), skin(2, 302, 0.37)];
        let kinds = kinds(&[(1, MP7)]);
        assert_eq!(locate(&wanted, &new, &kinds), vec![Located::Guessed(2)]);
    }

    #[test]
    fn unknown_kinds_are_guessed() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        let new = [skin(1, 302, 0.37), commodity(2)];
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Guessed(1), Located::Guessed(2)]
        );
    }

    #[test]
    fn not_found() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        assert_eq!(
            locate(&wanted, &[], &no_kinds()),
            vec![Located::NotFound, Located::NotFound]
        );
        assert!(ids_needing_kinds(&wanted, &[]).is_empty());

        // one of two arrived
        let new = [commodity(2)];
        let kinds = kinds(&[(2, CASE)]);
        assert_eq!(locate(&wanted, &new, &kinds), vec![Located::NotFound, Located::Kind(2)]);
    }

    #[test]
    fn other_games_are_not_found_and_not_waited_for() {
        let mut gem = wanted(667924416, None);
        gem.cs2 = false;
        let wanted = [gem, wanted(AUG, None)];
        let new = [skin(1, 302, 0.37)];
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::NotFound, Located::Only(1)]
        );
    }

    /// A game coordinator inventory that changes on each look: the next state of `states`, the last one staying.
    struct Script {
        before: HashSet<u64>,
        states: VecDeque<Option<Vec<GcItem>>>,
        looks: usize,
    }

    impl Script {
        fn new(before: &[u64], states: Vec<Option<Vec<GcItem>>>) -> Self {
            Self {
                before: before.iter().copied().collect(),
                states: states.into(),
                looks: 0,
            }
        }

        fn state(&mut self) -> Option<Vec<GcItem>> {
            self.looks += 1;
            if self.states.len() > 1 {
                self.states.pop_front().unwrap()
            } else {
                self.states.front().cloned().unwrap()
            }
        }
    }

    impl ItemSource for Script {
        fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError> {
            Ok(self.state().map(|_| self.before.clone()))
        }

        fn items_not_in(&mut self, known: &HashSet<u64>) -> Result<Option<Vec<GcItem>>, TradeError> {
            assert_eq!(known, &self.before);
            Ok(self.state())
        }
    }

    fn fast(timeout: Duration) -> Wait {
        Wait {
            timeout,
            poll: Duration::from_millis(1),
            kind_read_interval: Duration::from_millis(1),
        }
    }

    #[test]
    fn waits_through_an_unavailable_inventory() {
        let mut source = Script::new(
            &[],
            vec![
                Some(vec![]),
                None,
                None,
                Some(vec![commodity(1)]),
                Some(vec![commodity(1), commodity(2)]),
            ],
        );
        let before = HashSet::new();
        let new = block_on(wait_for_new_items(
            &mut source,
            &before,
            2,
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(new, vec![commodity(1), commodity(2)]);
        assert_eq!(source.looks, 5);
    }

    #[test]
    fn times_out_with_what_came() {
        let mut source = Script::new(&[], vec![Some(vec![commodity(1)]), None]);
        let before = HashSet::new();
        let started = Instant::now();
        let new = block_on(wait_for_new_items(
            &mut source,
            &before,
            2,
            fast(Duration::from_millis(50)),
        ))
        .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(50));
        // the last current inventory
        assert_eq!(new, vec![commodity(1)]);
    }

    fn asset(context: InventoryContext, assetid: i64, classid: i64) -> CEcon_Asset {
        serde_json::from_value(serde_json::json!({
            "appid": context.appid(), "contextid": context.contextid().to_string(), "assetid": assetid.to_string(),
            "classid": classid.to_string(), "instanceid": "0", "amount": "1"
        }))
        .unwrap()
    }

    /// An inventory page with skins `(assetid, classid, pattern, wear)`.
    fn web_inventory(context: InventoryContext, skins: &[(i64, i64, i64, &str)]) -> Inventory {
        let assets: Vec<String> = skins
            .iter()
            .map(|(assetid, classid, _, _)| {
                format!(
                    r#"{{"appid":{},"contextid":"{}","assetid":"{}","classid":"{}","instanceid":"0","amount":"1"}}"#,
                    context.appid(),
                    context.contextid(),
                    assetid,
                    classid
                )
            })
            .collect();
        let properties: Vec<String> = skins
            .iter()
            .map(|(assetid, _, pattern, wear)| {
                format!(
                    r#"{{"appid":{},"contextid":"{}","assetid":"{}","asset_properties":[
                      {{"propertyid":1,"int_value":"{}","name":"Pattern Template"}},
                      {{"propertyid":2,"float_value":"{}","name":"Wear Rating"}}]}}"#,
                    context.appid(),
                    context.contextid(),
                    assetid,
                    pattern,
                    wear
                )
            })
            .collect();
        Inventory::from_json(format!(
            r#"{{"assets":[{}],"asset_properties":[{}],"success":1}}"#,
            assets.join(","),
            properties.join(",")
        ))
        .unwrap()
    }

    fn no_inventory() -> impl Future<Output = Result<Inventory, TradeError>> {
        ready(Err(TradeError::GeneralError("not expected".to_string())))
    }

    #[test]
    fn nothing_is_accepted_without_a_current_inventory() {
        let mut source = Script::new(&[1], vec![None]);
        let accepted = Cell::new(false);
        let result = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &mut source,
            |_| no_inventory(),
            || {
                accepted.set(true);
                ready(Ok(Some(42)))
            },
            no_inventory,
            fast(Duration::from_secs(1)),
        ));
        assert!(
            matches!(result, Err(TradeError::Cs2InventoryUnavailable)),
            "{:?}",
            result
        );
        assert!(!accepted.get());
    }

    #[test]
    fn one_item_no_web_read() {
        let mut source = Script::new(
            &[1],
            vec![Some(vec![]), Some(vec![]), Some(vec![skin(54238728503, 302, 0.37)])],
        );
        let reads = Cell::new(0);
        let accepted = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 24234602605, AUG)],
            &mut source,
            |_| {
                reads.set(reads.get() + 1);
                no_inventory()
            },
            || ready(Ok(Some(42))),
            || {
                reads.set(reads.get() + 1);
                no_inventory()
            },
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(accepted.tradeid, Some(42));
        assert_eq!(accepted.received.len(), 1);
        assert_eq!(accepted.received[0].offered.assetid, 24234602605);
        assert_eq!(accepted.received[0].located, Located::Only(54238728503));
        assert_eq!(reads.get(), 0);
    }

    #[test]
    fn repeated_kind_reads_the_partner_once_per_context() {
        let new = vec![skin(901, 12, 0.2), skin(902, 355, 0.066_143_81)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(vec![]), Some(new)]);
        let contexts = std::cell::RefCell::new(Vec::new());
        let accepted = block_on(accept_and_locate(
            7,
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, MP7),
            ],
            &mut source,
            |context| {
                contexts.borrow_mut().push(context);
                ready(Ok(web_inventory(
                    InventoryContext::CS2,
                    &[
                        (100, MP7, 355, "0.0661438107490539551"),
                        (101, MP7, 12, "0.200000002980232239"),
                    ],
                )))
            },
            || ready(Ok(None)),
            no_inventory,
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(contexts.into_inner(), vec![InventoryContext::CS2]);
        assert_eq!(accepted.tradeid, None);
        let located: Vec<Located> = accepted.received.iter().map(|item| item.located).collect();
        assert_eq!(
            located,
            vec![Located::PatternAndWear(902), Located::PatternAndWear(901)]
        );
    }

    #[test]
    fn a_private_partner_inventory_is_not_an_error() {
        let new = vec![skin(901, 12, 0.2), skin(902, 355, 0.066_143_81)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(vec![]), Some(new)]);
        let accepted = block_on(accept_and_locate(
            7,
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, MP7),
            ],
            &mut source,
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            || {
                ready(Ok(web_inventory(
                    InventoryContext::CS2_TRADE_PROTECTED,
                    &[(901, MP7, 12, "0.2"), (902, MP7, 355, "0.06")],
                )))
            },
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        let located: Vec<Located> = accepted.received.iter().map(|item| item.located).collect();
        assert_eq!(located, vec![Located::Guessed(901), Located::Guessed(902)]);
    }

    #[test]
    fn a_stale_web_inventory_is_read_again() {
        let new = vec![skin(901, 302, 0.37), commodity(902)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(vec![]), Some(new)]);
        let reads = Cell::new(0);
        let accepted = block_on(accept_and_locate(
            7,
            vec![
                asset(InventoryContext::CS2, 100, AUG),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &mut source,
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            || {
                reads.set(reads.get() + 1);
                let body = if reads.get() < 2 {
                    // Steam's cache, from before the trade
                    r#"{"assets":[],"success":1}"#.to_string()
                } else {
                    format!(
                        r#"{{"assets":[
                          {{"appid":730,"contextid":"16","assetid":"901","classid":"{}","amount":"1"}},
                          {{"appid":730,"contextid":"16","assetid":"902","classid":"{}","amount":"1"}}],"success":1}}"#,
                        AUG, CASE
                    )
                };
                ready(Inventory::from_json(body))
            },
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(reads.get(), 2);
        let located: Vec<Located> = accepted.received.iter().map(|item| item.located).collect();
        assert_eq!(located, vec![Located::Kind(901), Located::Kind(902)]);
    }

    #[test]
    fn web_reads_stop_after_three() {
        let new = vec![skin(901, 302, 0.37), commodity(902)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(vec![]), Some(new)]);
        let reads = Cell::new(0);
        let accepted = block_on(accept_and_locate(
            7,
            vec![
                asset(InventoryContext::CS2, 100, AUG),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &mut source,
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            || {
                reads.set(reads.get() + 1);
                ready(Inventory::from_json(r#"{"assets":[],"success":1}"#.to_string()))
            },
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(reads.get(), KIND_READS);
        let located: Vec<Located> = accepted.received.iter().map(|item| item.located).collect();
        assert_eq!(located, vec![Located::Guessed(901), Located::Guessed(902)]);
    }

    #[test]
    fn a_timeout_after_the_accept_keeps_the_trade() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let accepted = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &mut source,
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            no_inventory,
            fast(Duration::from_millis(20)),
        ))
        .unwrap();
        assert_eq!(accepted.tradeid, Some(42));
        assert_eq!(accepted.received[0].located, Located::NotFound);
    }

    /// Fails after the accept, as a stopped client does.
    struct FailsAfter(usize);

    impl ItemSource for FailsAfter {
        fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError> {
            Ok(Some(HashSet::new()))
        }

        fn items_not_in(&mut self, _: &HashSet<u64>) -> Result<Option<Vec<GcItem>>, TradeError> {
            if self.0 == 0 {
                return Err(TradeError::GeneralError("the client stopped".to_string()));
            }
            self.0 -= 1;
            Ok(Some(Vec::new()))
        }
    }

    #[test]
    fn a_failure_after_the_accept_carries_the_trade_id() {
        let result = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &mut FailsAfter(2),
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            no_inventory,
            fast(Duration::from_secs(10)),
        ));
        match result {
            Err(TradeError::AcceptedButItemsNotFound {
                tradeoffer_id: 7,
                tradeid: Some(42),
                source,
            }) => assert!(matches!(*source, TradeError::GeneralError(_))),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn a_failed_accept_is_plain() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let result = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &mut source,
            |_| no_inventory(),
            || ready(Err(TradeError::GeneralError("refused".to_string()))),
            no_inventory,
            fast(Duration::from_secs(10)),
        ));
        assert!(matches!(result, Err(TradeError::GeneralError(_))), "{:?}", result);
    }

    /// The game coordinator's inventory as it is now (`None`: not current), holding every item, the known ones too.
    struct Now {
        items: Option<Vec<GcItem>>,
        looks: usize,
    }

    impl Now {
        fn current(items: Vec<GcItem>) -> Self {
            Self {
                items: Some(items),
                looks: 0,
            }
        }
    }

    impl ItemSource for Now {
        fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError> {
            unreachable!("no snapshot once the partner accepted")
        }

        fn items_not_in(&mut self, known: &HashSet<u64>) -> Result<Option<Vec<GcItem>>, TradeError> {
            self.looks += 1;
            Ok(self
                .items
                .as_ref()
                .map(|items| items.iter().filter(|item| !known.contains(&item.id)).copied().collect()))
        }
    }

    /// `find_and_locate` with `known` ids and `fingerprints`, with the contexts of our own web inventory read.
    fn find<S: ItemSource>(
        received: Vec<CEcon_Asset>,
        known: &[u64],
        fingerprints: &[(u64, Fingerprint)],
        source: &mut S,
        mut own_inventory: impl FnMut(InventoryContext) -> Result<Inventory, TradeError>,
    ) -> (Result<Vec<ReceivedItem>, TradeError>, Vec<InventoryContext>) {
        let known: HashSet<u64> = known.iter().copied().collect();
        let fingerprints: HashMap<u64, Fingerprint> = fingerprints.iter().copied().collect();
        let reads = std::cell::RefCell::new(Vec::new());
        let result = block_on(find_and_locate(received, &known, &fingerprints, source, |context| {
            reads.borrow_mut().push(context);
            ready(own_inventory(context))
        }));
        (result, reads.into_inner())
    }

    fn located(received: Vec<ReceivedItem>) -> Vec<Located> {
        received.iter().map(|item| item.located).collect()
    }

    /// Our `context` listing `(assetid, classid)`.
    fn kinds_page(context: InventoryContext, items: &[(u64, i64)]) -> Result<Inventory, TradeError> {
        let assets: Vec<String> = items
            .iter()
            .map(|(assetid, classid)| {
                format!(
                    r#"{{"appid":730,"contextid":"{}","assetid":"{}","classid":"{}","amount":"1"}}"#,
                    context.contextid(),
                    assetid,
                    classid
                )
            })
            .collect();
        Inventory::from_json(format!(r#"{{"assets":[{}],"success":1}}"#, assets.join(",")))
    }

    /// Context 16 listing `items`, context 2 nothing.
    fn protected(items: &[(u64, i64)]) -> impl FnMut(InventoryContext) -> Result<Inventory, TradeError> + '_ {
        move |context| match context {
            InventoryContext::CS2_TRADE_PROTECTED => kinds_page(context, items),
            _ => kinds_page(context, &[]),
        }
    }

    fn no_web_read(_: InventoryContext) -> Result<Inventory, TradeError> {
        panic!("no web read")
    }

    const PROTECTED_THEN_TRADABLE: [InventoryContext; 2] =
        [InventoryContext::CS2_TRADE_PROTECTED, InventoryContext::CS2];

    fn web_fingerprint(pattern: i64, wear: &str) -> Fingerprint {
        Fingerprint::new(pattern, (wear.parse::<f64>().unwrap() as f32).to_bits())
    }

    #[test]
    fn fingerprint_of_a_web_item() {
        let inventory = web_inventory(
            InventoryContext::CS2_TRADE_PROTECTED,
            &[(54238728503, AUG, 302, "0.372153759002685547")],
        );
        let page = inventory.pages().next().unwrap();
        let fingerprint = Fingerprint::of(&page.items().next().unwrap()).unwrap();
        assert_eq!(fingerprint, Fingerprint::new(302, 0x3ebe_8af0));
        assert_eq!((fingerprint.pattern(), fingerprint.wear_bits()), (302, 0x3ebe_8af0));

        let case = kinds_page(InventoryContext::CS2, &[(1, CASE)]).unwrap();
        let page = case.pages().next().unwrap();
        assert_eq!(Fingerprint::of(&page.items().next().unwrap()), None);
    }

    #[test]
    fn partner_accepted_one_item_of_its_kind() {
        let mut source = Now::current(vec![skin(1, 7, 0.1), commodity(2), skin(54238728503, 302, 0.37)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 24234602605, AUG)],
            &[1, 2],
            &[],
            &mut source,
            protected(&[(54238728503, AUG)]),
        );
        let received = result.unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].offered.assetid, 24234602605);
        assert_eq!(received[0].located, Located::Kind(54238728503));
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
        assert_eq!(source.looks, 1);
    }

    #[test]
    fn partner_accepted_one_item_of_unread_kind_is_not_found() {
        // without a fingerprint, the only new item may have come another way
        let mut source = Now::current(vec![skin(54238728503, 302, 0.37)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 24234602605, AUG)],
            &[],
            &[],
            &mut source,
            protected(&[]),
        );
        assert_eq!(located(result.unwrap()), vec![Located::NotFound]);
        assert_eq!(reads, PROTECTED_THEN_TRADABLE);
    }

    #[test]
    fn partner_accepted_a_new_item_of_another_kind_is_not_taken() {
        let mut source = Now::current(vec![skin(5, 7, 0.1)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &[],
            &mut source,
            protected(&[(5, MP7)]),
        );
        assert_eq!(located(result.unwrap()), vec![Located::NotFound]);
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
    }

    #[test]
    fn partner_accepted_kind_from_the_tradable_context() {
        // its protection ended before we looked
        let mut source = Now::current(vec![skin(7, 302, 0.37)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &[],
            &mut source,
            |context| match context {
                InventoryContext::CS2 => kinds_page(context, &[(7, AUG)]),
                _ => kinds_page(context, &[]),
            },
        );
        assert_eq!(located(result.unwrap()), vec![Located::Kind(7)]);
        assert_eq!(reads, PROTECTED_THEN_TRADABLE);
    }

    #[test]
    fn partner_accepted_guesses_only_among_its_kind() {
        // 2 isn't listed (yet) on the web: it may be the AUG, but it isn't taken for it
        let mut source = Now::current(vec![skin(1, 302, 0.37), skin(2, 303, 0.4)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &[],
            &mut source,
            protected(&[(1, AUG)]),
        );
        assert_eq!(located(result.unwrap()), vec![Located::Guessed(1)]);
        assert_eq!(reads, PROTECTED_THEN_TRADABLE);
    }

    #[test]
    fn partner_accepted_copies_of_one_kind_told_apart_by_fingerprints() {
        let mut source = Now::current(vec![skin(1, 7, 0.1), skin(901, 12, 0.2), skin(902, 355, 0.066_143_81)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, MP7),
            ],
            &[1],
            &[
                (100, web_fingerprint(355, "0.0661438107490539551")),
                (101, web_fingerprint(12, "0.200000002980232239")),
            ],
            &mut source,
            no_web_read,
        );
        assert_eq!(
            located(result.unwrap()),
            vec![Located::PatternAndWear(902), Located::PatternAndWear(901)]
        );
        assert!(reads.is_empty());
    }

    #[test]
    fn partner_accepted_copies_of_one_kind_not_told_apart_are_guessed() {
        let mut source = Now::current(vec![commodity(12), commodity(11)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, CASE),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[],
            &[],
            &mut source,
            protected(&[(11, CASE), (12, CASE)]),
        );
        assert_eq!(
            located(result.unwrap()),
            vec![Located::Guessed(11), Located::Guessed(12)]
        );
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
    }

    #[test]
    fn partner_accepted_kinds() {
        let mut source = Now::current(vec![skin(901, 302, 0.37), commodity(902)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, AUG),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[],
            &[],
            &mut source,
            protected(&[(901, AUG), (902, CASE)]),
        );
        assert_eq!(located(result.unwrap()), vec![Located::Kind(901), Located::Kind(902)]);
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
    }

    #[test]
    fn partner_accepted_with_an_unreadable_web_inventory_is_not_found() {
        let mut source = Now::current(vec![commodity(12), commodity(11)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, CASE),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[],
            &[],
            &mut source,
            |_| Err(TradeError::GeneralError("HTTP 429".to_string())),
        );
        assert_eq!(located(result.unwrap()), vec![Located::NotFound, Located::NotFound]);
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
    }

    #[test]
    fn partner_accepted_fingerprinted_items_keep_their_guess() {
        // two new copies match the MP7; the case has no fingerprint and no new item read to be a case
        let mut source = Now::current(vec![
            skin(5, 355, 0.066_143_81),
            skin(6, 355, 0.066_143_81),
            commodity(7),
        ]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[],
            &[(100, web_fingerprint(355, "0.0661438107490539551"))],
            &mut source,
            protected(&[]),
        );
        assert_eq!(located(result.unwrap()), vec![Located::Guessed(5), Located::NotFound]);
        assert_eq!(reads, PROTECTED_THEN_TRADABLE);
    }

    #[test]
    fn partner_accepted_missing_is_not_found() {
        let mut source = Now::current(vec![skin(1, 302, 0.37)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[1],
            &[],
            &mut source,
            no_web_read,
        );
        assert_eq!(located(result.unwrap()), vec![Located::NotFound]);
        assert!(reads.is_empty());
    }

    #[test]
    fn partner_accepted_known_ids_are_not_new() {
        // 5 is a copy the bot already holds, with the same pattern and wear
        let mut source = Now::current(vec![skin(5, 355, 0.066_143_81), skin(6, 355, 0.066_143_81)]);
        let (result, _) = find(
            vec![asset(InventoryContext::CS2, 100, MP7)],
            &[5],
            &[(100, web_fingerprint(355, "0.0661438107490539551"))],
            &mut source,
            no_web_read,
        );
        assert_eq!(located(result.unwrap()), vec![Located::PatternAndWear(6)]);
    }

    #[test]
    fn partner_accepted_without_a_fingerprint_for_one_item() {
        let mut source = Now::current(vec![skin(20, 12, 0.2), skin(10, 355, 0.066_143_81)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, MP7),
            ],
            &[],
            &[(100, web_fingerprint(355, "0.0661438107490539551"))],
            &mut source,
            protected(&[(20, MP7)]),
        );
        // the one left is no `Only`: its kind is read
        assert_eq!(
            located(result.unwrap()),
            vec![Located::PatternAndWear(10), Located::Kind(20)]
        );
        assert_eq!(reads, vec![InventoryContext::CS2_TRADE_PROTECTED]);
    }

    #[test]
    fn partner_accepted_without_a_current_inventory() {
        let mut source = Now { items: None, looks: 0 };
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &[],
            &mut source,
            no_web_read,
        );
        assert!(
            matches!(result, Err(TradeError::Cs2InventoryUnavailable)),
            "{:?}",
            result
        );
        assert!(reads.is_empty());
        assert_eq!(source.looks, 1);
    }

    #[test]
    fn partner_accepted_other_games_need_no_look() {
        let mut source = Now { items: None, looks: 0 };
        let (result, _) = find(
            vec![asset(InventoryContext::STEAM, 100, 667924416)],
            &[],
            &[],
            &mut source,
            no_web_read,
        );
        assert_eq!(located(result.unwrap()), vec![Located::NotFound]);
        let (result, _) = find(Vec::new(), &[], &[], &mut source, no_web_read);
        assert!(result.unwrap().is_empty());
        assert_eq!(source.looks, 0);
    }

    #[test]
    fn partner_accepted_game_coordinator_failure() {
        let (result, _) = find(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &[],
            &mut FailsAfter(0),
            no_web_read,
        );
        assert!(matches!(result, Err(TradeError::GeneralError(_))), "{:?}", result);
    }

    #[test]
    fn a_gift_waits_for_nothing() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let accepted = block_on(accept_and_locate(
            7,
            Vec::new(),
            &mut source,
            |_| no_inventory(),
            || ready(Ok(Some(42))),
            no_inventory,
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert!(accepted.received.is_empty());
        // before the accept only
        assert_eq!(source.looks, 2);
    }
}

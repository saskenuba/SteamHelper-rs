//! Where the items of an accepted offer went, found through CS2's game coordinator (feature `cs2`).
//!
//! Since ~2026-04 Steam's trade history has no `new_assetid` for CS2 trades. The item already has its new id right
//! after the accept, and the game coordinator's item id is the web asset id (seen live 2026-10-20/21), so the new ids
//! are those the game coordinator's inventory gained. Telling which received item is which comes from what a trade
//! keeps: the pattern and wear (the web prints the game coordinator's 32-bit float in full) and the kind (`classid`).
//! The pattern and wear come from the caller, read from the partner's inventory before the trade: offers and trade
//! history carry none (seen live 2026-10-22). Spec: `docs/specs/2026-10-21-received-items.md`.
//!
//! Two ways in, both placing items with [`locate`]: [`accept_and_locate`] notes the game coordinator's item ids,
//! accepts, and waits for new ones; [`find_and_locate`], for an offer the partner accepted earlier, looks once at the
//! items beyond the ids the caller knows.

use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;

use futures_timer::Delay;
use serde::Deserialize;
use serde::Serialize;
use steam_client::gc::cs2::Cs2;
use tappet::response_types::CEcon_Asset;
use tracing::debug;

use crate::Inventory;
use crate::InventoryContext;
use crate::InventoryItem;
use crate::TradeError;

/// How long to wait for the received items by default.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// How often the game coordinator's inventory is looked at while waiting. Local state: no request.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

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
    /// The game coordinator's item ids right before the accept. Pass them to
    /// [`SteamTradeManager::find_received_items`](crate::SteamTradeManager::find_received_items) as `known_ids` to look
    /// again for items [`Located::NotFound`], with the new ids other trades were found to bring since: else two offers
    /// with the same commodity can be given the same new id.
    pub known_ids: HashSet<u64>,
}

/// An item received, and where it went.
#[derive(Debug, Clone)]
pub struct ReceivedItem {
    /// The item as the offer listed it: its `assetid` is the partner's, from before the trade.
    pub offered: CEcon_Asset,
    pub located: Located,
}

/// Where a received item went in the game coordinator's inventory, with its new asset id.
///
/// It is looked for among the new items: those the inventory gained over the ids known before. Some may have come
/// another way (a drop, another trade), so an item is only placed on a new item that matches its [`Fingerprint`], or
/// without one, on a new item of its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Located {
    /// Its pattern equals the item's [`Fingerprint`]'s and its wear is within 1e-6 of it (the closest such new item):
    /// this copy.
    Exact(u64),
    /// A new item of its kind (`classid`), per our own web inventory (CS2's context 16), for an item without a
    /// fingerprint (every commodity: copies are alike). If copies of that kind differ, which one it is isn't known.
    AnyOfKind(u64),
    /// No new item for it: none arrived before the timeout after an accept, its fingerprint matched none, or our web
    /// inventory lists no new item of its kind. Also items of other games than CS2, which the game coordinator doesn't
    /// hold: check `offered.appid`, looking again won't find those.
    NotFound,
}

impl Located {
    /// The item's new asset id, if it was found.
    pub fn assetid(&self) -> Option<u64> {
        match *self {
            Located::Exact(id) | Located::AnyOfKind(id) => Some(id),
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

/// How far an item's wear may be from a fingerprint's and still match it: covers a wear rounded to 6 or more decimals
/// (off by 5e-7 at most, and a little more as `f32`).
const WEAR_TOLERANCE: f64 = 1e-6;

/// What a trade keeps of an item and tells copies of one kind apart: its pattern and wear. Commodities (cases,
/// graffiti) have neither.
///
/// An item matches it when its pattern is equal and its wear within 1e-6: a wear the caller stored rounded to 6 or
/// more decimals still matches. In JSON (serde) it is `{"pattern": 302, "wear": 0.37215376}`, the wear written as the
/// shortest decimal that reads back as the same `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "FingerprintJson", from = "FingerprintJson")]
pub struct Fingerprint {
    pattern: i64,
    /// The wear as the game coordinator's `f32`, by its bits so that a fingerprint can be a key.
    wear_bits: u32,
}

impl Fingerprint {
    /// From the pattern ("Pattern Template") and the wear ("Wear Rating"), kept as `f32`: the game coordinator's
    /// float, which the web prints in full.
    pub fn new(pattern: i64, wear: f64) -> Self {
        Self {
            pattern,
            wear_bits: (wear as f32).to_bits(),
        }
    }

    /// An item's, from its web inventory: `None` without a pattern or a wear.
    pub fn of(item: &InventoryItem<'_>) -> Option<Self> {
        Some(Self::new(item.pattern()?, item.wear()?))
    }

    pub const fn pattern(&self) -> i64 {
        self.pattern
    }

    pub fn wear(&self) -> f32 {
        f32::from_bits(self.wear_bits)
    }

    /// How far `item`'s wear is from this one's, if `item` matches: same pattern, wear within [`WEAR_TOLERANCE`].
    fn distance(&self, item: &GcItem) -> Option<f64> {
        if item.pattern.map(i64::from) != Some(self.pattern) {
            return None;
        }
        let distance = (f64::from(item.float?) - f64::from(self.wear())).abs();
        (distance <= WEAR_TOLERANCE).then_some(distance)
    }
}

/// A [`Fingerprint`] in JSON.
#[derive(Serialize, Deserialize)]
struct FingerprintJson {
    pattern: i64,
    wear: f32,
}

impl From<Fingerprint> for FingerprintJson {
    fn from(fingerprint: Fingerprint) -> Self {
        Self {
            pattern: fingerprint.pattern,
            wear: fingerprint.wear(),
        }
    }
}

impl From<FingerprintJson> for Fingerprint {
    fn from(json: FingerprintJson) -> Self {
        Self::new(json.pattern, f64::from(json.wear))
    }
}

/// A received item, as the matching sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wanted {
    classid: i64,
    /// Whether it is a CS2 item: the game coordinator holds no other.
    cs2: bool,
    /// The partner's copy's, from the caller.
    fingerprint: Option<Fingerprint>,
}

impl Wanted {
    /// Each of `received`, with its fingerprint from `fingerprints` by the partner's asset id.
    fn all(received: &[CEcon_Asset], fingerprints: &HashMap<u64, Fingerprint>) -> Vec<Self> {
        received
            .iter()
            .map(|offered| Self {
                classid: offered.classid,
                cs2: offered.appid == i64::from(InventoryContext::CS2.appid()),
                fingerprint: u64::try_from(offered.assetid)
                    .ok()
                    .and_then(|assetid| fingerprints.get(&assetid))
                    .copied(),
            })
            .collect()
    }
}

/// Where the received items went so far, and the new items left.
struct Placed {
    located: Vec<Option<Located>>,
    /// In id order.
    free: Vec<GcItem>,
}

impl Placed {
    fn take(&mut self, wanted: usize, free: usize, how: fn(u64) -> Located) {
        let item = self.free.remove(free);
        self.located[wanted] = Some(how(item.id));
    }
}

/// Items of other games [`Located::NotFound`]; then each item with a fingerprint, in the offer's order, on the closest
/// free new item that matches it, the lowest id of equally close ones ([`Located::Exact`]), else
/// [`Located::NotFound`].
fn place_by_fingerprints(wanted: &[Wanted], new: &[GcItem]) -> Placed {
    let mut free = new.to_vec();
    free.sort_by_key(|item| item.id);
    let mut placed = Placed {
        located: vec![None; wanted.len()],
        free,
    };
    for (index, item) in wanted.iter().enumerate() {
        if !item.cs2 {
            placed.located[index] = Some(Located::NotFound);
        } else if let Some(fingerprint) = item.fingerprint {
            // `min_by` keeps the first of equal ones: the lowest id
            let closest = placed
                .free
                .iter()
                .enumerate()
                .filter_map(|(free, new)| Some((free, fingerprint.distance(new)?)))
                .min_by(|(_, a), (_, b)| a.total_cmp(b));
            match closest {
                Some((free, _)) => placed.take(index, free, Located::Exact),
                None => placed.located[index] = Some(Located::NotFound),
            }
        }
    }
    placed
}

/// The new items whose kind (`classid`) [`locate`] needs, from our own web inventory: those left after the
/// fingerprints, and none when every CS2 item has a fingerprint.
pub(crate) fn ids_needing_kinds(wanted: &[Wanted], new: &[GcItem]) -> Vec<u64> {
    if !wanted.iter().any(|wanted| wanted.cs2 && wanted.fingerprint.is_none()) {
        return Vec::new();
    }
    place_by_fingerprints(wanted, new)
        .free
        .iter()
        .map(|item| item.id)
        .collect()
}

/// Where each of `wanted` (the received items, in the offer's order) went among `new` (the items the game
/// coordinator's inventory gained). `kinds` holds the `classid` of new items, for those [`ids_needing_kinds`] names
/// and our web inventory listed.
///
/// First the fingerprints (see [`place_by_fingerprints`]); then each item left, in the offer's order, on the first
/// free new item of its kind ([`Located::AnyOfKind`]), else [`Located::NotFound`].
pub(crate) fn locate(wanted: &[Wanted], new: &[GcItem], kinds: &HashMap<u64, i64>) -> Vec<Located> {
    let mut placed = place_by_fingerprints(wanted, new);
    for (index, item) in wanted.iter().enumerate() {
        if placed.located[index].is_some() {
            continue;
        }
        match placed
            .free
            .iter()
            .position(|new| kinds.get(&new.id) == Some(&item.classid))
        {
            Some(free) => placed.take(index, free, Located::AnyOfKind),
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
}

impl Wait {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            poll: POLL_INTERVAL,
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
/// tests can stand in for Steam: `accept()`, and `own_inventory()` (CS2's trade-protected context). `fingerprints`
/// are the partner's copies', by their asset id in the offer.
///
/// Notes the game coordinator's item ids, accepts, waits for the new items, and places the received ones as
/// [`find_and_locate`] does. Fails with [`TradeError::Cs2InventoryUnavailable`] before the accept while the inventory
/// isn't current; any failure after it is [`TradeError::AcceptedButItemsNotFound`].
pub(crate) async fn accept_and_locate<S, A, AF, O, OF>(
    tradeoffer_id: u64,
    received: Vec<CEcon_Asset>,
    fingerprints: &HashMap<u64, Fingerprint>,
    source: &mut S,
    accept: A,
    own_inventory: O,
    wait: Wait,
) -> Result<AcceptedOffer, TradeError>
where
    S: ItemSource,
    A: FnOnce() -> AF,
    AF: Future<Output = Result<Option<i64>, TradeError>>,
    O: FnOnce() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let wanted = Wanted::all(&received, fingerprints);

    // Right before the accept: an item that comes from elsewhere earlier isn't taken for a received one.
    let known_ids = source.item_ids()?.ok_or(TradeError::Cs2InventoryUnavailable)?;
    let tradeid = accept().await?;

    match locate_after_accept(&wanted, &known_ids, source, own_inventory, wait).await {
        Ok(located) => Ok(AcceptedOffer {
            tradeid,
            received: received_items(received, located),
            known_ids,
        }),
        Err(error) => Err(TradeError::AcceptedButItemsNotFound {
            tradeoffer_id,
            tradeid,
            known_ids,
            source: Box::new(error),
        }),
    }
}

/// Waits for the new items and places the received ones among them.
async fn locate_after_accept<S, O, OF>(
    wanted: &[Wanted],
    before: &HashSet<u64>,
    source: &mut S,
    own_inventory: O,
    wait: Wait,
) -> Result<Vec<Located>, TradeError>
where
    S: ItemSource,
    O: FnOnce() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let expected = wanted.iter().filter(|wanted| wanted.cs2).count();
    let new = if expected == 0 {
        Vec::new()
    } else {
        wait_for_new_items(source, before, expected, wait).await?
    };
    place(wanted, &new, own_inventory).await
}

/// The steps of `find_received_items`: where `received` (an accepted offer's items to receive) went among the items
/// of `source` not in `known`, looking at it once, with the partner's `fingerprints` by asset id.
/// `own_inventory()` reads CS2's trade-protected context of our web inventory, once at most.
///
/// Fails with [`TradeError::Cs2InventoryUnavailable`] while the inventory isn't current, unless no CS2 item is to be
/// found; with the error of a read of our web inventory that failed; otherwise only if the game coordinator does.
pub(crate) async fn find_and_locate<S, O, OF>(
    received: Vec<CEcon_Asset>,
    known: &HashSet<u64>,
    fingerprints: &HashMap<u64, Fingerprint>,
    source: &mut S,
    own_inventory: O,
) -> Result<Vec<ReceivedItem>, TradeError>
where
    S: ItemSource,
    O: FnOnce() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let wanted = Wanted::all(&received, fingerprints);
    let new = if wanted.iter().any(|wanted| wanted.cs2) {
        source.items_not_in(known)?.ok_or(TradeError::Cs2InventoryUnavailable)?
    } else {
        Vec::new()
    };
    let located = place(&wanted, &new, own_inventory).await?;
    Ok(received_items(received, located))
}

/// [`locate`], with the kinds it needs read from our own web inventory, once. A failed read is the error.
async fn place<O, OF>(wanted: &[Wanted], new: &[GcItem], own_inventory: O) -> Result<Vec<Located>, TradeError>
where
    O: FnOnce() -> OF,
    OF: Future<Output = Result<Inventory, TradeError>>,
{
    let needing = ids_needing_kinds(wanted, new);
    let kinds = if needing.is_empty() {
        HashMap::new()
    } else {
        kinds_of(&needing, &own_inventory().await?)
    };
    Ok(locate(wanted, new, &kinds))
}

/// Each of `received` with where it went.
fn received_items(received: Vec<CEcon_Asset>, located: Vec<Located>) -> Vec<ReceivedItem> {
    received
        .into_iter()
        .zip(located)
        .map(|(offered, located)| ReceivedItem { offered, located })
        .collect()
}

/// The `classid` of the items `ids` that `inventory` lists.
fn kinds_of(ids: &[u64], inventory: &Inventory) -> HashMap<u64, i64> {
    let mut kinds = HashMap::new();
    for page in inventory.pages() {
        for item in page.items() {
            if let Ok(id) = u64::try_from(item.assetid) {
                if ids.contains(&id) {
                    kinds.insert(id, item.classid);
                }
            }
        }
    }
    if kinds.len() < ids.len() {
        debug!("Our web inventory lists {} of {} new items.", kinds.len(), ids.len());
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
        Some(Fingerprint::new(pattern, f64::from(wear)))
    }

    fn wanted(classid: i64, fingerprint: Option<Fingerprint>) -> Wanted {
        Wanted {
            classid,
            cs2: true,
            fingerprint,
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

    fn ids(ids: &[u64]) -> HashSet<u64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn web_wear_matches_the_game_coordinator_float_bit_for_bit() {
        // B's AUG, live 2026-10-21: the web's "0.372153759002685547" and the game coordinator's 0x3ebe8af0
        let fingerprint = web_fingerprint(302, "0.372153759002685547");
        assert_eq!(fingerprint.wear().to_bits(), 0x3ebe_8af0);
        assert_eq!(
            fingerprint.distance(&skin(1, 302, f32::from_bits(0x3ebe_8af0))),
            Some(0.0)
        );
        assert_eq!(fingerprint.distance(&skin(1, 303, f32::from_bits(0x3ebe_8af0))), None);
        // the next float is within the tolerance, but not equal
        let next = fingerprint.distance(&skin(1, 302, f32::from_bits(0x3ebe_8af1)));
        assert!(next.unwrap() > 0.0);
        assert_eq!(fingerprint.distance(&commodity(1)), None);
    }

    #[test]
    fn a_rounded_wear_matches() {
        // the web's "0.372153759002685547" and "0.0661438107490539551", stored rounded to 6 decimals
        let wanted = [
            wanted(AUG, Some(Fingerprint::new(302, 0.372154))),
            wanted(MP7, Some(Fingerprint::new(355, 0.066144))),
        ];
        let new = [
            skin(10, 302, f32::from_bits(0x3ebe_8af0)),
            skin(20, 355, "0.0661438107490539551".parse().unwrap()),
        ];
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Exact(10), Located::Exact(20)]
        );
    }

    #[test]
    fn a_wear_off_by_1e_5_is_not_found() {
        let wanted = [wanted(AUG, Some(Fingerprint::new(302, 0.372_153_76 + 1e-5)))];
        let new = [skin(10, 302, f32::from_bits(0x3ebe_8af0))];
        assert_eq!(locate(&wanted, &new, &no_kinds()), vec![Located::NotFound]);
    }

    #[test]
    fn the_closest_wear_is_taken() {
        let wanted = [
            wanted(MP7, Some(Fingerprint::new(1, 0.5))),
            wanted(MP7, Some(Fingerprint::new(1, 0.5))),
        ];
        // both within the tolerance; 20 is closer
        let new = [skin(10, 1, 0.500_000_4), skin(20, 1, 0.500_000_1)];
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Exact(20), Located::Exact(10)]
        );
    }

    #[test]
    fn exact() {
        let wanted = [
            wanted(MP7, fingerprint(355, 0.066_143_81)),
            wanted(MP7, fingerprint(12, 0.2)),
        ];
        let new = [skin(20, 12, 0.2), skin(10, 355, 0.066_143_81)];
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Exact(10), Located::Exact(20)]
        );
    }

    #[test]
    fn exact_ignores_items_from_elsewhere() {
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
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Exact(10), Located::Exact(20)]
        );
    }

    #[test]
    fn a_fingerprint_without_a_match_is_not_found() {
        let wanted = [wanted(AUG, fingerprint(302, 0.372_153_76))];
        let elsewhere = [skin(9, 1, 0.5)];
        assert!(ids_needing_kinds(&wanted, &elsewhere).is_empty());
        // not even on a new item of its kind
        assert_eq!(
            locate(&wanted, &elsewhere, &kinds(&[(9, AUG)])),
            vec![Located::NotFound]
        );
    }

    #[test]
    fn identical_fingerprints_take_one_copy_each() {
        let wanted = [
            wanted(MP7, fingerprint(1, 0.5)),
            wanted(MP7, fingerprint(1, 0.5)),
            wanted(MP7, fingerprint(1, 0.5)),
        ];
        let new = [skin(30, 1, 0.5), skin(20, 1, 0.5)];
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::Exact(20), Located::Exact(30), Located::NotFound]
        );
    }

    #[test]
    fn fingerprints_are_placed_before_kinds() {
        // the first MP7 has no fingerprint: it doesn't take the copy the second one's fingerprint matches
        let wanted = [wanted(MP7, None), wanted(MP7, fingerprint(355, 0.066_143_81))];
        let new = [skin(10, 355, 0.066_143_81), skin(20, 12, 0.2)];
        assert_eq!(ids_needing_kinds(&wanted, &new), vec![20]);
        assert_eq!(
            locate(&wanted, &new, &kinds(&[(10, MP7), (20, MP7)])),
            vec![Located::AnyOfKind(20), Located::Exact(10)]
        );
    }

    #[test]
    fn any_of_kind() {
        let wanted = [wanted(CASE, None), wanted(CASE, None), wanted(AUG, None)];
        let new = [commodity(12), commodity(11), skin(13, 302, 0.37)];
        assert_eq!(ids_needing_kinds(&wanted, &new), vec![11, 12, 13]);
        let kinds = kinds(&[(11, CASE), (12, CASE), (13, AUG)]);
        assert_eq!(
            locate(&wanted, &new, &kinds),
            vec![Located::AnyOfKind(11), Located::AnyOfKind(12), Located::AnyOfKind(13)]
        );
    }

    #[test]
    fn a_new_item_of_another_kind_is_not_taken() {
        let wanted = [wanted(AUG, None)];
        let new = [skin(1, 7, 0.1)];
        assert_eq!(locate(&wanted, &new, &kinds(&[(1, MP7)])), vec![Located::NotFound]);
    }

    #[test]
    fn an_unread_kind_is_not_found() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        // 2 isn't listed (yet) on the web
        let new = [skin(1, 302, 0.37), commodity(2)];
        assert_eq!(
            locate(&wanted, &new, &kinds(&[(1, AUG)])),
            vec![Located::AnyOfKind(1), Located::NotFound]
        );
    }

    #[test]
    fn not_found() {
        let wanted = [wanted(AUG, None), wanted(CASE, None)];
        assert!(ids_needing_kinds(&wanted, &[]).is_empty());
        assert_eq!(
            locate(&wanted, &[], &no_kinds()),
            vec![Located::NotFound, Located::NotFound]
        );
    }

    #[test]
    fn other_games_are_not_found() {
        let mut gem = wanted(667924416, None);
        gem.cs2 = false;
        let wanted = [gem, wanted(AUG, fingerprint(302, 0.37))];
        let new = [skin(1, 302, 0.37), commodity(2)];
        // the gem has no fingerprint, but no kind is read for it
        assert!(ids_needing_kinds(&wanted, &new).is_empty());
        assert_eq!(
            locate(&wanted, &new, &no_kinds()),
            vec![Located::NotFound, Located::Exact(1)]
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
                before: ids(before),
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

    /// Context 16 listing `items`.
    fn protected(items: &[(u64, i64)]) -> impl FnOnce() -> Result<Inventory, TradeError> + '_ {
        move || kinds_page(InventoryContext::CS2_TRADE_PROTECTED, items)
    }

    fn no_web_read() -> Result<Inventory, TradeError> {
        panic!("no web read")
    }

    fn web_fingerprint(pattern: i64, wear: &str) -> Fingerprint {
        Fingerprint::new(pattern, wear.parse().unwrap())
    }

    fn too_many_requests() -> TradeError {
        tappet::errors::SteamAPIError::SteamHttpError("429".to_string()).into()
    }

    fn located(received: &[ReceivedItem]) -> Vec<Located> {
        received.iter().map(|item| item.located).collect()
    }

    /// `accept_and_locate` of offer 7, accepted as trade 42, with `fingerprints`, and how many times our own web
    /// inventory was read.
    fn accept<S: ItemSource>(
        received: Vec<CEcon_Asset>,
        fingerprints: &[(u64, Fingerprint)],
        source: &mut S,
        own_inventory: impl FnOnce() -> Result<Inventory, TradeError>,
        wait: Wait,
    ) -> (Result<AcceptedOffer, TradeError>, usize) {
        let fingerprints: HashMap<u64, Fingerprint> = fingerprints.iter().copied().collect();
        let reads = Cell::new(0);
        let result = block_on(accept_and_locate(
            7,
            received,
            &fingerprints,
            source,
            || ready(Ok(Some(42))),
            || {
                reads.set(reads.get() + 1);
                ready(own_inventory())
            },
            wait,
        ));
        (result, reads.get())
    }

    #[test]
    fn nothing_is_accepted_without_a_current_inventory() {
        let mut source = Script::new(&[1], vec![None]);
        let accepted = Cell::new(false);
        let result = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &HashMap::new(),
            &mut source,
            || {
                accepted.set(true);
                ready(Ok(Some(42)))
            },
            || ready(no_web_read()),
            fast(Duration::from_secs(1)),
        ));
        assert!(
            matches!(result, Err(TradeError::Cs2InventoryUnavailable)),
            "{:?}",
            result
        );
        assert!(!accepted.get());
        assert_eq!(source.looks, 1);
    }

    #[test]
    fn one_fingerprinted_skin_no_web_read() {
        let mut source = Script::new(
            &[1],
            vec![
                Some(vec![]),
                Some(vec![]),
                Some(vec![skin(54238728503, 302, 0.372_153_76)]),
            ],
        );
        let (result, reads) = accept(
            vec![asset(InventoryContext::CS2, 24234602605, AUG)],
            &[(24234602605, web_fingerprint(302, "0.372153759002685547"))],
            &mut source,
            no_web_read,
            fast(Duration::from_secs(10)),
        );
        let accepted = result.unwrap();
        assert_eq!(accepted.tradeid, Some(42));
        assert_eq!(accepted.known_ids, ids(&[1]));
        assert_eq!(accepted.received.len(), 1);
        assert_eq!(accepted.received[0].offered.assetid, 24234602605);
        assert_eq!(accepted.received[0].located, Located::Exact(54238728503));
        assert_eq!(reads, 0);
    }

    #[test]
    fn copies_of_one_kind_after_a_confirmation() {
        let new = vec![skin(901, 12, 0.2), skin(902, 355, 0.066_143_81)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(new)]);
        let fingerprints: HashMap<u64, Fingerprint> = vec![
            (100, web_fingerprint(355, "0.0661438107490539551")),
            (101, web_fingerprint(12, "0.200000002980232239")),
        ]
        .into_iter()
        .collect();
        let accepted = block_on(accept_and_locate(
            7,
            vec![
                asset(InventoryContext::CS2, 100, MP7),
                asset(InventoryContext::CS2, 101, MP7),
            ],
            &fingerprints,
            &mut source,
            || ready(Ok(None)),
            || ready(no_web_read()),
            fast(Duration::from_secs(10)),
        ))
        .unwrap();
        assert_eq!(accepted.tradeid, None);
        assert_eq!(
            located(&accepted.received),
            vec![Located::Exact(902), Located::Exact(901)]
        );
    }

    #[test]
    fn a_commodity_after_the_accept_reads_our_inventory_once() {
        let new = vec![skin(901, 302, 0.37), commodity(902)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(new)]);
        let (result, reads) = accept(
            vec![
                asset(InventoryContext::CS2, 100, AUG),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[(100, Fingerprint::new(302, 0.37))],
            &mut source,
            protected(&[(901, AUG), (902, CASE)]),
            fast(Duration::from_secs(10)),
        );
        let accepted = result.unwrap();
        assert_eq!(
            located(&accepted.received),
            vec![Located::Exact(901), Located::AnyOfKind(902)]
        );
        assert_eq!(reads, 1);
    }

    #[test]
    fn a_timeout_after_the_accept_keeps_the_trade_and_the_snapshot() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let (result, reads) = accept(
            vec![asset(InventoryContext::CS2, 100, CASE)],
            &[],
            &mut source,
            no_web_read,
            fast(Duration::from_millis(20)),
        );
        let accepted = result.unwrap();
        assert_eq!(accepted.tradeid, Some(42));
        assert_eq!(accepted.known_ids, ids(&[1]));
        assert_eq!(accepted.received[0].located, Located::NotFound);
        // nothing new: no kind to read
        assert_eq!(reads, 0);
    }

    #[test]
    fn a_failed_web_read_after_the_accept_carries_the_trade_and_the_snapshot() {
        let new = vec![commodity(902)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(new)]);
        let (result, reads) = accept(
            vec![asset(InventoryContext::CS2, 101, CASE)],
            &[],
            &mut source,
            || Err(too_many_requests()),
            fast(Duration::from_secs(10)),
        );
        assert_eq!(reads, 1);
        let error = result.unwrap_err();
        assert!(error.is_too_many_requests(), "{:?}", error);
        match error {
            TradeError::AcceptedButItemsNotFound {
                tradeoffer_id: 7,
                tradeid: Some(42),
                known_ids,
                ..
            } => assert_eq!(known_ids, ids(&[1])),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn after_a_failed_web_read_the_snapshot_finds_the_items() {
        let received = vec![
            asset(InventoryContext::CS2, 100, AUG),
            asset(InventoryContext::CS2, 101, CASE),
        ];
        let fingerprints = [(100, Fingerprint::new(302, 0.37))];
        let new = vec![skin(901, 302, 0.37), commodity(902)];
        let mut source = Script::new(&[1], vec![Some(vec![]), Some(new)]);
        let (result, _) = accept(
            received.clone(),
            &fingerprints,
            &mut source,
            || Err(too_many_requests()),
            fast(Duration::from_secs(10)),
        );
        let known_ids = match result {
            Err(TradeError::AcceptedButItemsNotFound { known_ids, .. }) => known_ids,
            other => panic!("{:?}", other),
        };

        // later: 903 came from elsewhere
        let mut now = Now::current(vec![
            skin(1, 7, 0.1),
            skin(901, 302, 0.37),
            commodity(902),
            commodity(903),
        ]);
        let known: Vec<u64> = known_ids.into_iter().collect();
        let (result, reads) = find(
            received,
            &known,
            &fingerprints,
            &mut now,
            protected(&[(902, CASE), (903, MP7)]),
        );
        assert_eq!(
            located(&result.unwrap()),
            vec![Located::Exact(901), Located::AnyOfKind(902)]
        );
        assert_eq!(reads, 1);
    }

    /// Fails after the accept, as a stopped client does.
    struct FailsAfter(usize);

    impl ItemSource for FailsAfter {
        fn item_ids(&mut self) -> Result<Option<HashSet<u64>>, TradeError> {
            Ok(Some(ids(&[1])))
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
    fn a_game_coordinator_failure_after_the_accept_carries_the_trade_and_the_snapshot() {
        let (result, _) = accept(
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &[],
            &mut FailsAfter(2),
            no_web_read,
            fast(Duration::from_secs(10)),
        );
        match result {
            Err(TradeError::AcceptedButItemsNotFound {
                tradeoffer_id: 7,
                tradeid: Some(42),
                known_ids,
                source,
            }) => {
                assert_eq!(known_ids, ids(&[1]));
                assert!(matches!(*source, TradeError::GeneralError(_)));
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn a_failed_accept_is_plain() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let result = block_on(accept_and_locate(
            7,
            vec![asset(InventoryContext::CS2, 100, AUG)],
            &HashMap::new(),
            &mut source,
            || ready(Err(TradeError::GeneralError("refused".to_string()))),
            || ready(no_web_read()),
            fast(Duration::from_secs(10)),
        ));
        assert!(matches!(result, Err(TradeError::GeneralError(_))), "{:?}", result);
    }

    #[test]
    fn a_gift_waits_for_nothing() {
        let mut source = Script::new(&[1], vec![Some(vec![])]);
        let (result, reads) = accept(Vec::new(), &[], &mut source, no_web_read, fast(Duration::from_secs(10)));
        let accepted = result.unwrap();
        assert!(accepted.received.is_empty());
        assert_eq!(accepted.known_ids, ids(&[1]));
        // the snapshot only
        assert_eq!(source.looks, 1);
        assert_eq!(reads, 0);
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

    /// `find_and_locate` with `known` ids and `fingerprints`, with how many times our own web inventory was read.
    fn find<S: ItemSource>(
        received: Vec<CEcon_Asset>,
        known: &[u64],
        fingerprints: &[(u64, Fingerprint)],
        source: &mut S,
        own_inventory: impl FnOnce() -> Result<Inventory, TradeError>,
    ) -> (Result<Vec<ReceivedItem>, TradeError>, usize) {
        let known = ids(known);
        let fingerprints: HashMap<u64, Fingerprint> = fingerprints.iter().copied().collect();
        let reads = Cell::new(0);
        let result = block_on(find_and_locate(received, &known, &fingerprints, source, || {
            reads.set(reads.get() + 1);
            ready(own_inventory())
        }));
        (result, reads.get())
    }

    #[test]
    fn fingerprint_of_a_web_item() {
        let inventory = web_inventory(
            InventoryContext::CS2_TRADE_PROTECTED,
            &[(54238728503, AUG, 302, "0.372153759002685547")],
        );
        let page = inventory.pages().next().unwrap();
        let fingerprint = Fingerprint::of(&page.items().next().unwrap()).unwrap();
        assert_eq!(
            fingerprint,
            Fingerprint::new(302, "0.372153759002685547".parse().unwrap())
        );
        assert_eq!(
            (fingerprint.pattern(), fingerprint.wear()),
            (302, f32::from_bits(0x3ebe_8af0))
        );

        let case = kinds_page(InventoryContext::CS2, &[(1, CASE)]).unwrap();
        let page = case.pages().next().unwrap();
        assert_eq!(Fingerprint::of(&page.items().next().unwrap()), None);
    }

    #[test]
    fn fingerprint_json() {
        let live = web_fingerprint(302, "0.372153759002685547");
        let json = serde_json::to_string(&live).unwrap();
        assert_eq!(json, r#"{"pattern":302,"wear":0.37215376}"#);
        assert_eq!(serde_json::from_str::<Fingerprint>(&json).unwrap(), live);
        // the web's wear in full reads as the same float
        let full = r#"{"pattern":302,"wear":0.372153759002685547}"#;
        assert_eq!(serde_json::from_str::<Fingerprint>(full).unwrap(), live);
    }

    #[test]
    fn fingerprint_json_round_trips() {
        // xorshift64: the same values on every run
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..10_000 {
            let random = next();
            // any f32 in [0, 1), by its bits; and a wear as f64 in [0, 1)
            let bits = Fingerprint::new(
                random as i64,
                f64::from(f32::from_bits((random >> 32) as u32 % 0x3f80_0000)),
            );
            let wear = Fingerprint::new(1000, (random >> 11) as f64 / (1_u64 << 53) as f64);
            for fingerprint in [bits, wear] {
                let json = serde_json::to_string(&fingerprint).unwrap();
                let read: Fingerprint = serde_json::from_str(&json).unwrap();
                assert_eq!(read, fingerprint, "{}", json);
                assert_eq!(read.wear().to_bits(), fingerprint.wear().to_bits(), "{}", json);
            }
        }
    }

    #[test]
    fn partner_accepted_skins_by_fingerprints() {
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
        let received = result.unwrap();
        assert_eq!(received[0].offered.assetid, 100);
        assert_eq!(located(&received), vec![Located::Exact(902), Located::Exact(901)]);
        assert_eq!(reads, 0);
        assert_eq!(source.looks, 1);
    }

    #[test]
    fn partner_accepted_commodities_by_kind() {
        let mut source = Now::current(vec![commodity(12), commodity(11), skin(13, 302, 0.37)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, CASE),
                asset(InventoryContext::CS2, 101, CASE),
                asset(InventoryContext::CS2, 102, AUG),
            ],
            &[],
            &[],
            &mut source,
            protected(&[(11, CASE), (12, CASE), (13, AUG)]),
        );
        assert_eq!(
            located(&result.unwrap()),
            vec![Located::AnyOfKind(11), Located::AnyOfKind(12), Located::AnyOfKind(13)]
        );
        assert_eq!(reads, 1);
        assert_eq!(source.looks, 1);
    }

    #[test]
    fn partner_accepted_a_fingerprint_and_a_kind() {
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
        assert_eq!(
            located(&result.unwrap()),
            vec![Located::Exact(10), Located::AnyOfKind(20)]
        );
        assert_eq!(reads, 1);
    }

    #[test]
    fn partner_accepted_an_unread_kind_is_not_found() {
        let mut source = Now::current(vec![skin(54238728503, 302, 0.37)]);
        let (result, reads) = find(
            vec![asset(InventoryContext::CS2, 24234602605, AUG)],
            &[],
            &[],
            &mut source,
            protected(&[]),
        );
        assert_eq!(located(&result.unwrap()), vec![Located::NotFound]);
        assert_eq!(reads, 1);
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
        assert_eq!(located(&result.unwrap()), vec![Located::NotFound]);
        assert_eq!(reads, 1);
    }

    #[test]
    fn partner_accepted_with_an_unreadable_web_inventory_fails_with_the_read() {
        let mut source = Now::current(vec![commodity(12), commodity(11)]);
        let (result, reads) = find(
            vec![
                asset(InventoryContext::CS2, 100, CASE),
                asset(InventoryContext::CS2, 101, CASE),
            ],
            &[],
            &[],
            &mut source,
            || Err(too_many_requests()),
        );
        match result {
            Err(error) => assert!(error.is_too_many_requests(), "{:?}", error),
            other => panic!("{:?}", other),
        }
        assert_eq!(reads, 1);
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
        assert_eq!(located(&result.unwrap()), vec![Located::NotFound]);
        assert_eq!(reads, 0);
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
        assert_eq!(located(&result.unwrap()), vec![Located::Exact(6)]);
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
        assert_eq!(reads, 0);
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
        assert_eq!(located(&result.unwrap()), vec![Located::NotFound]);
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
}

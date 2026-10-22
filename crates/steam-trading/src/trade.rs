//! Trades: what accepted offers became, as `GetTradeHistory` lists them.

use std::convert::TryFrom;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use steamid_parser::SteamID;
/// A trade's status, Steam's `ETradeStatus` (tappet's), and the values this version doesn't know.
pub use tappet::response_types::ETradeStatus as TradeStatus;
use tappet::response_types::TradeHistory_Trade;
use tappet::response_types::TradeHistory_TradedAsset;

use crate::offer::number;
use crate::Asset;
use crate::AssetId;
use crate::InventoryContext;
use crate::TradeId;

/// How many trades `GetTradeHistory` is asked for by default, as this crate always asked.
const DEFAULT_MAX_TRADES: u32 = 500;

/// A trade of this account, as `GetTradeHistory` lists it.
///
/// Since ~2026-04 Steam sends no `new_assetid` for CS2 trades (seen live), so a CS2 trade is never
/// [`Trade::is_settled`] and [`Trade::new_asset`] finds nothing for its items: use `find_received_items` (feature
/// `cs2`) to find them.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// Its id (`tradeid`).
    pub id: TradeId,
    /// The other account (`steamid_other`).
    pub partner: SteamID,
    /// Its status (`status`).
    pub status: TradeStatus,
    /// Unix time it started (`time_init`).
    pub started: i64,
    /// Unix time its items' trade protection ends (`time_settlement`), Steam's own: seen live on a CS2 trade, where
    /// it was the time the item's inventory description gave. Older trades have none.
    pub protected_until: Option<i64>,
    /// The items this account received.
    pub received: Vec<TradedItem>,
    /// The items this account gave.
    pub given: Vec<TradedItem>,
}

impl Trade {
    /// Whether its items are still trade protected, now.
    pub fn is_protected(&self) -> bool {
        self.is_protected_at(unix_now())
    }

    /// Whether its items are still trade protected at the Unix time `unix`.
    pub fn is_protected_at(&self, unix: i64) -> bool {
        self.protected_until.is_some_and(|until| until > unix)
    }

    /// How long its items stay trade protected; `None` when that is over, or the trade has no protection.
    pub fn time_left(&self) -> Option<Duration> {
        let left = self.protected_until? - unix_now();
        u64::try_from(left)
            .ok()
            .filter(|&left| left > 0)
            .map(Duration::from_secs)
    }

    /// Whether every item has its new asset id. Never for a CS2 trade (see [`Trade`]).
    pub fn is_settled(&self) -> bool {
        self.items().all(|item| item.new_asset.is_some())
    }

    /// Where the item that was `old` before the trade is now, if Steam says.
    pub fn new_asset(&self, old: AssetId) -> Option<Asset> {
        self.items().find(|item| item.asset.id == old)?.new_asset
    }

    fn items(&self) -> impl Iterator<Item = &TradedItem> {
        self.received.iter().chain(&self.given)
    }
}

fn unix_now() -> i64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    i64::try_from(now.as_secs()).unwrap_or(i64::MAX)
}

/// An item of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TradedItem {
    /// Where it was before the trade, in its owner's inventory.
    pub asset: Asset,
    /// Its kind. It stays the same when the item is traded.
    pub classid: u64,
    /// With `classid`, its description (`instanceid`).
    pub instanceid: u64,
    /// How many of it, more than 1 for a stackable item.
    pub amount: u32,
    /// Where it is now (`new_assetid`, `new_contextid`), once the trade settled. Never for CS2 (see [`Trade`]).
    pub new_asset: Option<Asset>,
    /// Where it went back to, if the trade was rolled back (`rollback_new_assetid`, `rollback_new_contextid`).
    pub rollback_new_asset: Option<Asset>,
}

/// Which trades [`SteamTradeManager::trades`](crate::SteamTradeManager::trades) lists. The default is the request
/// this crate always sent: the last 500, failed ones left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeQuery {
    /// At most this many, newest first.
    pub max: u32,
    /// Failed trades too.
    pub include_failed: bool,
}

impl Default for TradeQuery {
    fn default() -> Self {
        Self {
            max: DEFAULT_MAX_TRADES,
            include_failed: false,
        }
    }
}

impl Trade {
    /// The trade Steam sent, or why it can't be one.
    pub(crate) fn of(trade: TradeHistory_Trade) -> Result<Self, String> {
        let items = |items: Option<Vec<TradeHistory_TradedAsset>>| -> Result<Vec<TradedItem>, String> {
            items.unwrap_or_default().iter().map(TradedItem::of).collect()
        };
        Ok(Self {
            id: TradeId(number(trade.tradeid, "tradeid")?),
            partner: SteamID::from_steam64(trade.steamid_other),
            status: trade.status,
            started: trade.time_init,
            protected_until: trade.time_settlement,
            received: items(trade.assets_received)?,
            given: items(trade.assets_given)?,
        })
    }
}

impl TradedItem {
    /// The item Steam sent, or why it can't be one.
    pub(crate) fn of(item: &TradeHistory_TradedAsset) -> Result<Self, String> {
        let appid = item.appid;
        let moved = |id: Option<i64>, context: Option<u32>| -> Result<Option<Asset>, String> {
            match (id, context) {
                (Some(id), Some(context)) => Ok(Some(Asset::new(
                    InventoryContext::new(appid, context),
                    AssetId(number(id, "new_assetid")?),
                ))),
                _ => Ok(None),
            }
        };
        Ok(Self {
            asset: Asset::new(
                InventoryContext::new(appid, number(item.contextid, "contextid")?),
                AssetId(number(item.assetid, "assetid")?),
            ),
            classid: number(item.classid, "classid")?,
            instanceid: number(item.instanceid, "instanceid")?,
            amount: number(item.amount.ok_or("no `amount`")?, "amount")?,
            new_asset: moved(item.new_assetid, item.new_contextid)?,
            rollback_new_asset: moved(item.rollback_new_assetid, item.rollback_new_contextid)?,
        })
    }
}

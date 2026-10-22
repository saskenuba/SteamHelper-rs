//! Types returned by Steam trade operations.
//!
//! The models described here can be found at:
//! `https://developer.valvesoftware.com/wiki/Steam_Web_API/IEconService`
//!
//! Every field is `optional` in Valve's schema (`webui/service_econ.proto`, `CEcon_TradeOffer` and siblings). Fields
//! that identify something stay required; the others are `Option`, or `#[serde(default)]` where the default is what
//! Steam means by leaving them out. 64-bit integers come as JSON strings.

use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DisplayFromStr};
use steam_language::enums::EResult;
use steam_language::enums::ETradeOfferConfirmationMethod;
use steam_language::enums::ETradeOfferState;

/// A value of a Steam enum, or one this version doesn't know yet.
///
/// Steam adds enum values without notice, and one unknown value must not fail a whole response. Compares equal to
/// `E` both ways, so `offer.state == ETradeOfferState::Active` works as with a plain `ETradeOfferState`. A `match`
/// needs `EnumOrUnknown::Known(ETradeOfferState::Active)`, or a match on [Self::known].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumOrUnknown<E> {
    Known(E),
    /// The raw value.
    Unknown(i32),
}

impl<E> EnumOrUnknown<E> {
    pub fn known(&self) -> Option<&E> {
        match self {
            EnumOrUnknown::Known(value) => Some(value),
            EnumOrUnknown::Unknown(_) => None,
        }
    }
}

impl<E> From<E> for EnumOrUnknown<E> {
    fn from(value: E) -> Self {
        EnumOrUnknown::Known(value)
    }
}

impl<E: PartialEq> PartialEq<E> for EnumOrUnknown<E> {
    fn eq(&self, other: &E) -> bool {
        matches!(self, EnumOrUnknown::Known(value) if value == other)
    }
}

/// `E == EnumOrUnknown<E>`, for the enums this crate wraps. The orphan rule forbids a generic
/// `impl<E> PartialEq<EnumOrUnknown<E>> for E`.
macro_rules! eq_enum_or_unknown {
    ($($enum:ty),*) => {
        $(
            impl PartialEq<EnumOrUnknown<$enum>> for $enum {
                fn eq(&self, other: &EnumOrUnknown<$enum>) -> bool {
                    other == self
                }
            }
        )*
    };
}

eq_enum_or_unknown!(ETradeOfferState, ETradeOfferConfirmationMethod, EResult);

impl<'de, E: DeserializeOwned> Deserialize<'de> for EnumOrUnknown<E> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = i32::deserialize(deserializer)?;
        let deserializer: serde::de::value::I32Deserializer<serde::de::value::Error> = raw.into_deserializer();

        Ok(match E::deserialize(deserializer) {
            Ok(value) => EnumOrUnknown::Known(value),
            Err(_) => EnumOrUnknown::Unknown(raw),
        })
    }
}

/// Tracks the status of a completed trade. I.e. after a trade offer has been accepted.
/// Received at GetTradeHistory endpoint, on `status` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ETradeStatus {
    /// Trade has just been accepted/confirmed, but no work has been done yet
    Init,
    /// Steam is about to start committing the trade
    PreCommitted,
    /// The items have been exchanged
    Committed,
    /// All work is finished
    Complete,
    /// Something went wrong after Init, but before Committed, and the trade has been rolled back
    Failed,
    /// A support person rolled back the trade for one side
    PartialSupportRollback,
    /// A support person rolled back the trade for both sides
    FullSupportRollback,
    /// A support person rolled back the trade for some set of items
    SupportRollbackSelective,
    /// We tried to roll back the trade when it failed, but haven't managed to do that for all
    /// items yet
    RollbackFailed,
    /// We tried to roll back the trade, but some failure didn't go away and we gave up */
    RollbackAbandoned,
    /// Trade is in escrow
    InEscrow,
    /// A trade in escrow was rolled back
    EscrowRollback,
    /// A value this version doesn't know; Steam adds statuses without notice.
    Unknown(i32),
}

impl From<i32> for ETradeStatus {
    fn from(value: i32) -> Self {
        match value {
            0 => ETradeStatus::Init,
            1 => ETradeStatus::PreCommitted,
            2 => ETradeStatus::Committed,
            3 => ETradeStatus::Complete,
            4 => ETradeStatus::Failed,
            5 => ETradeStatus::PartialSupportRollback,
            6 => ETradeStatus::FullSupportRollback,
            7 => ETradeStatus::SupportRollbackSelective,
            8 => ETradeStatus::RollbackFailed,
            9 => ETradeStatus::RollbackAbandoned,
            10 => ETradeStatus::InEscrow,
            11 => ETradeStatus::EscrowRollback,
            other => ETradeStatus::Unknown(other),
        }
    }
}

impl From<ETradeStatus> for i32 {
    fn from(status: ETradeStatus) -> Self {
        match status {
            ETradeStatus::Init => 0,
            ETradeStatus::PreCommitted => 1,
            ETradeStatus::Committed => 2,
            ETradeStatus::Complete => 3,
            ETradeStatus::Failed => 4,
            ETradeStatus::PartialSupportRollback => 5,
            ETradeStatus::FullSupportRollback => 6,
            ETradeStatus::SupportRollbackSelective => 7,
            ETradeStatus::RollbackFailed => 8,
            ETradeStatus::RollbackAbandoned => 9,
            ETradeStatus::InEscrow => 10,
            ETradeStatus::EscrowRollback => 11,
            ETradeStatus::Unknown(other) => other,
        }
    }
}

// Hand-written: `serde_repr` has no `#[serde(other)]` to catch unknown values.
impl<'de> Deserialize<'de> for ETradeStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        i32::deserialize(deserializer).map(ETradeStatus::from)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Descriptions {
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u32,
    pub marketable: bool,
    pub tradable: bool,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct GetTradeOfferResponse {
    pub response: CEcon_GetTradeOffer_Response,
}

/// Steam's answer for one offer. Without an `offer`, there is no such offer: node-steam-tradeoffer-manager fails then
/// (`lib/index.js` ~531-545).
#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
#[serde(from = "RawGetTradeOfferResponse")]
pub struct CEcon_GetTradeOffer_Response {
    /// The offer, if Steam sent one that could be read; else see [Self::malformed_offer].
    pub offer: Option<TradeOffer_Trade>,
    /// The offer Steam sent, if it is not a [TradeOffer_Trade], as in `GetTradeOffers`.
    pub malformed_offer: Option<MalformedOffer>,
}

/// `CEcon_GetTradeOffer_Response` as Steam sends it.
#[derive(Deserialize)]
struct RawGetTradeOfferResponse {
    offer: Option<OfferEntry>,
}

impl From<RawGetTradeOfferResponse> for CEcon_GetTradeOffer_Response {
    fn from(raw: RawGetTradeOfferResponse) -> Self {
        match raw.offer {
            None => Self {
                offer: None,
                malformed_offer: None,
            },
            Some(OfferEntry::Offer(offer)) => Self {
                offer: Some(*offer),
                malformed_offer: None,
            },
            Some(OfferEntry::Malformed(malformed)) => Self {
                offer: None,
                malformed_offer: Some(malformed),
            },
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents the raw CEcon_GetTradeOffers_Response_Base
pub struct GetTradeOffersResponse {
    pub response: CEcon_GetTradeOffers_Response,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
#[serde(from = "RawGetTradeOffersResponse")]
pub struct CEcon_GetTradeOffers_Response {
    /// The offers that could be read; the others are in [Self::malformed_offers_sent].
    pub trade_offers_sent: Option<Vec<TradeOffer_Trade>>,
    /// The offers that could be read; the others are in [Self::malformed_offers_received].
    pub trade_offers_received: Option<Vec<TradeOffer_Trade>>,
    /// Steam pages this endpoint: pass this to `GetTradeOffers::cursor` for the next page. See
    /// [Self::next_page_cursor].
    pub next_cursor: Option<u32>,
    /// Entries of Steam's `trade_offers_sent` that are not a [TradeOffer_Trade], e.g. without `accountid_other`.
    pub malformed_offers_sent: Vec<MalformedOffer>,
    /// Entries of Steam's `trade_offers_received` that are not a [TradeOffer_Trade].
    pub malformed_offers_received: Vec<MalformedOffer>,
}

impl CEcon_GetTradeOffers_Response {
    /// The cursor of the next page, or `None` on the last page. node-steam-tradeoffer-manager stops when
    /// `next_cursor` is 0 or absent (`lib/index.js:609-610`).
    pub fn next_page_cursor(&self) -> Option<u32> {
        self.next_cursor.filter(|cursor| *cursor != 0)
    }

    /// Whether an offer could not be read: the offer lists are then incomplete.
    pub fn has_malformed_offers(&self) -> bool {
        !self.malformed_offers_sent.is_empty() || !self.malformed_offers_received.is_empty()
    }
}

/// `CEcon_GetTradeOffers_Response` as Steam sends it, before the offers that can't be read are put aside.
#[derive(Deserialize)]
struct RawGetTradeOffersResponse {
    trade_offers_sent: Option<Vec<OfferEntry>>,
    trade_offers_received: Option<Vec<OfferEntry>>,
    next_cursor: Option<u32>,
}

impl From<RawGetTradeOffersResponse> for CEcon_GetTradeOffers_Response {
    fn from(raw: RawGetTradeOffersResponse) -> Self {
        let (trade_offers_sent, malformed_offers_sent) = split_offers(raw.trade_offers_sent);
        let (trade_offers_received, malformed_offers_received) = split_offers(raw.trade_offers_received);

        Self {
            trade_offers_sent,
            trade_offers_received,
            next_cursor: raw.next_cursor,
            malformed_offers_sent,
            malformed_offers_received,
        }
    }
}

/// An absent list stays `None`.
fn split_offers(entries: Option<Vec<OfferEntry>>) -> (Option<Vec<TradeOffer_Trade>>, Vec<MalformedOffer>) {
    let entries = match entries {
        Some(entries) => entries,
        None => return (None, vec![]),
    };

    let mut offers = Vec::with_capacity(entries.len());
    let mut malformed = vec![];
    for entry in entries {
        match entry {
            OfferEntry::Offer(offer) => offers.push(*offer),
            OfferEntry::Malformed(entry) => malformed.push(entry),
        }
    }
    (Some(offers), malformed)
}

/// An entry of an offer list. One offer that can't be read must not hide the others, so it is kept aside instead of
/// failing the whole response.
enum OfferEntry {
    Offer(Box<TradeOffer_Trade>),
    Malformed(MalformedOffer),
}

impl<'de> Deserialize<'de> for OfferEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(deserializer)?;

        Ok(match TradeOffer_Trade::deserialize(&raw) {
            Ok(offer) => OfferEntry::Offer(Box::new(offer)),
            Err(error) => OfferEntry::Malformed(MalformedOffer {
                raw,
                reason: error.to_string(),
            }),
        })
    }
}

/// An offer list entry that could not be read as a [TradeOffer_Trade].
///
/// node-steam-tradeoffer-manager fails the whole `GetTradeOffers` call when an offer has no `accountid_other`
/// (`offerSuperMalformed` in `lib/helpers.js`, "Data temporarily unavailable"); here the other offers are still read.
#[derive(Debug, Clone, PartialEq)]
pub struct MalformedOffer {
    /// The entry as Steam sent it.
    pub raw: serde_json::Value,
    /// Why it was not read.
    pub reason: String,
}

impl MalformedOffer {
    /// The offer's id, if `raw` has a readable one.
    pub fn tradeofferid(&self) -> Option<u64> {
        match self.raw.get("tradeofferid")? {
            serde_json::Value::String(id) => id.parse().ok(),
            id => id.as_u64(),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents a steam trade offer. CEcon_Trade
/// Returned by GetTradeOffers (vector) and GetTradeOffer.
pub struct TradeOffer_Trade {
    /// Unique ID generated when a trade offer is created
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeofferid: u64,
    /// SteamID3
    pub accountid_other: u64,
    /// Message included by the creator of the trade offer. Empty if Steam leaves it out.
    #[serde(default)]
    pub message: String,
    /// Unix time when the offer will expire (or expired, if it is in the past)
    pub expiration_time: Option<i64>,
    /// State of trade offer
    #[serde(rename = "trade_offer_state")]
    pub state: EnumOrUnknown<ETradeOfferState>,
    /// May hold entries that are not usable items; see [OfferItem] and [Self::has_malformed_items].
    #[serde(default)]
    pub items_to_give: Vec<OfferItem>,
    /// May hold entries that are not usable items; see [OfferItem] and [Self::has_malformed_items].
    #[serde(default)]
    pub items_to_receive: Vec<OfferItem>,
    /// Indicates the account binded with the api key requested this trade
    #[serde(default)]
    pub is_our_offer: bool,
    #[serde(default)]
    pub time_created: i64,
    #[serde(default)]
    pub time_updated: i64,
    /// Historical number of the trade.
    /// Used find the new generated asset ids after the trade is completed.
    ///
    /// **Note**: Shows up only after the trade has been completed, and also can be found on the [TradeHistory_Trade]
    /// from the TradeHistory endpoint.
    pub tradeid: Option<String>,
    #[serde(default)]
    pub from_real_time_trade: bool,
    /// Unix timestamp of when the trade hold period is supposed to be over for this trade offer.
    /// 0 when there is no hold, also when Steam leaves it out.
    #[serde(default)]
    pub escrow_end_date: i64,
    pub confirmation_method: Option<EnumOrUnknown<ETradeOfferConfirmationMethod>>,
    pub partner_confirmation_method: Option<EnumOrUnknown<ETradeOfferConfirmationMethod>>,
    /// Defaults to OK (1) in the schema.
    pub eresult: Option<EnumOrUnknown<EResult>>,
    /// Trade protection. Seen live (2026-10-20): `true` on every CS2 offer, in any state.
    pub delay_settlement: Option<bool>,
    /// Unix time its items' trade protection ends. Seen live (2026-10-20, 2026-10-22): on an accepted CS2 offer, the
    /// same time as its trade's [`TradeHistory_Trade::time_settlement`]; 0 on offers that weren't accepted.
    pub settlement_date: Option<i64>,
}

impl TradeOffer_Trade {
    /// Whether the offer's items can't be trusted, see [Self::malformed_items].
    ///
    /// The offer's item lists are then incomplete: don't accept it based on the items that could be read.
    pub fn has_malformed_items(&self) -> bool {
        self.malformed_items().is_some()
    }

    /// Why the offer's items can't be trusted: both item lists are absent or empty, or the first entry that is not
    /// a usable item. `None` for a clean offer, including a gift (only one side has items).
    ///
    /// The limit, the same as node-steam-tradeoffer-manager's: when Steam leaves out only one list, the offer reads
    /// as a gift, since an absent list and an empty one can't be told apart. Check that a gift is expected.
    pub fn malformed_items(&self) -> Option<MalformedItems<'_>> {
        if self.items_to_give.is_empty() && self.items_to_receive.is_empty() {
            return Some(MalformedItems::NoItems);
        }

        self.items_to_give
            .iter()
            .chain(self.items_to_receive.iter())
            .find_map(|item| match item {
                OfferItem::Asset(_) => None,
                OfferItem::Malformed(malformed) => Some(MalformedItems::Item(malformed)),
            })
    }

    /// The items to give, or why they can't be trusted: never a list that leaves out an entry Steam sent. Empty for
    /// a gift received, or when Steam left the list out (see [Self::malformed_items]); an error when the offer has
    /// no items at all.
    pub fn assets_to_give(&self) -> Result<Vec<&CEcon_Asset>, MalformedItems<'_>> {
        self.assets_of(&self.items_to_give)
    }

    /// The items to receive, or why they can't be trusted: never a list that leaves out an entry Steam sent. Empty
    /// for a gift sent, or when Steam left the list out (see [Self::malformed_items]); an error when the offer has
    /// no items at all.
    pub fn assets_to_receive(&self) -> Result<Vec<&CEcon_Asset>, MalformedItems<'_>> {
        self.assets_of(&self.items_to_receive)
    }

    fn assets_of<'a>(&'a self, items: &'a [OfferItem]) -> Result<Vec<&'a CEcon_Asset>, MalformedItems<'a>> {
        if self.items_to_give.is_empty() && self.items_to_receive.is_empty() {
            return Err(MalformedItems::NoItems);
        }

        items
            .iter()
            .map(|item| match item {
                OfferItem::Asset(asset) => Ok(asset),
                OfferItem::Malformed(malformed) => Err(MalformedItems::Item(malformed)),
            })
            .collect()
    }
}

/// Why an offer's items can't be trusted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MalformedItems<'a> {
    /// Neither side has an item, which no real offer has. Steam sends offers like that, sometimes for good ("some
    /// offers can be empty just forever"); node-steam-tradeoffer-manager counts them as malformed (`offerMalformed`
    /// in `lib/helpers.js`). Absent and empty lists can't be told apart here.
    NoItems,
    /// An entry that is not a usable item.
    Item(&'a MalformedItem),
}

impl std::fmt::Display for MalformedItems<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MalformedItems::NoItems => f.write_str("the offer has no items on either side"),
            MalformedItems::Item(item) => write!(f, "an item could not be read: {}", item.reason),
        }
    }
}

/// An entry of an offer's item list.
///
/// node-steam-tradeoffer-manager silently drops items without appid, contextid or assetid (`sanitizeRawOffer` and
/// `itemMalformed` in `lib/helpers.js`, since 2.12.2). Dropping them here would let a bot accept an offer whose item
/// list it never fully saw, and failing the parse would hide every other offer; so they are kept, and visible.
#[derive(Debug, Clone, PartialEq)]
pub enum OfferItem {
    Asset(CEcon_Asset),
    Malformed(MalformedItem),
}

impl OfferItem {
    pub fn asset(&self) -> Option<&CEcon_Asset> {
        match self {
            OfferItem::Asset(asset) => Some(asset),
            OfferItem::Malformed(_) => None,
        }
    }
}

/// An item list entry that could not be read as a [CEcon_Asset].
#[derive(Debug, Clone, PartialEq)]
pub struct MalformedItem {
    /// The entry as Steam sent it.
    pub raw: serde_json::Value,
    /// Why it was not read.
    pub reason: String,
}

impl<'de> Deserialize<'de> for OfferItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(deserializer)?;

        let parsed = CEcon_Asset::deserialize(&raw);
        let reason = match parsed {
            // Stricter than node-steam-tradeoffer-manager, which only rejects appid 0 (`"0"` strings are truthy in
            // JS); no real item has a 0 id, so 0 is malformed too.
            Ok(asset) if asset.appid == 0 => "`appid` is 0".to_string(),
            Ok(asset) if asset.contextid == 0 => "`contextid` is 0".to_string(),
            Ok(asset) if asset.assetid == 0 => "`assetid` is 0".to_string(),
            Ok(asset) => return Ok(OfferItem::Asset(asset)),
            Err(error) => error.to_string(),
        };

        Ok(OfferItem::Malformed(MalformedItem { raw, reason }))
    }
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_Asset {
    pub appid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub contextid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub assetid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: i64,
    /// 0 when Steam leaves it out, as for items without an instance.
    #[serde(default, with = "serde_with::rust::display_fromstr")]
    pub instanceid: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde(default)]
    pub missing: bool,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub est_usd: Option<i64>,
    pub currencyid: Option<u32>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// Represents the raw CEcon_GetTradeHistory_Response_Trade_Base
pub struct GetTradeHistoryResponse {
    pub response: CEcon_GetTradeHistory_Response_Trade_Intermediate,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_GetTradeHistory_Response_Trade_Intermediate {
    /// Only with `include_total`.
    pub total_trades: Option<u32>,
    #[serde(default)]
    pub more: bool,
    /// Empty if Steam leaves it out (the schema allows it).
    #[serde(default)]
    pub trades: Vec<TradeHistory_Trade>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A trade returned by GetTradeHistory
/// Known as CEcon_GetTradeHistory_Response_Trade
pub struct TradeHistory_Trade {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub steamid_other: u64,
    /// Unix epoch when the trade offer was completed, and turned into a trade.
    #[serde(default)]
    pub time_init: i64,
    pub time_escrow_end: Option<i64>,
    pub status: ETradeStatus,
    pub assets_received: Option<Vec<TradeHistory_TradedAsset>>,
    pub assets_given: Option<Vec<TradeHistory_TradedAsset>>,
    pub currency_received: Option<Vec<TradeHistory_TradedCurrency>>,
    pub currency_given: Option<Vec<TradeHistory_TradedCurrency>>,
    /// Unix time when the trade protection of the trade's items ends. Seen live (2026-10-20) on a CS2 trade; older
    /// trades have none.
    pub time_settlement: Option<i64>,
    /// Unix time of the last change. From the schema, not seen in a real response yet.
    pub time_mod: Option<i64>,
    /// The trade that reversed this one. From the schema, not seen in a real response yet.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub rollback_trade: Option<i64>,
    pub trade_auth: Option<TradeHistory_TradeAuthorization>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// How a trade was confirmed. From the schema, not seen in a real response yet.
/// Known as CEcon_GetTradeHistory_Response_Trade_Authorization
pub struct TradeHistory_TradeAuthorization {
    pub is_sender: Option<bool>,
    pub confirm_type: Option<i32>,
    /// Unix time.
    pub time_confirmed: Option<i64>,
    pub country: Option<String>,
    pub state: Option<String>,
    pub city: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub token_id: Option<u64>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// A traded item returned by GetTradeHistory
/// Known as  CEcon_GetTradeHistory_Response_Trade_TradedAsset
pub struct TradeHistory_TradedAsset {
    /// Absent until the trade has settled, and for failed trades.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub new_assetid: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub rollback_new_assetid: Option<i64>,
    /// Absent until the trade has settled, and for failed trades.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub new_contextid: Option<u32>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub rollback_new_contextid: Option<u32>,
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub contextid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub assetid: i64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: i64,
    /// 0 when Steam leaves it out, as for items without an instance.
    #[serde(default, with = "serde_with::rust::display_fromstr")]
    pub instanceid: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub amount: Option<i64>,
}

#[serde_as]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
/// Traded currency returned by GetTradeHistory. From the schema, not seen in a real response yet.
/// Known as CEcon_GetTradeHistory_Response_Trade_TradedCurrency
pub struct TradeHistory_TradedCurrency {
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub contextid: i64,
    pub currencyid: u32,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub fee_amount: Option<u64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub classid: Option<i64>,
    pub new_currencyid: Option<u32>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub new_contextid: Option<u32>,
    pub rollback_new_currencyid: Option<u32>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub rollback_new_contextid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
/// Base response for GetTradeOffersSummary
pub struct GetTradeOffersSummaryResponse {
    pub response: CEcon_GetTradeOffersSummary_Response,
}

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
/// Offer counts. Every field is optional in the schema; an absent count is read as 0.
pub struct CEcon_GetTradeOffersSummary_Response {
    #[serde(default)]
    pub pending_received_count: u32,
    #[serde(default)]
    pub new_received_count: u32,
    #[serde(default)]
    pub updated_received_count: u32,
    #[serde(default)]
    pub historical_received_count: u32,
    #[serde(default)]
    pub pending_sent_count: u32,
    #[serde(default)]
    pub newly_accepted_sent_count: u32,
    #[serde(default)]
    pub updated_sent_count: u32,
    #[serde(default)]
    pub historical_sent_count: u32,
    #[serde(default)]
    pub escrow_received_count: u32,
    #[serde(default)]
    pub escrow_sent_count: u32,
    #[serde(default)]
    pub provisional: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every fixture here is synthetic, built from the field names in `webui/service_econ.proto`, except
    // `history_from_2020`. No real response of today's Steam has been captured yet.

    fn offers(json: &str) -> CEcon_GetTradeOffers_Response {
        serde_json::from_str::<GetTradeOffersResponse>(json).unwrap().response
    }

    fn asset(id: &str) -> String {
        format!(
            r#"{{"appid":730,"contextid":"2","assetid":"{}","classid":"310776560","instanceid":"302028390","amount":"1","missing":false,"est_usd":"12"}}"#,
            id
        )
    }

    #[test]
    fn full_offer() {
        let json = format!(
            r#"{{"response":{{"trade_offers_received":[{{
                "tradeofferid":"6000000000","accountid_other":79925588,"message":"hi","expiration_time":1700000000,
                "trade_offer_state":2,"items_to_give":[{}],"items_to_receive":[{}],"is_our_offer":false,
                "time_created":1690000000,"time_updated":1690000001,"tradeid":"3622543526924228084",
                "from_real_time_trade":false,"escrow_end_date":0,"confirmation_method":2,
                "partner_confirmation_method":1,"eresult":1,"delay_settlement":true,"settlement_date":1700600000
            }}],"next_cursor":0}}}}"#,
            asset("15319724006"),
            asset("15319724007")
        );
        let response = offers(&json);
        let offer = &response.trade_offers_received.as_ref().unwrap()[0];

        assert_eq!(offer.tradeofferid, 6000000000);
        assert_eq!(offer.state, ETradeOfferState::Active);
        assert_eq!(offer.expiration_time, Some(1700000000));
        assert_eq!(
            offer.confirmation_method,
            Some(ETradeOfferConfirmationMethod::MobileApp.into())
        );
        assert_eq!(
            offer.partner_confirmation_method,
            Some(ETradeOfferConfirmationMethod::Email.into())
        );
        assert_eq!(offer.eresult, Some(EResult::OK.into()));
        assert_eq!(offer.delay_settlement, Some(true));
        assert_eq!(offer.settlement_date, Some(1700600000));
        assert!(!offer.has_malformed_items());

        let given = offer.assets_to_give().unwrap();
        assert_eq!(given.len(), 1);
        assert_eq!(given[0].assetid, 15319724006);
        assert_eq!(given[0].amount, Some(1));
        assert_eq!(given[0].est_usd, Some(12));
        assert_eq!(offer.assets_to_receive().unwrap()[0].assetid, 15319724007);

        assert_eq!(response.next_cursor, Some(0));
        assert_eq!(response.next_page_cursor(), None);
    }

    #[test]
    fn minimal_offer() {
        let response = offers(
            r#"{"response":{"trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":3}]}}"#,
        );
        let offer = &response.trade_offers_sent.as_ref().unwrap()[0];

        assert_eq!(offer.state, ETradeOfferState::Accepted);
        assert_eq!(offer.message, "");
        assert_eq!(offer.expiration_time, None);
        assert!(offer.items_to_give.is_empty() && offer.items_to_receive.is_empty());
        assert_eq!(offer.malformed_items(), Some(MalformedItems::NoItems));
        assert!(!offer.is_our_offer);
        assert_eq!(
            (offer.time_created, offer.time_updated, offer.escrow_end_date),
            (0, 0, 0)
        );
        assert_eq!(offer.tradeid, None);
        assert_eq!(offer.confirmation_method, None);
        assert_eq!(offer.eresult, None);
        assert_eq!(offer.settlement_date, None);
        assert!(response.trade_offers_received.is_none());
        assert_eq!(response.next_cursor, None);
        assert_eq!(response.next_page_cursor(), None);
    }

    #[test]
    fn minimal_item() {
        let response = offers(
            r#"{"response":{"trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2,
                "items_to_give":[{"appid":753,"contextid":"6","assetid":"77","classid":"88"}]}]}}"#,
        );
        let asset = response.trade_offers_sent.unwrap()[0].assets_to_give().unwrap()[0].clone();

        assert_eq!(
            asset,
            CEcon_Asset {
                appid: 753,
                contextid: 6,
                assetid: 77,
                classid: 88,
                instanceid: 0,
                amount: None,
                missing: false,
                est_usd: None,
                currencyid: None,
            }
        );
    }

    #[test]
    fn next_cursor() {
        let response = offers(r#"{"response":{"trade_offers_received":[],"next_cursor":100}}"#);
        assert_eq!(response.next_page_cursor(), Some(100));
    }

    #[test]
    fn unknown_enum_values() {
        let response = offers(
            r#"{"response":{"trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":99,
                "confirmation_method":7,"partner_confirmation_method":0,"eresult":123456}]}}"#,
        );
        let offer = &response.trade_offers_sent.as_ref().unwrap()[0];

        assert_eq!(offer.state, EnumOrUnknown::Unknown(99));
        assert!(offer.state != ETradeOfferState::Active);
        assert_eq!(offer.state.known(), None);
        assert_eq!(offer.confirmation_method, Some(EnumOrUnknown::Unknown(7)));
        assert_eq!(
            offer.partner_confirmation_method,
            Some(ETradeOfferConfirmationMethod::Invalid.into())
        );
        assert_eq!(offer.eresult, Some(EnumOrUnknown::Unknown(123456)));
    }

    #[test]
    fn malformed_items_are_kept_and_visible() {
        let json = format!(
            r#"{{"response":{{"trade_offers_received":[
                {{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2,
                  "items_to_give":[{}],
                  "items_to_receive":[{},{{"appid":730,"contextid":"2","classid":"1"}},
                                      {{"appid":0,"contextid":"2","assetid":"5","classid":"1"}}]}},
                {{"tradeofferid":"3","accountid_other":4,"trade_offer_state":2,"items_to_receive":[{}]}}
            ]}}}}"#,
            asset("10"),
            asset("11"),
            asset("12")
        );
        let response = offers(&json);
        let received = response.trade_offers_received.unwrap();

        let bad = &received[0];
        assert!(bad.has_malformed_items());
        assert_eq!(bad.items_to_receive.len(), 3, "nothing is dropped");
        assert_eq!(bad.assets_to_give().unwrap()[0].assetid, 10);

        let first_malformed = match bad.assets_to_receive() {
            Err(MalformedItems::Item(item)) => item,
            other => panic!("{:?}", other),
        };
        assert!(first_malformed.reason.contains("assetid"), "{}", first_malformed.reason);
        assert_eq!(first_malformed.raw["classid"], "1");
        assert_eq!(bad.malformed_items(), Some(MalformedItems::Item(first_malformed)));
        match &bad.items_to_receive[2] {
            OfferItem::Malformed(item) => assert_eq!(item.reason, "`appid` is 0"),
            other => panic!("{:?}", other),
        }

        let good = &received[1];
        assert!(!good.has_malformed_items());
        assert_eq!(good.assets_to_receive().unwrap()[0].assetid, 12);
    }

    #[test]
    fn offer_without_items_is_malformed() {
        let response = offers(
            r#"{"response":{"trade_offers_received":[
                {"tradeofferid":"1","accountid_other":2,"trade_offer_state":2},
                {"tradeofferid":"2","accountid_other":2,"trade_offer_state":2,"items_to_give":[],"items_to_receive":[]},
                {"tradeofferid":"3","accountid_other":2,"trade_offer_state":2,"items_to_receive":[]}
            ]}}"#,
        );

        for offer in response.trade_offers_received.as_ref().unwrap() {
            assert!(offer.has_malformed_items(), "{}", offer.tradeofferid);
            assert_eq!(offer.malformed_items(), Some(MalformedItems::NoItems));
            assert_eq!(offer.assets_to_give(), Err(MalformedItems::NoItems));
            assert_eq!(offer.assets_to_receive(), Err(MalformedItems::NoItems));
        }
        assert_eq!(
            MalformedItems::NoItems.to_string(),
            "the offer has no items on either side"
        );
    }

    #[test]
    fn gifts_are_not_malformed() {
        let json = format!(
            r#"{{"response":{{"trade_offers_received":[
                {{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2,"items_to_receive":[{}]}},
                {{"tradeofferid":"2","accountid_other":2,"trade_offer_state":2,"items_to_give":[{}],
                  "items_to_receive":[]}}
            ]}}}}"#,
            asset("10"),
            asset("11")
        );
        let response = offers(&json);
        let received = response.trade_offers_received.unwrap();

        let gift_received = &received[0];
        assert!(!gift_received.has_malformed_items());
        assert_eq!(gift_received.assets_to_give(), Ok(vec![]));
        assert_eq!(gift_received.assets_to_receive().unwrap()[0].assetid, 10);

        let gift_sent = &received[1];
        assert_eq!(gift_sent.malformed_items(), None);
        assert_eq!(gift_sent.assets_to_give().unwrap()[0].assetid, 11);
        assert_eq!(gift_sent.assets_to_receive(), Ok(vec![]));
    }

    #[test]
    fn unreadable_offers_are_kept_aside_and_visible() {
        let json = format!(
            r#"{{"response":{{
                "trade_offers_sent":[
                  {{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2,"items_to_give":[{}]}},
                  {{"tradeofferid":"3","trade_offer_state":2,"items_to_give":[{}]}}
                ],
                "trade_offers_received":[
                  {{"accountid_other":2,"trade_offer_state":2}},
                  {{"tradeofferid":"5","accountid_other":2}},
                  {{"tradeofferid":6,"accountid_other":2,"trade_offer_state":2}},
                  null,
                  {{"tradeofferid":"7","accountid_other":2,"trade_offer_state":2,"items_to_receive":[{}]}}
                ],
                "next_cursor":0}}}}"#,
            asset("10"),
            asset("11"),
            asset("12")
        );
        let response = offers(&json);

        assert!(response.has_malformed_offers());
        let sent = response.trade_offers_sent.as_ref().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].tradeofferid, 1);
        assert_eq!(response.malformed_offers_sent.len(), 1);
        let no_partner = &response.malformed_offers_sent[0];
        assert!(no_partner.reason.contains("accountid_other"), "{}", no_partner.reason);
        assert_eq!(no_partner.tradeofferid(), Some(3));
        assert_eq!(no_partner.raw["items_to_give"][0]["assetid"], "11");

        let received = response.trade_offers_received.as_ref().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].tradeofferid, 7);
        let reasons: Vec<(Option<u64>, &str)> = response
            .malformed_offers_received
            .iter()
            .map(|offer| (offer.tradeofferid(), offer.reason.as_str()))
            .collect();
        assert_eq!(reasons.len(), 4, "{:?}", reasons);
        assert_eq!(reasons[0].0, None);
        assert!(reasons[0].1.contains("tradeofferid"), "{:?}", reasons);
        assert_eq!(reasons[1].0, Some(5));
        assert!(reasons[1].1.contains("trade_offer_state"), "{:?}", reasons);
        // a number where Steam sends a string
        assert_eq!(reasons[2].0, Some(6));
        assert_eq!(reasons[3].0, None);

        let clean = offers(r#"{"response":{"trade_offers_sent":[],"next_cursor":0}}"#);
        assert!(!clean.has_malformed_offers());
        assert_eq!(clean.trade_offers_sent.map(|offers| offers.len()), Some(0));
        assert!(clean.trade_offers_received.is_none());
    }

    #[test]
    fn one_offer() {
        let one = |json: &str| serde_json::from_str::<GetTradeOfferResponse>(json).unwrap().response;

        let found = one(r#"{"response":{"offer":{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2}}}"#);
        assert_eq!(found.offer.map(|offer| offer.tradeofferid), Some(1));
        assert!(found.malformed_offer.is_none());

        // no such offer
        let none = one(r#"{"response":{}}"#);
        assert!(none.offer.is_none() && none.malformed_offer.is_none());

        let unreadable = one(r#"{"response":{"offer":{"tradeofferid":"3","trade_offer_state":2}}}"#);
        assert!(unreadable.offer.is_none());
        let malformed = unreadable.malformed_offer.unwrap();
        assert_eq!(malformed.tradeofferid(), Some(3));
        assert!(malformed.reason.contains("accountid_other"), "{}", malformed.reason);
    }

    #[test]
    fn enums_compare_with_enum_or_unknown_both_ways() {
        let active: EnumOrUnknown<ETradeOfferState> = ETradeOfferState::Active.into();

        assert!(active == ETradeOfferState::Active);
        assert!(ETradeOfferState::Active == active);
        assert!(ETradeOfferState::Accepted != active);
        assert!(ETradeOfferState::Active != EnumOrUnknown::Unknown(2));
        assert!(ETradeOfferConfirmationMethod::Email == EnumOrUnknown::from(ETradeOfferConfirmationMethod::Email));
        assert!(EResult::OK == EnumOrUnknown::from(EResult::OK));

        // `match` needs the variant, or `known()`
        assert!(matches!(active, EnumOrUnknown::Known(ETradeOfferState::Active)));
        assert!(matches!(active.known(), Some(ETradeOfferState::Active)));
    }

    #[test]
    fn trade_status() {
        let status = |json: &str| serde_json::from_str::<ETradeStatus>(json).unwrap();

        assert_eq!(status("3"), ETradeStatus::Complete);
        assert_eq!(status("11"), ETradeStatus::EscrowRollback);
        assert_eq!(status("42"), ETradeStatus::Unknown(42));
        for value in -1..13 {
            assert_eq!(i32::from(ETradeStatus::from(value)), value);
        }
    }

    // From steam-trading's tests (commit 818682b, 2020-11), apparently a real response; trimmed to three trades.
    #[test]
    fn history_from_2020() {
        let json = r#"{"response":{"more":true,"trades":[
          {"tradeid":"3622543526924228084","steamid_other":"76561198040191316","time_init":1603998438,"status":3,
           "assets_given":[{"appid":730,"contextid":"2","assetid":"15319724006","amount":"1","classid":"3035569977",
             "instanceid":"302028390","new_assetid":"19793871926","new_contextid":"2"}]},
          {"tradeid":"2289455842905057389","steamid_other":"76561198994791561","time_init":1582942255,
           "time_escrow_end":1584238255,"status":3,
           "assets_given":[{"appid":730,"contextid":"2","assetid":"16832065568","amount":"1","classid":"1",
             "instanceid":"2","new_assetid":"18074934023","new_contextid":"2"}]},
          {"tradeid":"2022547174628335361","steamid_other":"76561197976600825","time_init":1515644947,"status":3,
           "assets_received":[{"appid":730,"contextid":"2","assetid":"1667881814169014779","amount":"1",
             "classid":"1","instanceid":"2","new_assetid":"1827766562939536102","new_contextid":"2"}]}
        ]}}"#;
        let response = serde_json::from_str::<GetTradeHistoryResponse>(json).unwrap().response;

        assert!(response.more);
        assert_eq!(response.total_trades, None);
        assert_eq!(response.trades.len(), 3);
        let first = &response.trades[0];
        assert_eq!(first.status, ETradeStatus::Complete);
        let asset = &first.assets_given.as_ref().unwrap()[0];
        assert_eq!(asset.new_assetid, Some(19793871926));
        assert_eq!(asset.new_contextid, Some(2));
        assert_eq!(asset.rollback_new_assetid, None);
        assert_eq!(response.trades[1].time_escrow_end, Some(1584238255));
        assert_eq!(
            response.trades[2].assets_received.as_ref().unwrap()[0].new_assetid,
            Some(1827766562939536102)
        );
    }

    #[test]
    fn history_failed_trade_and_trade_protection_fields() {
        let json = r#"{"response":{"total_trades":2,"more":false,"trades":[
          {"tradeid":"1","steamid_other":"76561198040191316","time_init":1,"status":4,
           "assets_given":[{"appid":730,"contextid":"2","assetid":"10","amount":"1","classid":"20"}]},
          {"tradeid":"2","steamid_other":"76561198040191316","time_init":2,"status":12,
           "time_settlement":1700600000,"time_mod":1700000100,"rollback_trade":"3",
           "trade_auth":{"is_sender":true,"confirm_type":2,"time_confirmed":1700000050,"country":"BR",
             "state":"SP","city":"Somewhere","token_id":"18446744073709551615"},
           "assets_received":[{"appid":730,"contextid":"2","assetid":"11","amount":"1","classid":"21",
             "instanceid":"0","new_assetid":"30","new_contextid":"2","rollback_new_assetid":"40",
             "rollback_new_contextid":"2"}],
           "currency_given":[{"appid":440,"contextid":"3","currencyid":5,"amount":"100","fee_amount":"2",
             "classid":"7","new_currencyid":6,"new_contextid":"3","rollback_new_currencyid":8,
             "rollback_new_contextid":"3"}],
           "currency_received":[{"appid":440,"contextid":"3","currencyid":5}]}
        ]}}"#;
        let response = serde_json::from_str::<GetTradeHistoryResponse>(json).unwrap().response;
        assert_eq!(response.total_trades, Some(2));

        let failed = &response.trades[0];
        assert_eq!(failed.status, ETradeStatus::Failed);
        let asset = &failed.assets_given.as_ref().unwrap()[0];
        assert_eq!((asset.new_assetid, asset.new_contextid), (None, None));
        assert_eq!(asset.instanceid, 0);
        assert_eq!(failed.trade_auth, None);
        assert_eq!(failed.rollback_trade, None);

        let rolled_back = &response.trades[1];
        assert_eq!(rolled_back.status, ETradeStatus::Unknown(12));
        assert_eq!(rolled_back.time_settlement, Some(1700600000));
        assert_eq!(rolled_back.time_mod, Some(1700000100));
        assert_eq!(rolled_back.rollback_trade, Some(3));
        let auth = rolled_back.trade_auth.as_ref().unwrap();
        assert_eq!(auth.is_sender, Some(true));
        assert_eq!(auth.token_id, Some(u64::MAX));
        let asset = &rolled_back.assets_received.as_ref().unwrap()[0];
        assert_eq!(asset.rollback_new_assetid, Some(40));
        assert_eq!(asset.rollback_new_contextid, Some(2));

        let given = &rolled_back.currency_given.as_ref().unwrap()[0];
        assert_eq!(given.amount, Some(100));
        assert_eq!(given.fee_amount, Some(2));
        assert_eq!(given.rollback_new_contextid, Some(3));
        let received = &rolled_back.currency_received.as_ref().unwrap()[0];
        assert_eq!((received.amount, received.new_currencyid), (None, None));
    }

    #[test]
    fn history_without_trades() {
        let response = serde_json::from_str::<GetTradeHistoryResponse>(r#"{"response":{}}"#)
            .unwrap()
            .response;
        assert!(!response.more);
        assert!(response.trades.is_empty());
    }

    #[test]
    fn trade_offers_summary() {
        let json = r#"{"response":{"pending_received_count":3,"new_received_count":1,"escrow_sent_count":2}}"#;
        let summary = serde_json::from_str::<GetTradeOffersSummaryResponse>(json)
            .unwrap()
            .response;

        assert_eq!(
            summary,
            CEcon_GetTradeOffersSummary_Response {
                pending_received_count: 3,
                new_received_count: 1,
                escrow_sent_count: 2,
                ..Default::default()
            }
        );
    }
}

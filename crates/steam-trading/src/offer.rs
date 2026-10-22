//! Trade offers: the one to send ([`OfferRequest`]), and those Steam lists ([`Offer`]).

use std::convert::TryFrom;

use steam_language::enums::ETradeOfferState;
use steamid_parser::SteamID;
use tappet::response_types::CEcon_Asset;
use tappet::response_types::EnumOrUnknown;
use tappet::response_types::OfferItem;
use tappet::response_types::TradeOffer_Trade;

use crate::types::trade_offer_web::TradeOfferCreateResponse;
use crate::Asset;
use crate::AssetId;
use crate::InventoryContext;
use crate::OfferId;
use crate::TradeError;
use crate::TradeId;
use crate::TradeLink;
use crate::TRADE_MAX_ITEMS;

/// An offer to send with [`SteamTradeManager::create_offer`](crate::SteamTradeManager::create_offer) or
/// [`SteamTradeManager::send`](crate::SteamTradeManager::send).
///
/// ```
/// use steam_trading::{Asset, AssetId, InventoryContext, OfferRequest, TradeLink};
///
/// let link = TradeLink::parse("https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn").unwrap();
/// let request = OfferRequest::to(link)
///     .give(Asset::new(InventoryContext::CS2, AssetId(1)))
///     .take(Asset::new(InventoryContext::CS2, AssetId(2)))
///     .message("code 7F3A");
/// assert_eq!(request.gives().len(), 1);
/// ```
///
/// `create_offer` checks it before anything is sent: [`TradeError::EmptyOffer`] without an item,
/// [`TradeError::TooManyItems`] with more than [`TRADE_MAX_ITEMS`] on a side, [`TradeError::InvalidAmount`] for an
/// amount of 0.
#[derive(Debug, Clone, PartialEq)]
pub struct OfferRequest {
    partner: TradeLink,
    gives: Vec<(Asset, u32)>,
    takes: Vec<(Asset, u32)>,
    message: String,
}

impl OfferRequest {
    /// An offer to the owner of `partner`, with no item and no message yet.
    pub fn to(partner: TradeLink) -> Self {
        Self {
            partner,
            gives: Vec::new(),
            takes: Vec::new(),
            message: String::new(),
        }
    }

    /// Gives `asset`, one of it.
    pub fn give(self, asset: Asset) -> Self {
        self.give_amount(asset, 1)
    }

    /// Gives `amount` of a stackable `asset`.
    pub fn give_amount(mut self, asset: Asset, amount: u32) -> Self {
        self.gives.push((asset, amount));
        self
    }

    /// Asks for `asset`, one of it.
    pub fn take(self, asset: Asset) -> Self {
        self.take_amount(asset, 1)
    }

    /// Asks for `amount` of a stackable `asset`, as node-steam-tradeoffer-manager sends the caller's amount
    /// (`lib/classes/TradeOffer.js:328,359`).
    pub fn take_amount(mut self, asset: Asset, amount: u32) -> Self {
        self.takes.push((asset, amount));
        self
    }

    /// The message the partner sees with the offer.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// The trade link of the account the offer goes to.
    pub fn partner(&self) -> &TradeLink {
        &self.partner
    }

    /// The items given, with their amounts.
    pub fn gives(&self) -> &[(Asset, u32)] {
        &self.gives
    }

    /// The items asked for, with their amounts.
    pub fn takes(&self) -> &[(Asset, u32)] {
        &self.takes
    }

    /// The message set with [`OfferRequest::message`]. Not `message()`, which is the builder's name.
    pub fn message_text(&self) -> &str {
        &self.message
    }

    /// Whether the offer can be sent, see [`OfferRequest`].
    pub(crate) fn check(&self) -> Result<(), TradeError> {
        if self.gives.is_empty() && self.takes.is_empty() {
            return Err(TradeError::EmptyOffer);
        }
        let max = usize::from(TRADE_MAX_ITEMS);
        if self.gives.len() > max || self.takes.len() > max {
            return Err(TradeError::TooManyItems);
        }
        if self.gives.iter().chain(&self.takes).any(|&(_, amount)| amount == 0) {
            return Err(TradeError::InvalidAmount);
        }
        Ok(())
    }
}

/// A trade offer, as Steam lists it (`GetTradeOffers`, `GetTradeOffer`).
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    /// Its id (`tradeofferid`).
    pub id: OfferId,
    /// The other account (`accountid_other`).
    pub partner: SteamID,
    /// Whether this account sent it.
    pub is_ours: bool,
    /// Its state (`trade_offer_state`).
    pub state: OfferState,
    /// The items this account gives (`items_to_give`), those that could be read.
    pub give: Vec<OfferedItem>,
    /// The items this account receives (`items_to_receive`), those that could be read.
    pub take: Vec<OfferedItem>,
    /// Whether an entry of either list couldn't be read: `give` and `take` then leave it out, so don't act on them.
    /// An offer with no item on either side has both empty, and this `false`.
    pub has_malformed_items: bool,
    /// The sender's message, empty without one.
    pub message: String,
    /// Unix time.
    pub created: i64,
    /// Unix time of the last change.
    pub updated: i64,
    /// Unix time when it expires (or expired).
    pub expires: Option<i64>,
    /// Unix time when its trade hold ends (`escrow_end_date`), `None` without one.
    pub escrow_end: Option<i64>,
    /// The trade it became, once accepted (`tradeid`).
    pub trade_id: Option<TradeId>,
    /// Unix time its items' trade protection ends (`settlement_date`), `None` when Steam sends 0. Seen live on an
    /// accepted CS2 offer (2026-10-20), equal to its trade's [`Trade::protected_until`](crate::Trade::protected_until)
    /// (`time_settlement`); 0 on the offers that weren't accepted.
    pub protected_until: Option<i64>,
}

/// An offer's state, Steam's `ETradeOfferState`, and the values this version doesn't know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OfferState {
    Invalid,
    /// Waiting for the partner.
    Active,
    /// The partner accepted it, and the items were exchanged.
    Accepted,
    /// The partner made a counter offer.
    Countered,
    /// It expired without an answer.
    Expired,
    /// The sender canceled it.
    Canceled,
    /// The partner declined it.
    Declined,
    /// An item of it is no longer available.
    InvalidItems,
    /// Waiting for the sender's confirmation (mobile or email).
    CreatedNeedsConfirmation,
    /// Canceled from the confirmation.
    CanceledBySecondFactor,
    /// Accepted, and held (escrow).
    InEscrow,
    /// The raw value.
    Unknown(i32),
}

impl OfferState {
    /// The state Steam sent.
    pub(crate) fn of(state: EnumOrUnknown<ETradeOfferState>) -> Self {
        match state {
            EnumOrUnknown::Known(ETradeOfferState::Invalid) => OfferState::Invalid,
            EnumOrUnknown::Known(ETradeOfferState::Active) => OfferState::Active,
            EnumOrUnknown::Known(ETradeOfferState::Accepted) => OfferState::Accepted,
            EnumOrUnknown::Known(ETradeOfferState::Countered) => OfferState::Countered,
            EnumOrUnknown::Known(ETradeOfferState::Expired) => OfferState::Expired,
            EnumOrUnknown::Known(ETradeOfferState::Canceled) => OfferState::Canceled,
            EnumOrUnknown::Known(ETradeOfferState::Declined) => OfferState::Declined,
            EnumOrUnknown::Known(ETradeOfferState::InvalidItems) => OfferState::InvalidItems,
            EnumOrUnknown::Known(ETradeOfferState::CreatedNeedsConfirmation) => OfferState::CreatedNeedsConfirmation,
            EnumOrUnknown::Known(ETradeOfferState::CanceledBySecondFactor) => OfferState::CanceledBySecondFactor,
            EnumOrUnknown::Known(ETradeOfferState::InEscrow) => OfferState::InEscrow,
            EnumOrUnknown::Known(other) => OfferState::Unknown(other as i32),
            EnumOrUnknown::Unknown(raw) => OfferState::Unknown(raw),
        }
    }
}

/// An item of an offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OfferedItem {
    /// Where it is: in its owner's inventory, with the owner's asset id from before the trade.
    pub asset: Asset,
    /// Its kind. It stays the same when the item is traded.
    pub classid: u64,
    /// With `classid`, its description (`instanceid`).
    pub instanceid: u64,
    /// How many of it, more than 1 for a stackable item.
    pub amount: u32,
    /// Whether it left its owner's inventory: Steam's `missing`, seen live `true` on the items of an accepted offer.
    pub missing: bool,
}

impl OfferedItem {
    /// The item Steam sent, or why it can't be one.
    pub(crate) fn of(item: &CEcon_Asset) -> Result<Self, String> {
        let context = InventoryContext::new(number(item.appid, "appid")?, number(item.contextid, "contextid")?);
        Ok(Self {
            asset: Asset::new(context, AssetId(number(item.assetid, "assetid")?)),
            classid: number(item.classid, "classid")?,
            instanceid: number(item.instanceid, "instanceid")?,
            amount: number(item.amount.ok_or("no `amount`")?, "amount")?,
            missing: item.missing,
        })
    }
}

/// `value` as `T`, or why it isn't one.
pub(crate) fn number<T: TryFrom<i64>>(value: i64, name: &str) -> Result<T, String> {
    T::try_from(value).map_err(|_| format!("`{}` is out of range: {}", name, value))
}

impl Offer {
    /// The offer Steam sent, or why it can't be one. An offer whose own fields can't be read is an error; an item that
    /// can't be read is left out, and noted in `has_malformed_items`.
    pub(crate) fn of(offer: TradeOffer_Trade) -> Result<Self, String> {
        let account_id = u32::try_from(offer.accountid_other)
            .map_err(|_| format!("`accountid_other` is out of range: {}", offer.accountid_other))?;
        let trade_id = match offer.tradeid.as_deref() {
            Some(id) => Some(id.parse().map_err(|_| format!("`tradeid` isn't a number: {:?}", id))?),
            None => None,
        };
        let (give, malformed_give) = offered_items(&offer.items_to_give);
        let (take, malformed_take) = offered_items(&offer.items_to_receive);
        Ok(Self {
            id: OfferId(offer.tradeofferid),
            partner: SteamID::from_account_id(account_id),
            is_ours: offer.is_our_offer,
            state: OfferState::of(offer.state),
            give,
            take,
            has_malformed_items: malformed_give || malformed_take,
            message: offer.message,
            created: offer.time_created,
            updated: offer.time_updated,
            expires: offer.expiration_time,
            escrow_end: Some(offer.escrow_end_date).filter(|&end| end != 0),
            trade_id,
            protected_until: offer.settlement_date.filter(|&end| end != 0),
        })
    }
}

/// The items of `items` that can be read, and whether one couldn't.
fn offered_items(items: &[OfferItem]) -> (Vec<OfferedItem>, bool) {
    let mut malformed = false;
    let read = items
        .iter()
        .filter_map(|item| {
            let read = item.asset().and_then(|asset| OfferedItem::of(asset).ok());
            malformed |= read.is_none();
            read
        })
        .collect();
    (read, malformed)
}

/// Which offers [`SteamTradeManager::offers`](crate::SteamTradeManager::offers) lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferQuery {
    /// Offers this account sent.
    pub sent: bool,
    /// Offers this account received.
    pub received: bool,
    /// Only those waiting for an answer (Steam's `active_only`).
    pub active_only: bool,
}

impl OfferQuery {
    /// Sent and received, active only.
    pub const ACTIVE: Self = Self {
        sent: true,
        received: true,
        active_only: true,
    };
    /// Sent, active only.
    pub const ACTIVE_SENT: Self = Self {
        sent: true,
        received: false,
        active_only: true,
    };
    /// Received, active only.
    pub const ACTIVE_RECEIVED: Self = Self {
        sent: false,
        received: true,
        active_only: true,
    };
    /// Sent and received, in any state.
    pub const ALL: Self = Self {
        sent: true,
        received: true,
        active_only: false,
    };
}

/// What Steam waits for after a create or an accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// Nothing: the offer was sent, or accepted.
    Nothing,
    /// A mobile confirmation: [`SteamTradeManager::confirm_offers`](crate::SteamTradeManager::confirm_offers).
    MobileConfirmation,
    /// An email confirmation, which this crate can't give.
    EmailConfirmation,
}

impl Needs {
    /// An absent flag is read as `false`, as node-steam-tradeoffer-manager does: an answer with neither flag set is a
    /// sent or accepted offer (`lib/classes/TradeOffer.js:452-475`, `:611-615`). This deliberately reverses the
    /// earlier "assume a mobile confirmation when Steam doesn't say", which failed after a successful accept, e.g. of
    /// a gift. A mobile confirmation wins over an email one: it is the one this crate can give.
    pub(crate) fn of(response: &TradeOfferCreateResponse) -> Self {
        if response.needs_mobile_confirmation == Some(true) {
            Needs::MobileConfirmation
        } else if response.needs_email_confirmation == Some(true) {
            Needs::EmailConfirmation
        } else {
            Needs::Nothing
        }
    }
}

/// An offer [`SteamTradeManager::create_offer`](crate::SteamTradeManager::create_offer) created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedOffer {
    /// The new offer's id.
    pub id: OfferId,
    /// What it waits for before the partner sees it.
    pub needs: Needs,
}

/// Steam's answer to [`SteamTradeManager::post_accept`](crate::SteamTradeManager::post_accept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostedAccept {
    /// The trade the offer became, when Steam said (seen live with [`Needs::Nothing`]).
    pub trade_id: Option<TradeId>,
    /// What the accept waits for.
    pub needs: Needs,
}

/// An offer [`SteamTradeManager::accept`](crate::SteamTradeManager::accept) accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    /// The trade the offer became, from Steam's answer to the accept. `None` after a mobile confirmation: that answer
    /// has no trade id, and neither has the confirmation's; the offer has it then, as
    /// [`SteamTradeManager::offers`](crate::SteamTradeManager::offers) with [`OfferQuery::ALL`] and
    /// [`SteamTradeManager::offer`](crate::SteamTradeManager::offer) give it (both seen live).
    pub trade_id: Option<TradeId>,
}

/// What [`SteamTradeManager::confirm_offers`](crate::SteamTradeManager::confirm_offers) did, by offer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confirmed {
    /// Offers whose mobile confirmation was accepted.
    pub confirmed: Vec<OfferId>,
    /// Offers without a mobile confirmation among the account's: none was needed, it was given already, or it isn't
    /// there yet.
    pub not_found: Vec<OfferId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link() -> TradeLink {
        TradeLink::parse("https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn").unwrap()
    }

    fn cs2(id: u64) -> Asset {
        Asset::new(InventoryContext::CS2, AssetId(id))
    }

    fn with_items(gives: usize, takes: usize) -> OfferRequest {
        let mut request = OfferRequest::to(link());
        for id in 0..gives {
            request = request.give(cs2(id as u64 + 1));
        }
        for id in 0..takes {
            request = request.take(cs2(id as u64 + 1));
        }
        request
    }

    #[test]
    fn builder() {
        let request = OfferRequest::to(link())
            .give(cs2(1))
            .take(cs2(2))
            .take_amount(Asset::new(InventoryContext::STEAM, AssetId(3)), 500)
            .give_amount(cs2(4), 2)
            .message("code 7F3A");
        assert_eq!(request.partner(), &link());
        assert_eq!(request.gives(), &[(cs2(1), 1), (cs2(4), 2)]);
        assert_eq!(
            request.takes(),
            &[(cs2(2), 1), (Asset::new(InventoryContext::STEAM, AssetId(3)), 500)]
        );
        assert_eq!(request.message_text(), "code 7F3A");
        assert!(request.check().is_ok());
    }

    #[test]
    fn no_items() {
        assert!(matches!(with_items(0, 0).check(), Err(TradeError::EmptyOffer)));
    }

    #[test]
    fn one_side() {
        assert!(with_items(1, 0).check().is_ok());
        assert!(with_items(0, 1).check().is_ok());
    }

    #[test]
    fn item_limit() {
        assert!(with_items(255, 255).check().is_ok());
        assert!(matches!(with_items(256, 0).check(), Err(TradeError::TooManyItems)));
        assert!(matches!(with_items(0, 256).check(), Err(TradeError::TooManyItems)));
        assert!(matches!(with_items(1, 256).check(), Err(TradeError::TooManyItems)));
    }

    #[test]
    fn zero_amount() {
        for request in [
            with_items(1, 0).take_amount(cs2(9), 0),
            with_items(0, 1).give_amount(cs2(9), 0),
            OfferRequest::to(link()).give_amount(cs2(9), 0),
        ] {
            assert!(
                matches!(request.check(), Err(TradeError::InvalidAmount)),
                "{:?}",
                request
            );
        }
    }

    #[test]
    fn states() {
        let state = |raw: i32| OfferState::of(serde_json::from_str(&raw.to_string()).unwrap());
        assert_eq!(state(2), OfferState::Active);
        assert_eq!(state(3), OfferState::Accepted);
        assert_eq!(state(11), OfferState::InEscrow);
        assert_eq!(state(12), OfferState::Unknown(12));
        assert_eq!(state(0), OfferState::Unknown(0));
    }
}

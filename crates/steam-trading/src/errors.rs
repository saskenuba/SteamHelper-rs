#[cfg(feature = "cs2")]
use std::collections::HashSet;

use steam_language::enums::EResult;
use steam_mobile::errors::AuthError;
use steam_mobile::HttpError;
use tappet::errors::SteamAPIError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TradeError {
    #[error("`{0}`")]
    GeneralError(String),

    #[error(transparent)]
    TradeOfferError(#[from] OfferError),

    #[error(transparent)]
    TradeValidationError(#[from] OfferValidationError),

    #[error(transparent)]
    SteamAPIError(#[from] SteamAPIError),

    #[error(transparent)]
    ConfirmationError(#[from] ConfirmationError),

    #[error(transparent)]
    AuthError(#[from] AuthError),

    #[error(transparent)]
    InternalError(#[from] InternalError),

    /// The trade is not among the last 500 trades of the account's trade history.
    #[error("Trade `{0}` is not among the last 500 trades of the trade history.")]
    TradeNotInHistory(i64),

    /// An asset of the trade has no new asset id yet: the trade has not settled, or it failed.
    #[error("Asset `{assetid}` of trade `{tradeid}` has no new asset id: the trade has not settled, or it failed.")]
    TradeNotSettled { tradeid: i64, assetid: i64 },

    /// Not every `GetTradeOffers` page could be read, so no offers are returned rather than an incomplete list.
    /// `cursor` is the page that was not read.
    #[error("GetTradeOffers stopped at cursor `{cursor}`, offers would be missing: {reason}.")]
    TradeOffersIncomplete { cursor: u32, reason: String },

    /// Steam sent trade offers that could not be read (see tappet's `MalformedOffer`), so no offers are returned
    /// rather than a list without them. `tradeoffer_ids` holds each one's id, when it could be read; `reason` is why
    /// the first one could not.
    ///
    /// Like node-steam-tradeoffer-manager, every call fails while Steam keeps sending such an offer, so one offer can
    /// stop polling and [`SteamTradeManager::decline_received_offers`](crate::SteamTradeManager::decline_received_offers)
    /// for good. To get past it, decline it with
    /// [`SteamTradeManager::deny_offer`](crate::SteamTradeManager::deny_offer) if it was sent to this account, or
    /// cancel it with [`SteamTradeManager::cancel_offer`](crate::SteamTradeManager::cancel_offer) if this account sent
    /// it, using its id from `tradeoffer_ids`: those two never look the offer up. The ids don't say which side sent
    /// the offer. There is no such way for an offer whose id is `None`: it stays until it is no longer active, e.g.
    /// declined or cancelled on Steam's website.
    #[error("{} trade offer(s) could not be read, the first because: {reason}", .tradeoffer_ids.len())]
    MalformedOffers {
        tradeoffer_ids: Vec<Option<u64>>,
        reason: String,
    },

    /// Steam sent the trade offer, but it could not be read (see tappet's `MalformedOffer`).
    #[error("Trade offer `{tradeoffer_id}` could not be read: {reason}")]
    MalformedOffer { tradeoffer_id: u64, reason: String },

    /// The offer was accepted, but Steam waits for a confirmation by email, which this crate can't give. Confirm it
    /// from the email, or cancel it.
    #[error("Trade offer `{tradeoffer_id}` was accepted, but Steam waits for an email confirmation.")]
    NeedsEmailConfirmation { tradeoffer_id: u64 },

    /// The trade offer was created, but a later step failed. Cancel the offer with `tradeoffer_id`, or retry the
    /// confirmation.
    #[error("Trade offer `{tradeoffer_id}` was created, but confirming it failed: {source}")]
    OfferCreatedButNotConfirmed {
        tradeoffer_id: u64,
        #[source]
        source: Box<TradeError>,
    },

    /// CS2's game coordinator failed, e.g. the client stopped.
    #[cfg(feature = "cs2")]
    #[error(transparent)]
    GameCoordinatorError(steam_client::errors::ClientError),

    /// CS2's game coordinator has no current inventory of the account (see `Cs2::inventory`), so the items an accept
    /// brings could not be told from those already there. Nothing was accepted, or by `find_received_items` nothing
    /// was looked up: try again once the game coordinator sent the inventory (`Cs2Event::InventoryLoaded`).
    #[cfg(feature = "cs2")]
    #[error("CS2's game coordinator has no current inventory of the account; the offer was not accepted.")]
    Cs2InventoryUnavailable,

    /// The offer was accepted, but finding where its items went failed. The trade happened: `tradeid` is its id, as
    /// [`SteamTradeManager::accept_offer`](crate::SteamTradeManager::accept_offer) returns it (`None` after a mobile
    /// confirmation). `known_ids` are the game coordinator's item ids right before the accept: call
    /// [`SteamTradeManager::find_received_items`](crate::SteamTradeManager::find_received_items) with them later to
    /// find its items, adding the new ids other trades were found to bring since.
    #[cfg(feature = "cs2")]
    #[error("Trade offer `{tradeoffer_id}` was accepted (trade {tradeid:?}), but finding its items failed: {source}")]
    AcceptedButItemsNotFound {
        tradeoffer_id: u64,
        tradeid: Option<i64>,
        known_ids: HashSet<u64>,
        #[source]
        source: Box<TradeError>,
    },
}

impl TradeError {
    /// Whether Steam answered HTTP 429 Too Many Requests: its firewall does that to requests it takes for a bot's.
    /// Retry later, and more slowly.
    pub fn is_too_many_requests(&self) -> bool {
        match self {
            TradeError::InternalError(InternalError::MobileInternalError(
                steam_mobile::errors::InternalError::TooManyRequests { .. },
            )) => true,
            // Confirmations are fetched and accepted through steam-mobile, whose errors arrive as `AuthError`.
            TradeError::AuthError(AuthError::InternalError(steam_mobile::errors::InternalError::TooManyRequests {
                ..
            })) => true,
            TradeError::SteamAPIError(SteamAPIError::SteamHttpError(status)) => status == "429",
            TradeError::OfferCreatedButNotConfirmed { source, .. } => source.is_too_many_requests(),
            #[cfg(feature = "cs2")]
            TradeError::AcceptedButItemsNotFound { source, .. } => source.is_too_many_requests(),
            _ => false,
        }
    }
}

#[derive(Error, Debug)]
pub enum InternalError {
    /// Its URL has no query string.
    #[error(transparent)]
    NetworkError(HttpError),

    #[error(transparent)]
    MobileInternalError(#[from] steam_mobile::errors::InternalError),
}

/// `reqwest::Error` shows its URL in `Display` and `Debug`; a query may carry a secret, so it is removed, as
/// steam-mobile and tappet do. Some Steam Community URLs this crate requests have one: the inventory's
/// (`?l=english&count=…&start_assetid=…`, no secret), and the trade offer page's that feature `additional-checks` reads
/// (`?partner=…&token=…`, the partner's trade link token).
impl From<HttpError> for InternalError {
    fn from(mut error: HttpError) -> Self {
        if let Some(url) = error.url_mut() {
            url.set_query(None);
        }
        InternalError::NetworkError(error)
    }
}

#[derive(Error, Debug, PartialEq, Copy, Clone)]
pub enum TradelinkError {
    #[error("The Tradeoffer URL was not valid.")]
    Invalid,
}

#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
pub enum OfferValidationError {
    #[error(transparent)]
    TradelinkError(#[from] TradelinkError),

    #[error("`{0}`")]
    InvalidTrade(String),
}

#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
pub enum OfferError {
    #[error(
        "This trade offer is in an invalid state, and cannot be acted upon. Perhaps you are trying to cancel a trade \
         offer that was already canceled, or something similar."
    )]
    InvalidState,

    #[error(
        "This trade offer id could not be found. It may be already canceled or even accepted.Are you sure this is the \
         correct id?"
    )]
    NoMatch,

    #[error(
        "This response code suggests that one or more of the items in this trade offer does not exist in the \
         inventory from which it was requested."
    )]
    Revoked,

    #[error(
        "This suggests that the user receiving the trade offer recently activated his mobile SteamGuard and is under \
         the 7 day restriction period."
    )]
    SteamGuardRecentlyEnabled,

    #[error("General Failure: `{0}`")]
    GeneralFailure(String),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum ConfirmationError {
    #[error("Could not find the requested confirmation.")]
    NotFound,
    #[error("Could not find the requested confirmation, but offer was created. Trade offer id: `{0}`")]
    NotFoundButTradeCreated(u64),
}

pub fn tradeoffer_error_from_eresult(eresult: EResult) -> OfferError {
    match eresult {
        EResult::Revoked => OfferError::Revoked,
        EResult::InvalidState => OfferError::InvalidState,
        EResult::NoMatch => OfferError::NoMatch,
        e => OfferError::GeneralFailure(format!(
            "{}{}",
            "Please check: https://steamerrors.com/",
            &*serde_json::to_string(&e).unwrap()
        )),
    }
}

/// The error for a Steam `strError` message, from the EResult code in parentheses that Steam appends, e.g.
/// `There was an error accepting this trade offer. Please try again later. (28)`.
///
/// Localized messages may hold other parentheses, so the last `(N)` with a known EResult wins. `None` if there is no
/// such code.
pub fn error_from_strmessage(message: &str) -> Option<OfferError> {
    message.match_indices('(').rev().find_map(|(index_start, _)| {
        let after_start = &message[index_start + 1..];
        let number = &after_start[..after_start.find(')')?];

        serde_json::from_str::<EResult>(number.trim())
            .map(tradeoffer_error_from_eresult)
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_strmessage() {
        let error_message = "Something went wrong (26)";
        assert_eq!(error_from_strmessage(error_message).unwrap(), OfferError::Revoked)
    }

    #[test]
    fn error_strmessage_non_ascii() {
        assert_eq!(
            error_from_strmessage("Ocorreu um erro ao enviar a oferta de troca. Tente novamente mais tarde. (26)"),
            Some(OfferError::Revoked)
        );
        assert_eq!(
            error_from_strmessage("交易报价出错（请稍后再试）。 (42)"),
            Some(OfferError::NoMatch)
        );
        assert_eq!(error_from_strmessage("ошибка (11) ä"), Some(OfferError::InvalidState));
    }

    #[test]
    fn error_strmessage_closing_before_opening() {
        assert_eq!(error_from_strmessage(") wrong order ("), None);
        assert_eq!(error_from_strmessage("a) b (26"), None);
        assert_eq!(error_from_strmessage("a) b (26)"), Some(OfferError::Revoked));
    }

    #[test]
    fn error_strmessage_no_known_code() {
        assert_eq!(error_from_strmessage(""), None);
        assert_eq!(error_from_strmessage("Something went wrong"), None);
        assert_eq!(error_from_strmessage("Something went wrong ()"), None);
        assert_eq!(error_from_strmessage("Something went wrong (abc)"), None);
        assert_eq!(error_from_strmessage("Something went wrong (99999)"), None);
        assert_eq!(error_from_strmessage("Something went wrong (-1)"), None);
        assert_eq!(error_from_strmessage("(((("), None);
    }

    #[test]
    fn error_strmessage_last_code_wins() {
        assert_eq!(
            error_from_strmessage("Item (StatTrak) is gone (26)"),
            Some(OfferError::Revoked)
        );
        assert_eq!(error_from_strmessage("(26) then (nothing)"), Some(OfferError::Revoked));
    }
}

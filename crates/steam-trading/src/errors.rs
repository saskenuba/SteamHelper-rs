#[cfg(feature = "cs2")]
use std::collections::HashSet;

use steam_language::enums::EResult;
use steam_mobile::errors::AuthError;
use steam_mobile::HttpError;
use steam_mobile::StatusCode;
use tappet::errors::SteamAPIError;
use thiserror::Error;

#[cfg(feature = "cs2")]
use crate::AssetId;
use crate::OfferId;
#[cfg(feature = "cs2")]
use crate::OfferState;
#[cfg(feature = "cs2")]
use crate::TradeId;
use crate::TRADE_MAX_ITEMS;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TradeError {
    /// A local failure, not Steam's.
    #[error("`{0}`")]
    GeneralError(String),

    /// Steam refused, with its code: the `(N)` at the end of a `strError`, a Web API call's `x-eresult` header, or a
    /// JSON `success` in an answer that can't be read as the one expected (create, accept, decline and cancel answers
    /// have only optional fields, so a body with only `success` reads as one of them, not as this error). Read it with
    /// [`TradeError::eresult`]; this crate gives codes no meaning. `message` is Steam's `strError`, when it sent one.
    #[error("Steam answered {eresult:?}{}", .message.as_ref().map(|message| format!(": {}", message)).unwrap_or_default())]
    Steam { eresult: EResult, message: Option<String> },

    /// Steam's answer could not be read, or it refused without a known code (a `strError` without one): the answer,
    /// or what was wrong with it.
    #[error("Unexpected answer from Steam: {0}")]
    UnexpectedResponse(String),

    /// A Web API call failed without a code from Steam, see [`WebApiError`].
    #[error(transparent)]
    WebApiError(#[from] WebApiError),

    #[error(transparent)]
    AuthError(#[from] AuthError),

    #[error(transparent)]
    InternalError(#[from] InternalError),

    #[error(transparent)]
    TradeLinkError(#[from] TradeLinkError),

    /// The offer has no item on either side: nothing was sent.
    #[error("An offer needs at least one item.")]
    EmptyOffer,

    /// A side of the offer has more than [`TRADE_MAX_ITEMS`] items: nothing was sent.
    #[error("An offer can't have more than {} items on a side.", TRADE_MAX_ITEMS)]
    TooManyItems,

    /// An item of the offer has an amount of 0: nothing was sent.
    #[error("An offer can't have an item with an amount of 0.")]
    InvalidAmount,

    /// The session has no `sessionid` cookie, which Steam Community requests need: log in first.
    #[error("The session has no sessionid cookie. You need to log in first.")]
    NoSessionId,

    /// `GetTradeHoldDurations` answered without `both_escrow` and without an error code. Not seen live (a wrong token
    /// is [`TradeError::Steam`] with `AccessDenied`); ArchiSteamFarm treats it as a failure too
    /// (`ArchiSteamFarm/Steam/Integration/ArchiWebHandler.cs:1829-1835`).
    #[error("Steam didn't say how long a trade with this partner is held.")]
    TradeHoldUnknown,

    /// The offer's mobile confirmation wasn't among the account's confirmations: it may not be there yet. Retry the
    /// confirmation ([`SteamTradeManager::confirm_offers`](crate::SteamTradeManager::confirm_offers)); an offer this
    /// account sent can also be cancelled.
    #[error("No mobile confirmation for trade offer `{offer_id}` was found.")]
    ConfirmationNotFound { offer_id: OfferId },

    /// Not every `GetTradeOffers` page could be read, so no offers are returned rather than an incomplete list.
    /// `cursor` is the page that was not read.
    #[error("GetTradeOffers stopped at cursor `{cursor}`, offers would be missing: {reason}.")]
    TradeOffersIncomplete { cursor: u32, reason: String },

    /// Steam sent trade offers that could not be read (see tappet's `MalformedOffer`), so no offers are returned
    /// rather than a list without them. `offer_ids` holds each one's id, when it could be read; `reason` is why the
    /// first one could not.
    ///
    /// Like node-steam-tradeoffer-manager, every call fails while Steam keeps sending such an offer, so one offer can
    /// stop polling for good. To get past it, decline it with
    /// [`SteamTradeManager::decline_offer`](crate::SteamTradeManager::decline_offer) if it was sent to this account,
    /// or cancel it with [`SteamTradeManager::cancel_offer`](crate::SteamTradeManager::cancel_offer) if this account
    /// sent it, using its id from `offer_ids`: those two never look the offer up. The ids don't say which side sent
    /// the offer. There is no such way for an offer whose id is `None`: it stays until it is no longer active, e.g.
    /// declined or cancelled on Steam's website.
    #[error("{} trade offer(s) could not be read, the first because: {reason}", .offer_ids.len())]
    MalformedOffers {
        offer_ids: Vec<Option<OfferId>>,
        reason: String,
    },

    /// Steam sent the trade offer, but it could not be read (see tappet's `MalformedOffer`).
    #[error("Trade offer `{offer_id}` could not be read: {reason}")]
    MalformedOffer { offer_id: OfferId, reason: String },

    /// Steam waits for a confirmation by email, which this crate can't give. Confirm it from the email, or cancel the
    /// offer.
    #[error("Trade offer `{offer_id}` waits for an email confirmation.")]
    NeedsEmailConfirmation { offer_id: OfferId },

    /// The trade offer was created, but a later step failed. Cancel the offer with `offer_id`, or retry the
    /// confirmation.
    #[error("Trade offer `{offer_id}` was created, but confirming it failed: {source}")]
    OfferCreatedButNotConfirmed {
        offer_id: OfferId,
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
    #[error("CS2's game coordinator has no current inventory of the account; nothing was accepted or looked up.")]
    Cs2InventoryUnavailable,

    /// The offer given to `find_received_items` isn't accepted, so no item of it was received: nothing was looked
    /// up. `state` is its state.
    #[cfg(feature = "cs2")]
    #[error("Trade offer `{offer_id}` is not accepted (state {state:?}); nothing was looked up.")]
    OfferNotAccepted { offer_id: OfferId, state: OfferState },

    /// The accept was sent, but its confirmation failed: the trade may still happen. Check the offer's state, then
    /// call [`SteamTradeManager::find_received_items`](crate::SteamTradeManager::find_received_items) with
    /// `known_ids`, the game coordinator's item ids right before the accept, to find its items.
    #[cfg(feature = "cs2")]
    #[error("Trade offer `{offer_id}` was accepted, but confirming the accept failed: {source}")]
    AcceptedButNotConfirmed {
        offer_id: OfferId,
        known_ids: HashSet<AssetId>,
        #[source]
        source: Box<TradeError>,
    },

    /// The offer was accepted, but finding where its items went failed. The trade happened: `trade_id` is its id, as
    /// [`SteamTradeManager::accept`](crate::SteamTradeManager::accept) returns it (`None` after a mobile
    /// confirmation). `known_ids` are the game coordinator's item ids right before the accept: call
    /// [`SteamTradeManager::find_received_items`](crate::SteamTradeManager::find_received_items) with them later to
    /// find its items, adding the new ids other trades were found to bring since.
    #[cfg(feature = "cs2")]
    #[error("Trade offer `{offer_id}` was accepted (trade {trade_id:?}), but finding its items failed: {source}")]
    AcceptedButItemsNotFound {
        offer_id: OfferId,
        trade_id: Option<TradeId>,
        known_ids: HashSet<AssetId>,
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
            TradeError::WebApiError(error) => error.is_too_many_requests(),
            TradeError::OfferCreatedButNotConfirmed { source, .. } => source.is_too_many_requests(),
            #[cfg(feature = "cs2")]
            TradeError::AcceptedButItemsNotFound { source, .. } => source.is_too_many_requests(),
            #[cfg(feature = "cs2")]
            TradeError::AcceptedButNotConfirmed { source, .. } => source.is_too_many_requests(),
            _ => false,
        }
    }

    /// Steam's code, of [`TradeError::Steam`] (a Web API call's `x-eresult` included), also when it is why a later step
    /// failed (inside [`TradeError::OfferCreatedButNotConfirmed`] and, with feature `cs2`, `AcceptedButNotConfirmed`
    /// and `AcceptedButItemsNotFound`).
    pub fn eresult(&self) -> Option<EResult> {
        match self {
            TradeError::Steam { eresult, .. } => Some(*eresult),
            TradeError::OfferCreatedButNotConfirmed { source, .. } => source.eresult(),
            #[cfg(feature = "cs2")]
            TradeError::AcceptedButItemsNotFound { source, .. } => source.eresult(),
            #[cfg(feature = "cs2")]
            TradeError::AcceptedButNotConfirmed { source, .. } => source.eresult(),
            _ => None,
        }
    }
}

/// A Web API call (`GetTradeOffers`, `GetTradeOffer`, `GetTradeHistory`, `GetTradeHoldDurations`) failed without a
/// code from Steam: the request itself, an HTTP status that isn't 200, an `X-error_message` header, an `x-eresult`
/// this version doesn't know, or an answer that can't be read. It shows why; its
/// [`source`](std::error::Error::source) is the error of the Web API client (tappet). A known `x-eresult` is
/// [`TradeError::Steam`] instead.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct WebApiError(#[source] SteamAPIError);

impl WebApiError {
    /// Whether Steam answered HTTP 429 Too Many Requests.
    fn is_too_many_requests(&self) -> bool {
        matches!(&self.0, SteamAPIError::SteamHttpError(status) if status == StatusCode::TOO_MANY_REQUESTS.as_str())
    }
}

/// The error of a failed Web API call: [`TradeError::Steam`] for a known `x-eresult` (tappet's text with it is only
/// `Value: N`, so no message), else [`TradeError::WebApiError`].
pub(crate) fn web_api_error(error: SteamAPIError) -> TradeError {
    match error {
        SteamAPIError::EResult(eresult, _) => TradeError::Steam { eresult, message: None },
        error => TradeError::WebApiError(WebApiError(error)),
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
/// steam-mobile and tappet do. The inventory's URL has one (`?l=english&count=…&start_assetid=…`, no secret).
impl From<HttpError> for InternalError {
    fn from(mut error: HttpError) -> Self {
        if let Some(url) = error.url_mut() {
            url.set_query(None);
        }
        InternalError::NetworkError(error)
    }
}

#[derive(Error, Debug, PartialEq, Copy, Clone)]
pub enum TradeLinkError {
    #[error("The Tradeoffer URL was not valid.")]
    Invalid,
}

/// The EResult code in parentheses that Steam appends to a `strError` message, e.g. `There was an error accepting
/// this trade offer. Please try again later. (28)`.
///
/// Localized messages may hold other parentheses, so the last `(N)` with a known EResult wins. `None` if there is no
/// such code.
pub(crate) fn eresult_of_strmessage(message: &str) -> Option<EResult> {
    message.match_indices('(').rev().find_map(|(index_start, _)| {
        let after_start = &message[index_start + 1..];
        let number = &after_start[..after_start.find(')')?];

        serde_json::from_str::<EResult>(number.trim()).ok()
    })
}

/// The error for a Steam `strError` message: [`TradeError::Steam`] with its code, else
/// [`TradeError::UnexpectedResponse`].
pub(crate) fn error_of_strmessage(message: String) -> TradeError {
    match eresult_of_strmessage(&message) {
        Some(eresult) => TradeError::Steam {
            eresult,
            message: Some(message),
        },
        None => TradeError::UnexpectedResponse(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_strmessage() {
        let error_message = "Something went wrong (26)";
        assert_eq!(eresult_of_strmessage(error_message), Some(EResult::Revoked))
    }

    #[test]
    fn error_strmessage_non_ascii() {
        assert_eq!(
            eresult_of_strmessage("Ocorreu um erro ao enviar a oferta de troca. Tente novamente mais tarde. (26)"),
            Some(EResult::Revoked)
        );
        assert_eq!(
            eresult_of_strmessage("交易报价出错（请稍后再试）。 (42)"),
            Some(EResult::NoMatch)
        );
        assert_eq!(eresult_of_strmessage("ошибка (11) ä"), Some(EResult::InvalidState));
    }

    #[test]
    fn error_strmessage_closing_before_opening() {
        assert_eq!(eresult_of_strmessage(") wrong order ("), None);
        assert_eq!(eresult_of_strmessage("a) b (26"), None);
        assert_eq!(eresult_of_strmessage("a) b (26)"), Some(EResult::Revoked));
    }

    #[test]
    fn error_strmessage_no_known_code() {
        assert_eq!(eresult_of_strmessage(""), None);
        assert_eq!(eresult_of_strmessage("Something went wrong"), None);
        assert_eq!(eresult_of_strmessage("Something went wrong ()"), None);
        assert_eq!(eresult_of_strmessage("Something went wrong (abc)"), None);
        assert_eq!(eresult_of_strmessage("Something went wrong (99999)"), None);
        assert_eq!(eresult_of_strmessage("Something went wrong (-1)"), None);
        assert_eq!(eresult_of_strmessage("(((("), None);
    }

    #[test]
    fn error_strmessage_last_code_wins() {
        assert_eq!(
            eresult_of_strmessage("Item (StatTrak) is gone (26)"),
            Some(EResult::Revoked)
        );
        assert_eq!(eresult_of_strmessage("(26) then (nothing)"), Some(EResult::Revoked));
    }

    #[test]
    fn steam_error_keeps_the_code_and_the_message() {
        let message = "There was an error sending your trade offer.  Please try again later. (26)";
        match error_of_strmessage(message.to_string()) {
            error @ TradeError::Steam { .. } => {
                assert_eq!(error.eresult(), Some(EResult::Revoked));
                assert_eq!(error.to_string(), format!("Steam answered Revoked: {}", message));
                // and through the error of a later step
                let wrapped = TradeError::OfferCreatedButNotConfirmed {
                    offer_id: OfferId(7),
                    source: Box::new(error),
                };
                assert_eq!(wrapped.eresult(), Some(EResult::Revoked));
            }
            other => panic!("{:?}", other),
        }

        match error_of_strmessage("Você não pode negociar (aguarde).".to_string()) {
            error @ TradeError::UnexpectedResponse(_) => assert_eq!(error.eresult(), None),
            other => panic!("{:?}", other),
        }
        let no_message = TradeError::Steam {
            eresult: EResult::Fail,
            message: None,
        };
        assert_eq!(no_message.to_string(), "Steam answered Fail");
    }

    /// `error`, also inside each error that wraps the error of a later step.
    fn wrapped(error: impl Fn() -> TradeError) -> Vec<TradeError> {
        #[allow(unused_mut)]
        let mut errors = vec![
            error(),
            TradeError::OfferCreatedButNotConfirmed {
                offer_id: OfferId(7),
                source: Box::new(error()),
            },
        ];
        #[cfg(feature = "cs2")]
        errors.extend([
            TradeError::AcceptedButNotConfirmed {
                offer_id: OfferId(7),
                known_ids: HashSet::new(),
                source: Box::new(error()),
            },
            TradeError::AcceptedButItemsNotFound {
                offer_id: OfferId(7),
                trade_id: None,
                known_ids: HashSet::new(),
                source: Box::new(error()),
            },
        ]);
        errors
    }

    #[test]
    fn web_api_eresult_is_steams_code() {
        let error = || web_api_error(SteamAPIError::EResult(EResult::AccessDenied, "Value: 15".to_string()));
        assert!(matches!(
            error(),
            TradeError::Steam {
                eresult: EResult::AccessDenied,
                message: None
            }
        ));
        for error in wrapped(error) {
            assert_eq!(error.eresult(), Some(EResult::AccessDenied), "{:?}", error);
            assert!(!error.is_too_many_requests());
        }
    }

    #[test]
    fn web_api_too_many_requests() {
        let error = || web_api_error(SteamAPIError::SteamHttpError("429".to_string()));
        assert!(matches!(error(), TradeError::WebApiError(_)));
        for error in wrapped(error) {
            assert!(error.is_too_many_requests(), "{:?}", error);
            assert_eq!(error.eresult(), None);
        }
        assert!(!web_api_error(SteamAPIError::SteamHttpError("500".to_string())).is_too_many_requests());
    }

    #[test]
    fn other_web_api_errors_are_ours() {
        let decode_error = serde_json::from_str::<u8>("<html>").unwrap_err();
        for tappet_error in [
            SteamAPIError::SteamHttpError("500".to_string()),
            SteamAPIError::ErrorMessage("bad tradeofferid".to_string()),
            SteamAPIError::UnknownEResult("99999".to_string()),
            SteamAPIError::DecodeError {
                source: decode_error,
                body: "<html>".to_string(),
            },
        ] {
            let shown = tappet_error.to_string();
            match web_api_error(tappet_error) {
                TradeError::WebApiError(error) => {
                    assert_eq!(error.to_string(), shown);
                    let source = std::error::Error::source(&error).expect("the Web API client's error");
                    assert_eq!(source.to_string(), shown);
                    let error = TradeError::WebApiError(error);
                    assert_eq!(error.eresult(), None);
                    assert!(!error.is_too_many_requests());
                }
                other => panic!("{:?}", other),
            }
        }
    }
}

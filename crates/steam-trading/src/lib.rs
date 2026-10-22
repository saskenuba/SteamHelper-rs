//! Trade offers of a Steam account: create, confirm, accept, decline and cancel them, and read offers, trades and
//! inventories, through the account's `SteamAuthenticator` (steam-mobile): its cookies, its access token and its
//! mobile confirmations.
//!
//! **IT IS VERY IMPORTANT THAT STEAM GUARD IS ENABLED ON THE ACCOUNT BEING USED, WITH MOBILE CONFIRMATIONS.**
//!
//! A [`SteamTradeManager`] owns the account's `SteamAuthenticator`: make one per account, and share it. The manager
//! reacts to no events, in comparison of alternatives such as Node.js: you keep track of offers and poll yourself.
//!
//! Its methods come in two layers. Steps are one Steam step each: one request, or a confirmation round (getlist, then
//! one multiajaxop), or every page of a paged list (offers, an inventory):
//! [`create_offer`](SteamTradeManager::create_offer), [`confirm_offers`](SteamTradeManager::confirm_offers),
//! [`post_accept`](SteamTradeManager::post_accept), [`decline_offer`](SteamTradeManager::decline_offer),
//! [`cancel_offer`](SteamTradeManager::cancel_offer), [`offers`](SteamTradeManager::offers),
//! [`offer`](SteamTradeManager::offer), [`trades`](SteamTradeManager::trades), [`trade`](SteamTradeManager::trade),
//! [`trade_hold`](SteamTradeManager::trade_hold), [`inventory`](SteamTradeManager::inventory) and
//! [`my_trade_link`](SteamTradeManager::my_trade_link). Convenience methods only chain steps:
//! [`send`](SteamTradeManager::send) (create, then confirm), [`accept`](SteamTradeManager::accept) (accept, then
//! confirm when Steam asks), and with feature `cs2` `accept_and_find_items` and `find_received_items`.
//!
//! ```no_run
//! use steam_trading::{
//!     Asset, AssetId, InventoryContext, OfferQuery, OfferRequest, SteamTradeManager, TradeError, TradeLink,
//! };
//!
//! async fn trade(manager: &SteamTradeManager, link: TradeLink) -> Result<(), TradeError> {
//!     let request = OfferRequest::to(link).give(Asset::new(InventoryContext::CS2, AssetId(1)));
//!     let sent = manager.send(&request).await?;
//!     println!("sent offer {}", sent);
//!
//!     for offer in manager.offers(OfferQuery::ACTIVE_RECEIVED).await? {
//!         manager.decline_offer(offer.id).await?;
//!     }
//!     Ok(())
//! }
//! ```
//!
//! An authenticator logged in with a password needs nothing running beside it. One logged in through a Steam client
//! (steam-mobile's `login_with_client`, which feature `cs2` turns on) keeps that client running, and needs it
//! connected to renew its access token, about daily, an hour before it expires (steam-mobile's `renew_before` sets
//! another margin). The first request once the renewal is due waits for it, up to the client's call deadline of 30
//! seconds; requests coming meanwhile go at once with the current token. While the client can't renew it, requests
//! go on with the current token until it expires. After, none is sent: requests wait for another attempt (up to the
//! same 30 seconds), and fail if it fails, or fail at once while the last one failed less than 30 seconds before.
//!
//! A Steam Community request (a page, an inventory, a form) that Steam redirects to its login page (a final URL on
//! `steamcommunity.com` whose path starts with `/login`, seen live as `login/home/?goto=…`) fails with steam-mobile's
//! `InternalError::SessionExpired` (in [`TradeError::InternalError`]): the session is gone, log in again. Checking the
//! final URL is this crate's rule. node-steamcommunity and aiosteampy check the `Location` of a redirect they didn't
//! follow (`components/http.js:106-111`, `transport/base.py:239`).
//!
//! Compiles on stable Rust.

// #![warn(missing_docs, missing_doc_code_examples)]
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_import_braces,
    unused_qualifications
)]

#[cfg(feature = "cs2")]
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use const_format::concatcp;
pub use errors::InternalError;
pub use errors::TradeError;
pub use errors::TradeLinkError;
pub use errors::WebApiError;
use futures::lock::Mutex;
use futures::TryFutureExt;
use futures_timer::Delay;
pub use ids::Asset;
pub use ids::AssetId;
pub use ids::OfferId;
pub use ids::TradeId;
pub use inventory::AssetProperty;
pub use inventory::Inventory;
pub use inventory::InventoryItem;
pub use inventory::InventoryPage;
pub use inventory::PropertyValue;
pub use offer::Accepted;
pub use offer::Confirmed;
pub use offer::CreatedOffer;
pub use offer::Needs;
pub use offer::Offer;
pub use offer::OfferQuery;
pub use offer::OfferRequest;
pub use offer::OfferState;
pub use offer::OfferedItem;
pub use offer::PostedAccept;
#[cfg(feature = "cs2")]
pub use received::AcceptedOffer;
#[cfg(feature = "cs2")]
pub use received::Fingerprint;
#[cfg(feature = "cs2")]
pub use received::Located;
#[cfg(feature = "cs2")]
pub use received::ReceivedItem;
use reqwest::Response;
use serde::de::DeserializeOwned;
#[cfg(feature = "cs2")]
use steam_client::gc::cs2::Cs2;
/// Steam's result codes, as [`TradeError::eresult`] returns them.
pub use steam_language::enums::EResult;
use steam_mobile::user::PresentMaFile;
use steam_mobile::Authenticated;
use steam_mobile::Confirmation;
use steam_mobile::ConfirmationAction;
use steam_mobile::HeaderMap;
use steam_mobile::Method;
use steam_mobile::StatusCode;
use steam_mobile::SteamAuthenticator;
use steam_mobile::Url;
use steam_mobile::STEAM_COMMUNITY_HOST;
pub use steamid_parser::SteamID;
use tappet::endpoints::ieconservice::get::GetTradeHoldDurations;
use tappet::endpoints::ieconservice::get::GetTradeOffer;
use tappet::endpoints::ieconservice::get::GetTradeOffers;
use tappet::errors::SteamAPIError;
use tappet::response_types::GetTradeHistoryResponse;
use tappet::response_types::GetTradeHoldDurationsResponseBase;
use tappet::response_types::GetTradeOfferResponse;
use tappet::response_types::GetTradeOffersResponse;
use tappet::response_types::MalformedOffer;
use tappet::response_types::TradeOffer_Trade;
use tappet::Credential;
use tappet::ExecutorResponse;
use tappet::SteamAPI;
use tracing::warn;
pub use trade::Trade;
pub use trade::TradeQuery;
pub use trade::TradeStatus;
pub use trade::TradedItem;
pub use types::context::InventoryContext;
pub use types::trade_link::TradeLink;

use crate::errors::web_api_error;
use crate::inventory::inventory_url;
use crate::inventory::read_all_inventory_pages;
use crate::offer::number;
use crate::response::check_response;
use crate::response::login_redirect_check;
use crate::response::status_check;
use crate::types::sessionid::HasSessionID;
use crate::types::trade_offer_web::TradeOfferAcceptRequest;
use crate::types::trade_offer_web::TradeOfferCancelResponse;
use crate::types::trade_offer_web::TradeOfferCommonParameters;
use crate::types::trade_offer_web::TradeOfferCreateRequest;
use crate::types::trade_offer_web::TradeOfferCreateResponse;
use crate::types::trade_offer_web::TradeOfferGenericErrorResponse;
use crate::types::trade_offer_web::TradeOfferGenericRequest;
use crate::types::trade_offer_web::TradeProtectionAcknowledgeRequest;
use crate::types::TradeKind;

mod errors;
mod ids;
mod inventory;
mod offer;
#[cfg(feature = "cs2")]
mod received;
mod response;
mod trade;
mod types;

const TRADEOFFER_BASE: &str = "https://steamcommunity.com/tradeoffer/";
const TRADEOFFER_NEW_URL: &str = concatcp!(TRADEOFFER_BASE, "new/send");
/// The `Referer` of a create.
const TRADEOFFER_NEW_REFERER: &str = concatcp!(TRADEOFFER_BASE, "new");
const TRADE_PROTECTION_ACKNOWLEDGE_URL: &str = "https://steamcommunity.com/trade/new/acknowledge";

/// At most this many items on a side of an offer, as ArchiSteamFarm (`ArchiSteamFarm/Steam/Exchange/Trading.cs:44`,
/// `MaxItemsPerTrade = byte.MaxValue`): decided upon various factors, mainly stability of Steam servers when dealing
/// with huge trade offers.
pub const TRADE_MAX_ITEMS: u8 = u8::MAX;

/// How long [`SteamTradeManager::send`] waits between the create and the confirmation.
const STANDARD_DELAY: Duration = Duration::from_secs(1);

const MAX_HISTORICAL_CUTOFF: u32 = u32::MAX;

/// At most this many `GetTradeOffers` pages are read in one call, a guard against a cursor that never ends.
const MAX_TRADE_OFFER_PAGES: usize = 100;

pub(crate) type SteamCompleteAuthenticator = SteamAuthenticator<Authenticated, PresentMaFile>;

/// Trades of the account its authenticator is logged in to.
///
/// One manager per account: it owns the account's authenticator, and its methods take `&self`, so share it (with an
/// `Arc`, say) instead of making another. Its trade protection acknowledgement and its accept lock last as long as it
/// does, so as long as the account's session.
#[derive(Debug)]
pub struct SteamTradeManager {
    authenticator: SteamCompleteAuthenticator,
    /// The HTTP client of every Web API call, so that they share one connection pool.
    web_api_client: reqwest::Client,
    /// Whether Steam took this manager's trade protection acknowledgement: sent before the first confirmation.
    acknowledged: AtomicBool,
    /// Held while accepting an offer: one accept at a time, so that the items one brings are told from another's.
    accepting: Mutex<()>,
    /// How long `accept_and_find_items` waits for the received items.
    #[cfg(feature = "cs2")]
    received_items_timeout: Duration,
}

/// The client for one Web API call, with the session's access token.
fn web_api_with_token(http_client: &reqwest::Client, access_token: String) -> SteamAPI {
    SteamAPI::with_client(http_client.clone(), Credential::AccessToken(access_token))
}

/// A Steam Community request, before the session's `sessionid` is added.
struct PreparedRequest {
    url: String,
    headers: Option<HeaderMap>,
    form: Box<dyn HasSessionID>,
}

impl PreparedRequest {
    /// The create of `offer`.
    fn create(offer: &OfferRequest) -> Self {
        Self {
            url: TradeKind::Create.endpoint(None),
            headers: Some(referer(TRADEOFFER_NEW_REFERER.to_string())),
            form: Box::new(TradeOfferCreateRequest::new(offer)),
        }
    }

    /// The accept of the offer `offer_id`, with `partner` in its form.
    fn accept(offer_id: OfferId, partner: u64) -> Self {
        Self {
            url: TradeKind::Accept.endpoint(Some(offer_id)),
            headers: Some(referer(format!("{}{}/", TRADEOFFER_BASE, offer_id))),
            form: Box::new(TradeOfferAcceptRequest {
                common: TradeOfferCommonParameters {
                    their_steamid: partner,
                    ..Default::default()
                },
                tradeofferid: offer_id.0,
                ..Default::default()
            }),
        }
    }

    /// The decline of the offer `offer_id`.
    fn decline(offer_id: OfferId) -> Self {
        Self::without_referer(TradeKind::Decline.endpoint(Some(offer_id)))
    }

    /// The cancel of the offer `offer_id`.
    fn cancel(offer_id: OfferId) -> Self {
        Self::without_referer(TradeKind::Cancel.endpoint(Some(offer_id)))
    }

    fn without_referer(url: String) -> Self {
        Self {
            url,
            headers: None,
            form: Box::<TradeOfferGenericRequest>::default(),
        }
    }

    fn acknowledge() -> Self {
        Self {
            url: TRADE_PROTECTION_ACKNOWLEDGE_URL.to_string(),
            headers: None,
            form: Box::<TradeProtectionAcknowledgeRequest>::default(),
        }
    }
}

/// Headers with the `Referer` `url`.
fn referer(url: String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Referer", url.parse().expect("a URL is a valid header value"));
    headers
}

/// How [`SteamTradeManager::confirm_offers`] failed: [`SteamTradeManager::send`] tells the two apart.
#[derive(Debug)]
enum ConfirmFailed {
    /// The account's confirmations could not be fetched: nothing was confirmed.
    Fetch(TradeError),
    /// Steam refused to accept the confirmations found, or the request failed.
    Process(TradeError),
}

impl From<ConfirmFailed> for TradeError {
    fn from(failed: ConfirmFailed) -> Self {
        match failed {
            ConfirmFailed::Fetch(error) | ConfirmFailed::Process(error) => error,
        }
    }
}

impl SteamTradeManager {
    /// Returns a new `[SteamTradeManager]` for the account `authenticator` is logged in to, which it owns: get it back
    /// with [`SteamTradeManager::authenticator`], [`SteamTradeManager::authenticator_mut`] or
    /// [`SteamTradeManager::into_authenticator`].
    ///
    /// Its Web API calls (`GetTradeOffers`, `GetTradeOffer`, `GetTradeHistory`, `GetTradeHoldDurations`) send the
    /// session's access token (`access_token=`), read from `authenticator` for every call, never kept: an
    /// authenticator logged in through a Steam client (steam-mobile's `login_with_client`) renews it about daily, when
    /// a call comes within its renewal margin (an hour by default, steam-mobile's `renew_before`).
    /// No API key is needed or used: with one, Steam leaves CS2 offers and trades out (seen live 2026-10-20). Steam
    /// Community requests (create, accept, decline, cancel, confirmations) use the session cookies.
    pub fn new(authenticator: SteamAuthenticator<Authenticated, PresentMaFile>) -> SteamTradeManager {
        Self {
            authenticator,
            web_api_client: reqwest::Client::new(),
            acknowledged: AtomicBool::new(false),
            accepting: Mutex::new(()),
            #[cfg(feature = "cs2")]
            received_items_timeout: received::DEFAULT_TIMEOUT,
        }
    }

    /// The account's authenticator, for its own requests, such as confirmations the manager doesn't handle.
    pub fn authenticator(&self) -> &SteamAuthenticator<Authenticated, PresentMaFile> {
        &self.authenticator
    }

    /// The account's authenticator, for what takes it mutably: `register_api_key` and `revoke_api_key`.
    ///
    /// For the account's own authenticator: swapping in another account's carries the manager's per-account state
    /// over, such as whether the account acknowledged trade protection.
    pub fn authenticator_mut(&mut self) -> &mut SteamAuthenticator<Authenticated, PresentMaFile> {
        &mut self.authenticator
    }

    /// The account's authenticator, giving the manager up.
    pub fn into_authenticator(self) -> SteamAuthenticator<Authenticated, PresentMaFile> {
        self.authenticator
    }

    /// The client for one Web API call, with the session's access token, renewed first if it is about to expire.
    async fn web_api(&self) -> Result<SteamAPI, TradeError> {
        let access_token = self.authenticator.access_token().await.map_err(InternalError::from)?;
        Ok(web_api_with_token(
            &self.web_api_client,
            access_token.expose_secret().to_string(),
        ))
    }

    /// Creates the offer `offer`: `POST https://steamcommunity.com/tradeoffer/new/send`. Its id, and what it waits
    /// for: a mobile confirmation ([`SteamTradeManager::confirm_offers`]) when this account gives items.
    ///
    /// # Errors
    ///
    /// Before anything is sent: [`TradeError::EmptyOffer`], [`TradeError::TooManyItems`],
    /// [`TradeError::InvalidAmount`] (see [`OfferRequest`]). [`TradeError::Steam`] when Steam refused with a code,
    /// [`TradeError::UnexpectedResponse`] for an answer without the offer's id.
    pub async fn create_offer(&self, offer: &OfferRequest) -> Result<CreatedOffer, TradeError> {
        offer.check()?;
        let answer: TradeOfferCreateResponse = self.send_request(PreparedRequest::create(offer)).await?;
        created_offer(&answer)
    }

    /// Accepts the mobile confirmations of the offers `offer_ids`, those this account created or accepted.
    ///
    /// Fetches the account's confirmations (`GET https://steamcommunity.com/mobileconf/getlist`) and, if any is of one
    /// of these offers, acknowledges trade protection (once per manager, see
    /// [`SteamTradeManager::acknowledge_trade_protection`]) and accepts them all in one request (`POST
    /// https://steamcommunity.com/mobileconf/multiajaxop`), as ArchiSteamFarm
    /// (`ArchiSteamFarm/Steam/Integration/ArchiWebHandler.cs:2073-2093`) and node-steamcommunity
    /// (`components/confirmations.js:223-243`) do. Sends nothing for no offer.
    ///
    /// # Errors
    ///
    /// The error of either request: Steam refusing the batch fails it for every offer.
    pub async fn confirm_offers(&self, offer_ids: &[OfferId]) -> Result<Confirmed, TradeError> {
        self.confirm(offer_ids).err_into().await
    }

    /// [`SteamTradeManager::confirm_offers`], telling a failed fetch from a failed accept: [`confirm_steps`] with
    /// Steam's requests.
    async fn confirm(&self, offer_ids: &[OfferId]) -> Result<Confirmed, ConfirmFailed> {
        confirm_steps(
            offer_ids,
            || {
                self.authenticator
                    .fetch_confirmations()
                    .map_ok(|confirmations| confirmations.0)
                    .err_into()
            },
            |confirmation: &Confirmation, offer_id| confirmation.has_trade_offer_id(offer_id.0),
            || acknowledge_once(&self.acknowledged, || self.acknowledge_trade_protection()),
            |found| {
                self.authenticator
                    .process_confirmations(ConfirmationAction::Accept, found)
                    .err_into()
            },
        )
        .await
    }

    /// Accepts `offer`, an offer made to this account: `POST https://steamcommunity.com/tradeoffer/{id}/accept`.
    /// Steam's answer: the trade's id when it needs nothing more, else what it waits for.
    /// [`SteamTradeManager::accept`] confirms too.
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    ///
    /// The form's `partner` is built from the offer's id, not from its partner, as this crate always sent it: wrong by
    /// reading, but Steam takes it. Changing it needs a live check.
    ///
    /// One accept at a time per manager: a call waits for the accept in progress (also of
    /// [`SteamTradeManager::accept`] and, with feature `cs2`, `accept_and_find_items`).
    pub async fn post_accept(&self, offer: &Offer) -> Result<PostedAccept, TradeError> {
        let _accepting = self.accepting.lock().await;
        self.post_accept_unlocked(offer).await
    }

    /// [`SteamTradeManager::post_accept`]. The caller holds the accept lock.
    async fn post_accept_unlocked(&self, offer: &Offer) -> Result<PostedAccept, TradeError> {
        let request = PreparedRequest::accept(offer.id, accept_partner(offer.id));
        let answer: TradeOfferCreateResponse = self.send_request(request).await?;
        Ok(posted_accept(&answer))
    }

    /// Declines `offer_id`, an offer made to this account: `POST https://steamcommunity.com/tradeoffer/{id}/decline`.
    ///
    /// # Errors
    ///
    /// [`TradeError::UnexpectedResponse`] if Steam's answer isn't about this offer, as node-steam-tradeoffer-manager
    /// (`lib/classes/TradeOffer.js` ~531) and aiosteampy (`client/components/trade/trade.py:520-585`) check.
    pub async fn decline_offer(&self, offer_id: OfferId) -> Result<(), TradeError> {
        let answer: TradeOfferCancelResponse = self.send_request(PreparedRequest::decline(offer_id)).await?;
        answer_about(offer_id, &answer)
    }

    /// Cancels `offer_id`, an offer this account sent: `POST https://steamcommunity.com/tradeoffer/{id}/cancel`.
    ///
    /// # Errors
    ///
    /// As [`SteamTradeManager::decline_offer`].
    pub async fn cancel_offer(&self, offer_id: OfferId) -> Result<(), TradeError> {
        let answer: TradeOfferCancelResponse = self.send_request(PreparedRequest::cancel(offer_id)).await?;
        answer_about(offer_id, &answer)
    }

    /// Acknowledges Steam's trade protection notice for this account: `POST /trade/new/acknowledge`, as the
    /// browser does when the notice is dismissed.
    ///
    /// The manager calls it before its first confirmation ([`SteamTradeManager::confirm_offers`]); it is public for
    /// bots that confirm through steam-mobile directly. Once per account is enough. Steam requires it to confirm trades
    /// of trade-protected items: live (2026-10-20, CS2), confirming such an offer answered `{"success":false}` until
    /// the account acknowledged, then `{"success":true}`. Steam answered `[]`; any 2xx answer without a `strError` is
    /// taken as success. ArchiSteamFarm sends it before trading items of other games than Steam's own (appid 753);
    /// node-steamcommunity has it as `acknowledgeTradeProtection()`.
    pub async fn acknowledge_trade_protection(&self) -> Result<(), TradeError> {
        let (status, url, text) = self.post_community_form(PreparedRequest::acknowledge()).await?;
        check_response(status, &url, &text)
    }

    /// The offers of this account that `query` asks for, from `GetTradeOffers`: sent ones first, then received ones.
    ///
    /// Steam pages this endpoint: every page is read, and the offers of all pages are merged. The call fails rather
    /// than return an incomplete list, like node-steam-tradeoffer-manager: with a page's error,
    /// [`TradeError::TradeOffersIncomplete`] when Steam repeats a cursor or sends more than 100 pages, and
    /// [`TradeError::MalformedOffers`] when Steam sent an offer that could not be read (`lib/index.js:606-609` fails
    /// on an offer without `accountid_other`). An offer whose items couldn't all be read is listed, with
    /// [`Offer::has_malformed_items`].
    ///
    /// Every call fails while Steam keeps sending an offer that can't be read: see [`TradeError::MalformedOffers`]
    /// for how to get past it.
    pub async fn offers(&self, query: OfferQuery) -> Result<Vec<Offer>, TradeError> {
        let pages = fetch_all_trade_offer_pages(|cursor| async move {
            let api_client = self.web_api().await?;
            trade_offers_request(&api_client, query, cursor)
                .execute_with_response()
                .map_err(web_api_error)
                .await
        })
        .await?;
        fail_on_malformed_offers(&pages)?;
        offers_of(pages)
    }

    /// The offer `offer_id`, in any state, from `GetTradeOffer`
    /// (`GET https://api.steampowered.com/IEconService/GetTradeOffer/v1?access_token=…&tradeofferid=…`), as
    /// node-steam-tradeoffer-manager (`lib/index.js:525`) and aiosteampy (`webapi/services/econ.py:15-26`) ask for it.
    /// `None` when Steam has no such offer: an answer without one, or `X-eresult` NoMatch, which aiosteampy reads as
    /// none (`client/components/trade/trade.py:175-198`).
    ///
    /// Seen live (2026-10-22): an id with no offer got HTTP 200, `x-eresult: 42` (NoMatch) and `{"response":{}}`.
    ///
    /// # Errors
    ///
    /// [`TradeError::MalformedOffer`] if Steam sent the offer but it could not be read. An offer whose items couldn't
    /// all be read is returned, with [`Offer::has_malformed_items`].
    pub async fn offer(&self, offer_id: OfferId) -> Result<Option<Offer>, TradeError> {
        let api_client = self.web_api().await?;
        let answer = trade_offer_request(&api_client, offer_id).execute_with_response().await;
        offer_of(offer_id, answer)
    }

    /// This account's trades that `query` asks for, newest first, from `GetTradeHistory`.
    ///
    /// # Errors
    ///
    /// [`TradeError::UnexpectedResponse`] if a trade could not be read: never a list that leaves one out.
    pub async fn trades(&self, query: TradeQuery) -> Result<Vec<Trade>, TradeError> {
        let history: GetTradeHistoryResponse = self
            .web_api()
            .await?
            .get()
            .IEconService()
            .GetTradeHistory(query.max, query.include_failed, false, None, None, None, None, None)
            .execute_with_response()
            .map_err(web_api_error)
            .await?;
        trades_of(history)
    }

    /// The trade `trade_id`, among the last trades Steam returns ([`TradeQuery::default`]: up to 500 asked, failed
    /// ones left out). aiosteampy notes that Steam returns at most 100 with an access token
    /// (`webapi/services/econ.py:7`); not verified: live, all 34 trades of an account came.
    pub async fn trade(&self, trade_id: TradeId) -> Result<Option<Trade>, TradeError> {
        let trades = self.trades(TradeQuery::default()).await?;
        Ok(trades.into_iter().find(|trade| trade.id == trade_id))
    }

    /// How long a trade with the owner of `link` would be held, from `GetTradeHoldDurations` as ArchiSteamFarm asks
    /// (`ArchiSteamFarm/Steam/Integration/ArchiWebHandler.cs:1771-1837`): Steam's `both_escrow`, zero when there is no
    /// hold. `GET https://api.steampowered.com/IEconService/GetTradeHoldDurations/v1` with `access_token`,
    /// `steamid_target` and `trade_offer_access_token`.
    ///
    /// Seen live (2026-10-22): two accounts without a hold got `both_escrow` 0 (`Ok` of zero). A link whose token
    /// isn't the partner's got HTTP 200, `x-eresult: 15` and `{"response":{}}`: [`TradeError::Steam`] with
    /// `EResult::AccessDenied`.
    ///
    /// # Errors
    ///
    /// [`TradeError::Steam`] with Steam's `x-eresult` (`AccessDenied` for a wrong token).
    /// [`TradeError::TradeHoldUnknown`] when an answer without an error code has no `both_escrow`; not seen live,
    /// ArchiSteamFarm treats it as a failure (`ArchiWebHandler.cs:1829-1835`).
    pub async fn trade_hold(&self, link: &TradeLink) -> Result<Duration, TradeError> {
        let api_client = self.web_api().await?;
        let answer = trade_hold_request(&api_client, link)
            .execute_with_response()
            .map_err(web_api_error)
            .await?;
        hold_of(answer)
    }

    /// `steamid`'s inventory in `context`, every page of it, as Steam sent it: read its items with
    /// [`Inventory::pages`] and [`InventoryPage::items`], which allocate nothing per item.
    ///
    /// `GET https://steamcommunity.com/inventory/{steamid}/{appid}/{contextid}?l=english&count=1000` with the session's
    /// cookies, then `&start_assetid=…` while Steam says more items come (not seen live: our inventories fit one page).
    /// Another account's inventory must be public; a private one is an HTTP 403. Only the owner sees
    /// [`InventoryContext::CS2_TRADE_PROTECTED`].
    ///
    /// Steam rate limits this endpoint (HTTP 429, [`TradeError::is_too_many_requests`]).
    ///
    /// # Errors
    ///
    /// HTTP 429, other non-2xx answers and a redirect to the login page, as for every Steam Community request;
    /// [`TradeError::UnexpectedResponse`] if the answer isn't an inventory with `success: 1`, or a page can't be
    /// followed (a repeated one, or more than 100): never an incomplete list.
    pub async fn inventory(&self, steamid: SteamID, context: InventoryContext) -> Result<Inventory, TradeError> {
        let steam64 = steamid.to_steam64();
        read_all_inventory_pages(|start| async move {
            let url = inventory_url(steam64, context, start.as_deref());
            self.get_body(url).await
        })
        .await
    }

    /// This account's inventory in `context`: [`SteamTradeManager::inventory`] with its own SteamID.
    pub async fn own_inventory(&self, context: InventoryContext) -> Result<Inventory, TradeError> {
        self.inventory(self.authenticator.steam_id(), context).await
    }

    /// This account's trade link, which others send it offers with, read from its trade offer privacy page
    /// (`GET https://steamcommunity.com/profiles/{steamid}/tradeoffers/privacy`), as node-steamcommunity
    /// (`index.js:310-325`, `getTradeURL`) and aiosteampy (`aiosteampy/client/state.py:280-293`, `sync_trade_token`)
    /// do.
    ///
    /// The Web API has no `IEconService/GetTradeOfferAccessToken`: it answered HTTP 404 with an access token (seen
    /// live on 2026-10-22). ArchiSteamFarm asks for the token over CM instead
    /// (`ArchiSteamFarm/Steam/Integration/ArchiHandler.cs:488-511`).
    ///
    /// Seen live (2026-10-22, after `login_with_client`): the page redirects to `market/eligibilitycheck/` and back,
    /// then answers with the link.
    ///
    /// # Errors
    ///
    /// Those of every Steam Community request (see the crate docs), [`TradeError::UnexpectedResponse`] if the page has
    /// no trade link of this account.
    pub async fn my_trade_link(&self) -> Result<TradeLink, TradeError> {
        let steam_id = self.authenticator.steam_id();
        let url = format!(
            "https://steamcommunity.com/profiles/{}/tradeoffers/privacy",
            steam_id.to_steam64()
        );
        trade_link_on_privacy_page(&self.get_body(url).await?, steam_id)
    }

    /// Creates `offer` and confirms it: [`SteamTradeManager::create_offer`], a second's wait, then
    /// [`SteamTradeManager::confirm_offers`] with its id. Returns its id.
    ///
    /// A confirmation found is always accepted, whatever Steam's answer to the create said. Without one, that answer
    /// decides, as node-steam-tradeoffer-manager reads it (`lib/classes/TradeOffer.js:452-475`): [`Needs::Nothing`]
    /// is a sent offer, even when the confirmations could not be fetched.
    ///
    /// The offer isn't accepted by this account after the create, as no sibling library does (node-steam-tradeoffer-
    /// manager refuses, `lib/classes/TradeOffer.js` ~558-561): this crate did until 0.7.
    ///
    /// # Errors
    ///
    /// Those of `create_offer`. Once the offer exists, every error carries its id:
    /// [`TradeError::ConfirmationNotFound`] if Steam said it needs a mobile confirmation and none was found,
    /// [`TradeError::OfferCreatedButNotConfirmed`] for any other failure (with [`TradeError::NeedsEmailConfirmation`]
    /// inside if Steam waits for an email one).
    pub async fn send(&self, offer: &OfferRequest) -> Result<OfferId, TradeError> {
        let created = self.create_offer(offer).await?;
        Delay::new(STANDARD_DELAY).await;
        sent_offer(created, self.confirm(&[created.id]).await)
    }

    /// Accepts `offer`, an offer made to this account, and confirms the accept when Steam asks for a mobile
    /// confirmation: [`SteamTradeManager::post_accept`], then [`SteamTradeManager::confirm_offers`].
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    ///
    /// Returns the trade's id from Steam's answer to the accept (`{"tradeid": "…"}`, seen live on 2026-10-20); `None`
    /// after a mobile confirmation (see [`Accepted::trade_id`]).
    ///
    /// One accept at a time per manager, as [`SteamTradeManager::post_accept`].
    ///
    /// # Errors
    ///
    /// [`TradeError::NeedsEmailConfirmation`] if Steam waits for an email confirmation instead,
    /// [`TradeError::ConfirmationNotFound`] if it asked for a mobile one and none was found.
    pub async fn accept(&self, offer: &Offer) -> Result<Accepted, TradeError> {
        let _accepting = self.accepting.lock().await;
        let posted = self.post_accept_unlocked(offer).await?;
        let trade_id = accept_steps(offer.id, posted, || {
            self.confirm_offers(std::slice::from_ref(&offer.id))
        })
        .await?;
        Ok(Accepted { trade_id })
    }

    /// The body of a GET of `url` with the session's cookies, if Steam answered 2xx and didn't redirect to its login
    /// page.
    async fn get_body(&self, url: String) -> Result<String, TradeError> {
        let response = self
            .authenticator
            .request_custom_endpoint(url, Method::GET, None, None::<()>)
            .err_into::<InternalError>()
            .await?;
        let (status, url, body) = community_answer(response).await?;
        status_check(status, &url, &body)?;
        Ok(body)
    }

    /// Posts `request` and reads Steam's answer as an `OUTPUT`, see [`answer_of`].
    async fn send_request<OUTPUT>(&self, request: PreparedRequest) -> Result<OUTPUT, TradeError>
    where
        OUTPUT: DeserializeOwned,
    {
        let (status, url, body) = self.post_community_form(request).await?;
        check_response(status, &url, &body)?;
        answer_of(&body)
    }

    /// Adds the session's `sessionid` to the form, posts it, and returns the answer's status, URL and body, unless
    /// Steam redirected to its login page.
    async fn post_community_form(&self, request: PreparedRequest) -> Result<(StatusCode, Url, String), TradeError> {
        let session_id_cookie = self
            .authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "sessionid")
            .ok_or(TradeError::NoSessionId)?;

        let form = with_session_id(request.form, session_id_cookie);

        let response = self
            .authenticator
            .request_custom_endpoint(request.url, Method::POST, request.headers, Some(form))
            .err_into::<InternalError>()
            .await?;
        community_answer(response).await
    }
}

/// The status, URL (after redirects) and body of Steam's answer to a Steam Community request, unless Steam redirected
/// it to its login page ([`login_redirect_check`]).
async fn community_answer(response: Response) -> Result<(StatusCode, Url, String), TradeError> {
    let status = response.status();
    let url = response.url().clone();
    login_redirect_check(&url)?;
    let body = response.text().err_into::<InternalError>().await?;
    Ok((status, url, body))
}

#[cfg(feature = "cs2")]
impl SteamTradeManager {
    /// Makes [`SteamTradeManager::accept_and_find_items`] wait up to `timeout` for the received items, instead of
    /// 60 s.
    pub fn with_received_items_timeout(mut self, timeout: Duration) -> Self {
        self.received_items_timeout = timeout;
        self
    }

    /// Accepts `offer` and finds where the items received went, through CS2's game coordinator: since 2026-04 Steam's
    /// trade history doesn't say for CS2 trades. Use it instead of [`SteamTradeManager::accept`] when the received
    /// items matter. Feature `cs2`.
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    ///
    /// `fingerprints` are the pattern and wear of the partner's copies, by their asset id in the offer, as read from
    /// the partner's inventory before the trade ([`SteamTradeManager::inventory`], then [`Inventory::fingerprints`]):
    /// offers and trade history carry none. Any may be missing; commodities have none.
    ///
    /// `cs2` is this account's session with CS2's game coordinator, borrowed for the call. Its events are not taken:
    /// the call only looks at [`Cs2::inventory`], which reads what came and leaves them as events (kept up to `Cs2`'s
    /// event capacity, then counted as `Lagged`). Steps:
    ///
    /// 1. Takes the offer's items to receive ([`Offer::take`]). Entries that couldn't be read are left out (check
    ///    [`Offer::has_malformed_items`] first); stackable items (`amount` > 1) count as one.
    /// 2. Notes the game coordinator's item ids. Fails with [`TradeError::Cs2InventoryUnavailable`] if its inventory
    ///    isn't current: nothing is accepted.
    /// 3. Accepts, as [`SteamTradeManager::accept`].
    /// 4. Waits for the new ids, looking at the local inventory every 250 ms (no request), until each CS2 item with a
    ///    fingerprint has a new item that matches it and the new items left are at least as many as the CS2 items
    ///    without one, or 60 s passed ([`SteamTradeManager::with_received_items_timeout`]). Items from elsewhere
    ///    (a drop, another trade) don't end it early for an item with a fingerprint. While the inventory isn't
    ///    current, it waits for the game coordinator to send it again; nothing greets the game coordinator meanwhile,
    ///    so a session lost during the wait ends it.
    /// 5. Places each received item among the new items of the last look, as
    ///    [`SteamTradeManager::find_received_items`] does: an item with a fingerprint on a new item that matches it
    ///    ([`Located::Exact`]), the others by kind ([`Located::AnyOfKind`]), read once from our own web inventory
    ///    (CS2's context 16, where received items land). After a timeout, those that didn't come are
    ///    [`Located::NotFound`].
    ///
    /// Web reads: none when every CS2 item received has a fingerprint, else one of our own. Steam rate limits them.
    ///
    /// One accept at a time per manager: this holds the accept lock of `accept` until the items are placed.
    ///
    /// Seen live once (2026-10-22): an offer needing a mobile confirmation, one CS2 item received, placed
    /// [`Located::Exact`] from the game coordinator alone. [`Located::AnyOfKind`] and the timeout here were not.
    ///
    /// # Errors
    ///
    /// Before Steam took the accept, the plain error: nothing was accepted. If confirming it then fails (finding or
    /// accepting the mobile confirmation, or Steam waits for an email one), [`TradeError::AcceptedButNotConfirmed`]
    /// with the item ids noted before the accept: the trade may still happen, check the offer's state, then pass those
    /// ids to `find_received_items`. Once accepted, [`TradeError::AcceptedButItemsNotFound`] with the trade id and
    /// those ids, for a failure of the game coordinator (the client stopped) or of the read of our web inventory: pass
    /// them to `find_received_items` later. Items not found in time are no error: [`Located::NotFound`].
    pub async fn accept_and_find_items(
        &self,
        offer: &Offer,
        fingerprints: &HashMap<AssetId, Fingerprint>,
        cs2: &mut Cs2,
    ) -> Result<AcceptedOffer, TradeError> {
        let _accepting = self.accepting.lock().await;
        received::accept_and_locate(
            offer.id,
            offer.take.clone(),
            fingerprints,
            cs2,
            move || async move {
                let posted = self
                    .post_accept_unlocked(offer)
                    .await
                    .map_err(received::AcceptFailed::NotSent)?;
                accept_steps(offer.id, posted, || {
                    self.confirm_offers(std::slice::from_ref(&offer.id))
                })
                .await
                .map_err(received::AcceptFailed::NotConfirmed)
            },
            move || self.own_inventory(InventoryContext::CS2_TRADE_PROTECTED),
            received::Wait::new(self.received_items_timeout),
        )
        .await
    }

    /// Finds where the items received went in an offer the partner accepted, such as one this account sent, through
    /// CS2's game coordinator. Nothing is accepted and nothing is waited for. Feature `cs2`.
    ///
    /// `offer` is the accepted offer ([`SteamTradeManager::offer`], or [`SteamTradeManager::offers`] with
    /// [`OfferQuery::ALL`]); its items to receive ([`Offer::take`]) are those to find, with the partner's asset ids.
    /// Entries that couldn't be read are left out (check [`Offer::has_malformed_items`] first); stackable items
    /// (`amount` > 1) count as one.
    ///
    /// The caller says what it knows:
    ///
    /// - `known_ids`: the game coordinator item ids (the asset ids) that aren't from this offer, such as the items held
    ///   before it and those other offers brought, or [`AcceptedOffer::known_ids`]. The items of `cs2`'s inventory not
    ///   in it are the new ones. Include the new ids earlier calls found for other trades: else two offers with the
    ///   same commodity can be given the same new id ([`Located::AnyOfKind`] takes the first free one by id).
    /// - `fingerprints`: the pattern and wear of the partner's copies, by their asset id in the offer, as read from the
    ///   partner's inventory before the trade ([`Inventory::fingerprints`]); any may be missing, and commodities have
    ///   none.
    ///
    /// Steps: looks once at [`Cs2::inventory`] (local state, no request; its events are not taken), and places each
    /// item. An item with a fingerprint is placed on the closest new item that matches it, the same pattern and a wear
    /// within 1e-6 (a wear rounded to 6 decimals still matches), the lowest id of equally close ones:
    /// [`Located::Exact`], else [`Located::NotFound`]. Then each item without one (every commodity) on the first new
    /// item left that our web inventory lists with its kind (`classid`): [`Located::AnyOfKind`], else
    /// [`Located::NotFound`]. Kinds are read once, and only when an item has no fingerprint, in CS2's context 16:
    /// received items land there and stay during the trade protection (seen live right after the trade). Where they
    /// are once it ended isn't verified, so for an offer older than its protection, commodities may be
    /// [`Located::NotFound`].
    ///
    /// Seen live once (2026-10-22): one case received, placed [`Located::AnyOfKind`] from context 16.
    /// [`Located::Exact`] here was not.
    ///
    /// # Errors
    ///
    /// [`TradeError::OfferNotAccepted`] if `offer` isn't [`OfferState::Accepted`]: nothing was looked up.
    /// [`TradeError::Cs2InventoryUnavailable`] while the game coordinator's inventory isn't current (it can't tell
    /// what is there): nothing was looked up, try again once it is. Not when the offer has no CS2 item to receive:
    /// those are [`Located::NotFound`] without a look. [`TradeError::GameCoordinatorError`] if the client stopped. The
    /// error of a web inventory read that failed, such as a rate limited one ([`TradeError::is_too_many_requests`]):
    /// nothing is placed, try again later. Items not found are no error: [`Located::NotFound`].
    pub async fn find_received_items(
        &self,
        offer: &Offer,
        known_ids: &HashSet<AssetId>,
        fingerprints: &HashMap<AssetId, Fingerprint>,
        cs2: &mut Cs2,
    ) -> Result<Vec<ReceivedItem>, TradeError> {
        let received = accepted_received_items(offer)?;
        received::find_and_locate(received, known_ids, fingerprints, cs2, || {
            self.own_inventory(InventoryContext::CS2_TRADE_PROTECTED)
        })
        .await
    }
}

/// The items `offer` gave this account, those that could be read; [`TradeError::OfferNotAccepted`] unless it was
/// accepted. An accepted CS2 offer is [`OfferState::Accepted`], its items there at once (seen live).
#[cfg(feature = "cs2")]
fn accepted_received_items(offer: &Offer) -> Result<Vec<OfferedItem>, TradeError> {
    if offer.state != OfferState::Accepted {
        return Err(TradeError::OfferNotAccepted {
            offer_id: offer.id,
            state: offer.state,
        });
    }
    Ok(offer.take.clone())
}

/// The form as it is sent.
fn with_session_id(mut form: Box<dyn HasSessionID>, sessionid: String) -> Arc<dyn HasSessionID> {
    form.set_sessionid(sessionid);
    Arc::from(form)
}

/// Steam's answer `body`, after [`check_response`], as an `OUTPUT`. A body that isn't one is
/// [`TradeError::Steam`] when it has a JSON `success` code, else [`TradeError::UnexpectedResponse`].
///
/// The answers read here (create, accept, decline, cancel) have only optional fields, so any JSON object is one, also
/// `{"success":16}`: then a create is `UnexpectedResponse` (no offer id), a decline or a cancel is `UnexpectedResponse`
/// (not about the offer), and an accept is [`Needs::Nothing`] without a trade id. Whether Steam answers these requests
/// with a `success` code isn't known: none of the answers in the fixtures has one.
fn answer_of<OUTPUT: DeserializeOwned>(body: &str) -> Result<OUTPUT, TradeError> {
    serde_json::from_str::<OUTPUT>(body).map_err(|_| {
        match serde_json::from_str::<TradeOfferGenericErrorResponse>(body) {
            Ok(TradeOfferGenericErrorResponse { eresult: Some(eresult) }) => {
                TradeError::Steam { eresult, message: None }
            }
            _ => TradeError::UnexpectedResponse(body.to_string()),
        }
    })
}

/// The `partner` sent to accept the offer `offer_id`.
///
/// Built from the offer id, not from the offer's partner: wrong by reading, but it is what bots send today and Steam
/// takes it (bug 3 in `workingcontext.md`). Changing it needs a live check.
fn accept_partner(offer_id: OfferId) -> u64 {
    SteamID::from_steam3(offer_id.0 as u32, None, None).to_steam64()
}

/// The offer Steam's answer to a create is about.
fn created_offer(answer: &TradeOfferCreateResponse) -> Result<CreatedOffer, TradeError> {
    let id = answer
        .tradeofferid
        .as_deref()
        .and_then(|id| OfferId::from_str(id).ok())
        .ok_or_else(|| TradeError::UnexpectedResponse(format!("Steam's answer has no trade offer id: {:?}", answer)))?;
    Ok(CreatedOffer {
        id,
        needs: Needs::of(answer),
    })
}

/// Steam's answer to an accept.
fn posted_accept(answer: &TradeOfferCreateResponse) -> PostedAccept {
    PostedAccept {
        trade_id: accepted_trade_id(answer),
        needs: Needs::of(answer),
    }
}

/// The trade id in Steam's answer to an accept. One that isn't a number is logged and left out: the accept is done
/// either way.
fn accepted_trade_id(answer: &TradeOfferCreateResponse) -> Option<TradeId> {
    let tradeid = answer.tradeid.as_deref()?;
    match TradeId::from_str(tradeid) {
        Ok(tradeid) => Some(tradeid),
        Err(_) => {
            warn!(
                "Steam's answer to an accept has a trade id that isn't a number: {:?}",
                tradeid
            );
            None
        }
    }
}

/// Whether Steam's answer to a decline or a cancel is about `offer_id`.
fn answer_about(offer_id: OfferId, answer: &TradeOfferCancelResponse) -> Result<(), TradeError> {
    if answer.tradeofferid.as_deref().map(OfferId::from_str) == Some(Ok(offer_id)) {
        return Ok(());
    }
    Err(TradeError::UnexpectedResponse(format!(
        "Steam's answer isn't about trade offer {}: {:?}",
        offer_id, answer
    )))
}

/// The trade link of `steam_id` with `token`, validated as any trade link.
fn trade_link_of(steam_id: SteamID, token: &str) -> Result<TradeLink, TradeError> {
    let link = format!(
        "https://steamcommunity.com/tradeoffer/new/?partner={}&token={}",
        steam_id.account_id(),
        token
    );
    Ok(TradeLink::parse(&link)?)
}

lazy_static::lazy_static! {
    /// The link as the privacy page shows it, in an attribute (`&amp;`) or as text; node-steamcommunity's
    /// (`index.js:317`), ASCII only.
    static ref PRIVACY_PAGE_TRADE_LINK: regex::Regex = regex::Regex::new(
        r"https://steamcommunity\.com/tradeoffer/new/\?partner=([0-9]+)&(?:amp;)?token=([A-Za-z0-9_-]+)"
    )
    .unwrap();
}

/// The trade link of `steam_id` on its trade offer privacy page. A link of another account is not taken.
fn trade_link_on_privacy_page(page: &str, steam_id: SteamID) -> Result<TradeLink, TradeError> {
    PRIVACY_PAGE_TRADE_LINK
        .captures_iter(page)
        .find(|link| link[1].parse::<u32>().ok() == Some(steam_id.account_id()))
        .ok_or_else(|| {
            TradeError::UnexpectedResponse(
                "The trade offer privacy page has no trade link of this account.".to_string(),
            )
        })
        .and_then(|link| trade_link_of(steam_id, &link[2]))
}

/// Sends `acknowledge()` unless `acknowledged` says Steam already took it, and notes when it does. A failure is logged
/// and leaves it to the next call: the confirmation is still tried, since an account may not need the acknowledgement.
async fn acknowledge_once<A, AF>(acknowledged: &AtomicBool, acknowledge: A)
where
    A: FnOnce() -> AF,
    AF: Future<Output = Result<(), TradeError>>,
{
    if acknowledged.load(Ordering::Acquire) {
        return;
    }
    match acknowledge().await {
        Ok(()) => acknowledged.store(true, Ordering::Release),
        Err(error) => warn!("Could not acknowledge trade protection, confirming anyway: {}", error),
    }
}

/// The confirmations of `confirmations` that are of an offer of `offer_ids`, each offer's first one, and which offers
/// have one. `is_of(confirmation, offer_id)` tells whether a confirmation is of an offer.
fn confirmations_of<C>(
    offer_ids: &[OfferId],
    mut confirmations: Vec<C>,
    is_of: impl Fn(&C, OfferId) -> bool,
) -> (Vec<C>, Confirmed) {
    let mut found = Vec::new();
    let mut confirmed = Confirmed::default();
    for &offer_id in offer_ids {
        match confirmations
            .iter()
            .position(|confirmation| is_of(confirmation, offer_id))
        {
            Some(index) => {
                found.push(confirmations.swap_remove(index));
                confirmed.confirmed.push(offer_id);
            }
            None => confirmed.not_found.push(offer_id),
        }
    }
    (found, confirmed)
}

/// The steps of [`SteamTradeManager::confirm_offers`] for `offer_ids`, given as closures so that tests can stand in for
/// Steam: `fetch()` the account's confirmations (getlist), and only if one of them is of an offer of `offer_ids`
/// (`is_of`), `acknowledge()` trade protection, then `process(found)` once with every one found (multiajaxop). Steam
/// refuses to confirm trades of trade-protected items before the acknowledgement (seen live 2026-10-20). Nothing is
/// sent for no offer.
async fn confirm_steps<C, F, FF, A, AF, P, PF>(
    offer_ids: &[OfferId],
    fetch: F,
    is_of: impl Fn(&C, OfferId) -> bool,
    acknowledge: A,
    process: P,
) -> Result<Confirmed, ConfirmFailed>
where
    F: FnOnce() -> FF,
    FF: Future<Output = Result<Vec<C>, TradeError>>,
    A: FnOnce() -> AF,
    AF: Future<Output = ()>,
    P: FnOnce(Vec<C>) -> PF,
    PF: Future<Output = Result<(), TradeError>>,
{
    if offer_ids.is_empty() {
        return Ok(Confirmed::default());
    }
    let confirmations = fetch().await.map_err(ConfirmFailed::Fetch)?;
    let (found, confirmed) = confirmations_of(offer_ids, confirmations, is_of);
    if !found.is_empty() {
        acknowledge().await;
        process(found).await.map_err(ConfirmFailed::Process)?;
    }
    Ok(confirmed)
}

/// What [`SteamTradeManager::send`] returns, once the offer `created` was created and `confirmed` tells how its
/// confirmation went.
///
/// A confirmation found is always accepted, as this crate always did. Without one, the answer to the create decides:
/// [`Needs::MobileConfirmation`] is an error, never a success; [`Needs::EmailConfirmation`] too, since this crate
/// can't give it; [`Needs::Nothing`] is a sent offer (e.g. we give nothing), also when the confirmations could not be
/// fetched. A confirmation that Steam refused to accept is an error whatever the answer said.
fn sent_offer(created: CreatedOffer, confirmed: Result<Confirmed, ConfirmFailed>) -> Result<OfferId, TradeError> {
    let offer_id = created.id;
    let found = match confirmed {
        Ok(confirmed) => confirmed.confirmed.contains(&offer_id),
        Err(ConfirmFailed::Fetch(error)) if created.needs == Needs::Nothing => {
            warn!(
                "Could not fetch confirmations for trade offer {}, which Steam said needs none: {}",
                offer_id, error
            );
            return Ok(offer_id);
        }
        Err(failed) => return Err(offer_created_error(offer_id, failed.into())),
    };
    match (found, created.needs) {
        (true, _) | (false, Needs::Nothing) => Ok(offer_id),
        (false, Needs::MobileConfirmation) => Err(offer_created_error(
            offer_id,
            TradeError::ConfirmationNotFound { offer_id },
        )),
        (false, Needs::EmailConfirmation) => Err(offer_created_error(
            offer_id,
            TradeError::NeedsEmailConfirmation { offer_id },
        )),
    }
}

/// The error of [`SteamTradeManager::send`] after the offer `offer_id` was created:
/// [`TradeError::ConfirmationNotFound`] as is, since it has the id, any other wrapped in
/// [`TradeError::OfferCreatedButNotConfirmed`].
fn offer_created_error(offer_id: OfferId, error: TradeError) -> TradeError {
    match error {
        error @ TradeError::ConfirmationNotFound { .. } => error,
        error => TradeError::OfferCreatedButNotConfirmed {
            offer_id,
            source: Box::new(error),
        },
    }
}

/// The steps of [`SteamTradeManager::accept`] once Steam answered the accept of `offer_id` with `posted`: the trade's
/// id, as `accept` returns it. `confirm()` confirms the accept, given as a closure so that tests can stand in for
/// Steam.
///
/// [`Needs::Nothing`]: done, with Steam's trade id. [`Needs::EmailConfirmation`]:
/// [`TradeError::NeedsEmailConfirmation`]. [`Needs::MobileConfirmation`]: `confirm()`, then no trade id, or
/// [`TradeError::ConfirmationNotFound`] if it found none.
async fn accept_steps<C, CF>(offer_id: OfferId, posted: PostedAccept, confirm: C) -> Result<Option<TradeId>, TradeError>
where
    C: FnOnce() -> CF,
    CF: Future<Output = Result<Confirmed, TradeError>>,
{
    match posted.needs {
        Needs::Nothing => Ok(posted.trade_id),
        Needs::EmailConfirmation => Err(TradeError::NeedsEmailConfirmation { offer_id }),
        Needs::MobileConfirmation => {
            if confirm().await?.confirmed.contains(&offer_id) {
                Ok(None)
            } else {
                Err(TradeError::ConfirmationNotFound { offer_id })
            }
        }
    }
}

/// The offers that could not be read, in sent then received order.
fn malformed_offers(offers: &GetTradeOffersResponse) -> impl Iterator<Item = &MalformedOffer> {
    offers
        .response
        .malformed_offers_sent
        .iter()
        .chain(&offers.response.malformed_offers_received)
}

/// [`TradeError::MalformedOffers`] if Steam sent an offer that could not be read.
fn fail_on_malformed_offers(offers: &GetTradeOffersResponse) -> Result<(), TradeError> {
    let malformed: Vec<&MalformedOffer> = malformed_offers(offers).collect();
    match malformed.first() {
        None => Ok(()),
        Some(first) => Err(TradeError::MalformedOffers {
            offer_ids: malformed
                .iter()
                .map(|offer| offer.tradeofferid().map(OfferId))
                .collect(),
            reason: first.reason.clone(),
        }),
    }
}

/// The offers of `response`, sent then received. [`TradeError::MalformedOffers`] with every offer that can't be one.
fn offers_of(response: GetTradeOffersResponse) -> Result<Vec<Offer>, TradeError> {
    let response = response.response;
    let all = response
        .trade_offers_sent
        .into_iter()
        .flatten()
        .chain(response.trade_offers_received.into_iter().flatten());

    let mut offers = Vec::new();
    let mut malformed = Vec::new();
    let mut first_reason = None;
    for offer in all {
        let offer_id = OfferId(offer.tradeofferid);
        match Offer::of(offer) {
            Ok(offer) => offers.push(offer),
            Err(reason) => {
                malformed.push(Some(offer_id));
                first_reason.get_or_insert(reason);
            }
        }
    }
    match first_reason {
        None => Ok(offers),
        Some(reason) => Err(TradeError::MalformedOffers {
            offer_ids: malformed,
            reason,
        }),
    }
}

/// The offer `offer_id` in `answer`, Steam's answer to `GetTradeOffer`: `None` for an answer without one, or for
/// `X-eresult` NoMatch.
fn offer_of(
    offer_id: OfferId,
    answer: Result<GetTradeOfferResponse, SteamAPIError>,
) -> Result<Option<Offer>, TradeError> {
    let answer = match answer {
        Ok(answer) => answer.response,
        Err(SteamAPIError::EResult(EResult::NoMatch, _)) => return Ok(None),
        Err(error) => return Err(web_api_error(error)),
    };
    match (answer.offer, answer.malformed_offer) {
        (Some(offer), _) => Offer::of(offer)
            .map(Some)
            .map_err(|reason| TradeError::MalformedOffer { offer_id, reason }),
        (None, Some(malformed)) => Err(TradeError::MalformedOffer {
            offer_id,
            reason: malformed.reason,
        }),
        (None, None) => Ok(None),
    }
}

/// The trades of `history`. [`TradeError::UnexpectedResponse`] if one can't be read.
fn trades_of(history: GetTradeHistoryResponse) -> Result<Vec<Trade>, TradeError> {
    history
        .response
        .trades
        .into_iter()
        .map(|trade| {
            let trade_id = trade.tradeid;
            Trade::of(trade).map_err(|reason| {
                TradeError::UnexpectedResponse(format!("Trade `{}` can't be read: {}", trade_id, reason))
            })
        })
        .collect()
}

/// The hold of Steam's answer to `GetTradeHoldDurations`: `both_escrow`, as ArchiSteamFarm reads it
/// (`ArchiSteamFarm/Steam/Integration/ArchiWebHandler.cs:1829`).
fn hold_of(answer: GetTradeHoldDurationsResponseBase) -> Result<Duration, TradeError> {
    let hold = answer.response.both_escrow.ok_or(TradeError::TradeHoldUnknown)?;
    let seconds = number(hold.escrow_end_duration_seconds, "escrow_end_duration_seconds")
        .map_err(TradeError::UnexpectedResponse)?;
    Ok(Duration::from_secs(seconds))
}

/// The request of one page of `GetTradeOffers`. Without a cursor, it is the request this crate always sent.
fn trade_offers_request(api_client: &SteamAPI, query: OfferQuery, cursor: Option<u32>) -> GetTradeOffers<'_> {
    let request = api_client.get().IEconService().GetTradeOffers(
        query.sent,
        query.received,
        MAX_HISTORICAL_CUTOFF,
        Some(query.active_only),
        None,
        None,
        None,
    );
    match cursor {
        Some(cursor) => request.cursor(cursor),
        None => request,
    }
}

/// The request of `GetTradeOffer` for `offer_id`: its id, nothing else, as node-steam-tradeoffer-manager sends.
fn trade_offer_request(api_client: &SteamAPI, offer_id: OfferId) -> GetTradeOffer<'_> {
    api_client.get().IEconService().GetTradeOffer(offer_id.0, None)
}

/// The request of `GetTradeHoldDurations` for the owner of `link`.
fn trade_hold_request<'a>(api_client: &'a SteamAPI, link: &TradeLink) -> GetTradeHoldDurations<'a> {
    api_client
        .get()
        .IEconService()
        .GetTradeHoldDurations(link.partner().to_steam64(), link.token().to_string())
}

/// Reads the first page with `page(None)`, then the next ones while Steam sends a cursor, and merges them.
///
/// Every page or an error, never an incomplete list: an error on any page is returned as is, like
/// node-steam-tradeoffer-manager does (`lib/index.js:597-600`), and so is [`TradeOfferPages::next_cursor`]'s.
async fn fetch_all_trade_offer_pages<F, Fut>(mut page: F) -> Result<GetTradeOffersResponse, TradeError>
where
    F: FnMut(Option<u32>) -> Fut,
    Fut: Future<Output = Result<GetTradeOffersResponse, TradeError>>,
{
    let mut pages = TradeOfferPages::new(page(None).await?);

    while let Some(cursor) = pages.next_cursor()? {
        pages.push(page(Some(cursor)).await?);
    }

    Ok(pages.into_response())
}

/// `GetTradeOffers` pages merged into one response.
struct TradeOfferPages {
    merged: GetTradeOffersResponse,
    requested_cursors: HashSet<u32>,
    pages: usize,
}

impl TradeOfferPages {
    fn new(mut first_page: GetTradeOffersResponse) -> Self {
        // Counted once within the first page too, as across pages.
        let response = &mut first_page.response;
        for malformed in [
            &mut response.malformed_offers_sent,
            &mut response.malformed_offers_received,
        ] {
            let page = std::mem::take(malformed);
            append_malformed_offers(malformed, page);
        }

        Self {
            merged: first_page,
            requested_cursors: HashSet::new(),
            pages: 1,
        }
    }

    /// The cursor of the next page to read, `None` after the last page. An error when the guard stops, since offers
    /// would be missing: a cursor that was already requested, or [`MAX_TRADE_OFFER_PAGES`] pages read.
    fn next_cursor(&mut self) -> Result<Option<u32>, TradeError> {
        let cursor = match self.merged.response.next_page_cursor() {
            Some(cursor) => cursor,
            None => return Ok(None),
        };

        if self.pages >= MAX_TRADE_OFFER_PAGES {
            return Err(TradeError::TradeOffersIncomplete {
                cursor,
                reason: format!("more than {} pages", MAX_TRADE_OFFER_PAGES),
            });
        }
        if !self.requested_cursors.insert(cursor) {
            return Err(TradeError::TradeOffersIncomplete {
                cursor,
                reason: "Steam sent this cursor before".to_string(),
            });
        }

        Ok(Some(cursor))
    }

    fn push(&mut self, page: GetTradeOffersResponse) {
        let merged = &mut self.merged.response;
        let page = page.response;

        append_offers(&mut merged.trade_offers_sent, page.trade_offers_sent);
        append_offers(&mut merged.trade_offers_received, page.trade_offers_received);
        append_malformed_offers(&mut merged.malformed_offers_sent, page.malformed_offers_sent);
        append_malformed_offers(&mut merged.malformed_offers_received, page.malformed_offers_received);
        merged.next_cursor = page.next_cursor;
        self.pages += 1;
    }

    fn into_response(self) -> GetTradeOffersResponse {
        self.merged
    }
}

/// Appends the offers of a page that are not in `merged` yet, in case an offer moves to the next page while they are
/// read.
fn append_offers(merged: &mut Option<Vec<TradeOffer_Trade>>, page: Option<Vec<TradeOffer_Trade>>) {
    let page = match page {
        Some(page) => page,
        None => return,
    };
    let merged = merged.get_or_insert_with(Vec::new);
    let known: HashSet<u64> = merged.iter().map(|offer| offer.tradeofferid).collect();

    merged.extend(page.into_iter().filter(|offer| !known.contains(&offer.tradeofferid)));
}

/// Like [`append_offers`], for offers that could not be read: the same id, or without one the same entry, is counted
/// once.
fn append_malformed_offers(merged: &mut Vec<MalformedOffer>, page: Vec<MalformedOffer>) {
    for offer in page {
        let known = merged.iter().any(|known| match offer.tradeofferid() {
            Some(id) => known.tradeofferid() == Some(id),
            None => known.raw == offer.raw,
        });
        if !known {
            merged.push(offer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::TradeError::GeneralError;

    fn get_tradeoffer_url_with_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn"
    }

    #[allow(clippy::too_many_lines)]
    fn sample_trade_history_response() -> GetTradeHistoryResponse {
        let response = r#"{
  "response": {
    "more": true,
    "trades": [
      {
        "tradeid": "3622543526924228084",
        "steamid_other": "76561198040191316",
        "time_init": 1603998438,
        "status": 3,
        "assets_given": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "15319724006",
            "amount": "1",
            "classid": "3035569977",
            "instanceid": "302028390",
            "new_assetid": "19793871926",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "3151905948742966439",
        "steamid_other": "76561198040191316",
        "time_init": 1594190957,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "17300115678",
            "amount": "1",
            "classid": "1989330488",
            "instanceid": "302028390",
            "new_assetid": "19034292089",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "3151905948742946486",
        "steamid_other": "76561198040191316",
        "time_init": 1594190486,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "17341684309",
            "amount": "1",
            "classid": "1989279043",
            "instanceid": "302028390",
            "new_assetid": "19034259977",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "3151905948734426645",
        "steamid_other": "76561198017653157",
        "time_init": 1593990409,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "8246208960",
            "amount": "1",
            "classid": "310776668",
            "instanceid": "302028390",
            "new_assetid": "19019879428",
            "new_contextid": "2"
          },
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "11589364986",
            "amount": "1",
            "classid": "469467368",
            "instanceid": "302028390",
            "new_assetid": "19019879441",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "2816382071757670028",
        "steamid_other": "76561198040191316",
        "time_init": 1587519425,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "17921552800",
            "amount": "1",
            "classid": "1989286992",
            "instanceid": "302028390",
            "new_assetid": "18426035472",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "2289455842905057389",
        "steamid_other": "76561198994791561",
        "time_init": 1582942255,
        "time_escrow_end": 1584238255,
        "status": 3,
        "assets_given": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "16832065568",
            "amount": "1",
            "classid": "1989312177",
            "instanceid": "302028390",
            "new_assetid": "18074934023",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "2022547174628342555",
        "steamid_other": "76561197966598809",
        "time_init": 1515645117,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "4345999",
            "amount": "1",
            "classid": "310777161",
            "instanceid": "188530139",
            "new_assetid": "13327873664",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "2022547174628335361",
        "steamid_other": "76561197976600825",
        "time_init": 1515644947,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "12792180950",
            "amount": "1",
            "classid": "2521767801",
            "instanceid": "0",
            "new_assetid": "13327860916",
            "new_contextid": "2"
          },
          {
            "appid": 447820,
            "contextid": "2",
            "assetid": "1667881814169014779",
            "amount": "1",
            "classid": "2219693199",
            "instanceid": "0",
            "new_assetid": "1827766562939536102",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "2022547174624411155",
        "steamid_other": "76561197971392179",
        "time_init": 1515552781,
        "status": 3,
        "assets_received": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "13213233361",
            "amount": "1",
            "classid": "2521767801",
            "instanceid": "0",
            "new_assetid": "13314519275",
            "new_contextid": "2"
          },
          {
            "appid": 447820,
            "contextid": "2",
            "assetid": "2364813217118906230",
            "amount": "1",
            "classid": "2219693201",
            "instanceid": "0",
            "new_assetid": "1827766492051349432",
            "new_contextid": "2"
          },
          {
            "appid": 578080,
            "contextid": "2",
            "assetid": "1807498550407081772",
            "amount": "1",
            "classid": "2451623575",
            "instanceid": "0",
            "new_assetid": "1827766492051349437",
            "new_contextid": "2"
          }
        ]
      },
      {
        "tradeid": "1640843092290105607",
        "steamid_other": "76561197998993178",
        "time_init": 1492806587,
        "status": 3,
        "assets_given": [
          {
            "appid": 730,
            "contextid": "2",
            "assetid": "4063307518",
            "amount": "1",
            "classid": "310779465",
            "instanceid": "188530139",
            "new_assetid": "9937692380",
            "new_contextid": "2"
          }
        ]
      }
    ]
  }
}
"#;
        serde_json::from_str::<GetTradeHistoryResponse>(&response).unwrap()
    }

    /// `GetTradeHistory` with the access token right after a CS2 trade (`fixtures/live/2026-10-20/ack-cycle.jsonl`),
    /// first two trades.
    fn live_trade_history_response() -> GetTradeHistoryResponse {
        serde_json::from_str(
            r#"{"response":{"more":false,"trades":[
              {"tradeid":"695401084301346863","steamid_other":"76561197961265729","time_init":1792510484,"status":3,
               "assets_received":[{"appid":730,"contextid":"2","assetid":"24234602605","amount":"1",
                 "classid":"241575744","instanceid":"302028390"}],
               "time_settlement":1793116800},
              {"tradeid":"4726007333531040160","steamid_other":"76561197961265731","time_init":1681996137,"status":3,
               "assets_given":[{"appid":730,"contextid":"2","assetid":"25838682254","amount":"1",
                 "classid":"4717330486","instanceid":"0","new_assetid":"29793633296","new_contextid":"2"}]}
            ]}}"#,
        )
        .unwrap()
    }

    fn trades(history: GetTradeHistoryResponse) -> Vec<Trade> {
        trades_of(history).unwrap()
    }

    fn trade_in(history: GetTradeHistoryResponse, id: u64) -> Option<Trade> {
        trades(history).into_iter().find(|trade| trade.id == TradeId(id))
    }

    fn cs2(id: u64) -> Asset {
        Asset::new(InventoryContext::CS2, AssetId(id))
    }

    #[test]
    fn new_asset_of_an_item() {
        let trade = trade_in(sample_trade_history_response(), 3622543526924228084).unwrap();
        assert_eq!(trade.new_asset(AssetId(15319724006)), Some(cs2(19793871926)));
        assert_eq!(trade.new_asset(AssetId(1)), None);
    }

    #[test]
    fn new_assets_of_a_trade() {
        let trade = trade_in(sample_trade_history_response(), 3151905948734426645).unwrap();
        assert!(trade.is_settled());
        let new: Vec<Option<Asset>> = trade.received.iter().map(|item| item.new_asset).collect();
        assert_eq!(new, vec![Some(cs2(19019879428)), Some(cs2(19019879441))]);
        assert!(trade.given.is_empty());
    }

    #[test]
    fn trade_not_in_history() {
        assert_eq!(trade_in(sample_trade_history_response(), 1), None);
    }

    #[test]
    fn unsettled_trade() {
        let history: GetTradeHistoryResponse = serde_json::from_str(
            r#"{"response":{"more":false,"trades":[{"tradeid":"42","steamid_other":"76561198040191316",
              "time_init":1603998438,"status":3,
              "assets_given":[
                {"appid":730,"contextid":"2","assetid":"100","amount":"1","classid":"1","instanceid":"0",
                 "new_assetid":"200","new_contextid":"2"},
                {"appid":730,"contextid":"2","assetid":"101","amount":"1","classid":"1","instanceid":"0"}
              ]}]}}"#,
        )
        .unwrap();

        let trade = trade_in(history, 42).unwrap();
        assert!(!trade.is_settled());
        assert_eq!(trade.new_asset(AssetId(100)), Some(cs2(200)));
        assert_eq!(trade.new_asset(AssetId(101)), None);
    }

    #[test]
    fn live_trades() {
        let trades = trades(live_trade_history_response());
        assert_eq!(
            trades[0],
            Trade {
                id: TradeId(695401084301346863),
                partner: SteamID::from_steam64(76561197961265729),
                status: TradeStatus::Complete,
                started: 1792510484,
                // 2026-10-27 16:00:00 UTC
                protected_until: Some(1793116800),
                received: vec![TradedItem {
                    asset: cs2(24234602605),
                    classid: 241575744,
                    instanceid: 302028390,
                    amount: 1,
                    new_asset: None,
                    rollback_new_asset: None,
                }],
                given: vec![],
            }
        );
        // a CS2 trade since 2026-04: no new asset id
        assert!(!trades[0].is_settled());
        assert!(trades[0].is_protected_at(1793116799));
        assert!(!trades[0].is_protected_at(1793116800));

        // an old trade: no protection, new asset ids
        assert_eq!(trades[1].protected_until, None);
        assert!(!trades[1].is_protected());
        assert_eq!(trades[1].time_left(), None);
        assert!(trades[1].is_settled());
        assert_eq!(trades[1].new_asset(AssetId(25838682254)), Some(cs2(29793633296)));
    }

    #[test]
    fn unreadable_trade_fails_the_list() {
        let history: GetTradeHistoryResponse = serde_json::from_str(
            r#"{"response":{"more":false,"trades":[{"tradeid":"42","steamid_other":"76561198040191316",
              "time_init":1603998438,"status":3,
              "assets_given":[{"appid":730,"contextid":"2","assetid":"100","classid":"1","instanceid":"0"}]}]}}"#,
        )
        .unwrap();
        match trades_of(history) {
            Err(TradeError::UnexpectedResponse(message)) => assert!(message.contains("amount"), "{}", message),
            other => panic!("{:?}", other),
        }
    }

    /// The form as steam-mobile encodes it (`serde_urlencoded::to_string` in `MobileClient::request`).
    fn sent_form(request: PreparedRequest) -> String {
        serde_urlencoded::to_string(with_session_id(request.form, "SESSION123".to_string())).unwrap()
    }

    fn referer(request: &PreparedRequest) -> Option<&str> {
        let headers = request.headers.as_ref()?;
        assert_eq!(headers.len(), 1, "{:?}", headers);
        Some(headers.get("Referer").unwrap().to_str().unwrap())
    }

    // Expected values were captured from the code of 0.7 (`prepare_offer`, `TradeKind::endpoint`, the header and form
    // building of the old `request()`), with the same inputs.

    #[test]
    fn create_request_is_unchanged() {
        let offer = OfferRequest::to(TradeLink::parse(get_tradeoffer_url_with_token()).unwrap())
            .give(cs2(17034419698))
            .give(Asset::new(InventoryContext::DOTA2, AssetId(1)))
            .take(cs2(18116227588))
            .message("hi & bye, ção=1");

        let request = PreparedRequest::create(&offer);
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/new/send");
        assert_eq!(referer(&request), Some("https://steamcommunity.com/tradeoffer/new"));
        assert_eq!(
            sent_form(request),
            "sessionid=SESSION123&serverid=1&partner=76561198040191316&captcha=&tradeoffermessage=hi+%26+bye%2C+%C3%A7%C3%A3o%3D1&json_tradeoffer=%7B%22newversion%22%3Atrue%2C%22version%22%3A2%2C%22me%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A730%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%2217034419698%22%7D%2C%7B%22appid%22%3A570%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%221%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%2C%22them%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A730%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%2218116227588%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%7D&trade_offer_create_params=%7B%22trade_offer_access_token%22%3A%22Ob27qXzn%22%7D"
        );

        let link = TradeLink::parse("https://steamcommunity.com/tradeoffer/new/?partner=1&token=a-b_c").unwrap();
        let offer = OfferRequest::to(link).give(Asset::new(InventoryContext::TF2, AssetId(5)));
        assert_eq!(
            sent_form(PreparedRequest::create(&offer)),
            "sessionid=SESSION123&serverid=1&partner=76561197960265729&captcha=&tradeoffermessage=&json_tradeoffer=%7B%22newversion%22%3Atrue%2C%22version%22%3A2%2C%22me%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A440%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%225%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%2C%22them%22%3A%7B%22assets%22%3A%5B%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%7D&trade_offer_create_params=%7B%22trade_offer_access_token%22%3A%22a-b_c%22%7D"
        );
    }

    #[test]
    fn stackable_amount_is_sent() {
        let offer = OfferRequest::to(TradeLink::parse(get_tradeoffer_url_with_token()).unwrap())
            .take_amount(Asset::new(InventoryContext::STEAM, AssetId(3)), 500);
        let form = sent_form(PreparedRequest::create(&offer));
        assert!(
            form.contains(
                "%7B%22appid%22%3A753%2C%22contextid%22%3A%226%22%2C%22amount%22%3A500%2C%22assetid%22%3A%223%22%7D"
            ),
            "{}",
            form
        );
    }

    #[test]
    fn accept_request_is_unchanged() {
        let offer_id = OfferId(4127395150);
        let partner = accept_partner(offer_id);
        assert_eq!(partner, 76561202087660878);

        let request = PreparedRequest::accept(offer_id, partner);
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/4127395150/accept");
        assert_eq!(
            referer(&request),
            Some("https://steamcommunity.com/tradeoffer/4127395150/")
        );
        assert_eq!(
            sent_form(request),
            "sessionid=SESSION123&serverid=1&partner=76561202087660878&captcha=&tradeofferid=4127395150"
        );
    }

    #[test]
    fn decline_and_cancel_requests_are_unchanged() {
        let request = PreparedRequest::decline(OfferId(4127395150));
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/4127395150/decline");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123");

        let request = PreparedRequest::cancel(OfferId(4127395150));
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/4127395150/cancel");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123");
    }

    #[test]
    fn acknowledge_trade_protection_request() {
        let request = PreparedRequest::acknowledge();
        assert_eq!(request.url, "https://steamcommunity.com/trade/new/acknowledge");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123&message=1");
    }

    #[test]
    fn decline_and_cancel_answers() {
        // Steam's answers in `ack-cycle.jsonl` (decline, then cancel)
        let answer = |body: &str| answer_of::<TradeOfferCancelResponse>(body).unwrap();
        assert!(answer_about(OfferId(9438483046), &answer(r#"{"tradeofferid":"9438483046"}"#)).is_ok());
        assert!(answer_about(OfferId(9438482627), &answer(r#"{"tradeofferid":"9438482627"}"#)).is_ok());

        for body in &[r#"{"tradeofferid":"9438482627"}"#, r#"{}"#, r#"{"tradeofferid":"abc"}"#] {
            match answer_about(OfferId(9438483046), &answer(body)) {
                Err(TradeError::UnexpectedResponse(_)) => {}
                other => panic!("{}: {:?}", body, other),
            }
        }
    }

    #[test]
    fn answers_that_are_not_the_expected_type() {
        match answer_of::<TradeOfferCancelResponse>(r#"{"success":16}"#) {
            // all fields optional: a code alone reads as an answer, as in 0.7
            Ok(answer) => assert_eq!(answer.tradeofferid, None),
            other => panic!("{:?}", other),
        }
        match answer_of::<TradeOfferCancelResponse>(r#"{"success":16,"tradeofferid":7}"#) {
            Err(error @ TradeError::Steam { message: None, .. }) => assert_eq!(error.eresult(), Some(EResult::Timeout)),
            other => panic!("{:?}", other),
        }
        for body in &["<html></html>", "", r#"{"success":false,"tradeofferid":7}"#] {
            match answer_of::<TradeOfferCancelResponse>(body) {
                Err(TradeError::UnexpectedResponse(answer)) => assert_eq!(&answer, body),
                other => panic!("{}: {:?}", body, other),
            }
        }
    }

    /// Runs `acknowledge_once` with an acknowledgement answering `answer`, and returns whether it was sent.
    fn acknowledge_with(acknowledged: &AtomicBool, answer: Result<(), TradeError>) -> bool {
        let sent = std::cell::Cell::new(false);
        futures::executor::block_on(acknowledge_once(acknowledged, || {
            sent.set(true);
            futures::future::ready(answer)
        }));
        sent.get()
    }

    #[test]
    fn trade_protection_is_acknowledged_once() {
        let acknowledged = AtomicBool::new(false);
        assert!(acknowledge_with(&acknowledged, Ok(())));
        assert!(acknowledged.load(Ordering::Acquire));
        assert!(!acknowledge_with(&acknowledged, Ok(())));
    }

    #[test]
    fn a_failed_acknowledgement_is_sent_again() {
        let acknowledged = AtomicBool::new(false);
        assert!(acknowledge_with(
            &acknowledged,
            Err(GeneralError("refused".to_string()))
        ));
        assert!(!acknowledged.load(Ordering::Acquire));
        assert!(acknowledge_with(&acknowledged, Ok(())));
        assert!(!acknowledge_with(&acknowledged, Ok(())));
    }

    fn create_response(body: &str) -> TradeOfferCreateResponse {
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn created_offer_of_the_answer() {
        // Steam's answer to the create of `ack-cycle.jsonl`
        assert_eq!(
            created_offer(&create_response(
                r#"{"tradeofferid":"9438482627","needs_mobile_confirmation":true,"needs_email_confirmation":false,"email_domain":"example.com"}"#
            ))
            .unwrap(),
            CreatedOffer {
                id: OfferId(9438482627),
                needs: Needs::MobileConfirmation,
            }
        );
        for body in &[
            r#"{}"#,
            r#"{"success":16}"#,
            r#"{"tradeofferid":"abc"}"#,
            r#"{"tradeofferid":null}"#,
        ] {
            match created_offer(&create_response(body)) {
                Err(TradeError::UnexpectedResponse(_)) => {}
                other => panic!("{}: {:?}", body, other),
            }
        }
    }

    #[test]
    fn trade_id_of_an_accept() {
        // Steam's answer to the accept of `ack-cycle.jsonl`
        assert_eq!(
            posted_accept(&create_response(r#"{"tradeid":"695401084301346863"}"#)),
            PostedAccept {
                trade_id: Some(TradeId(695401084301346863)),
                needs: Needs::Nothing,
            }
        );
        assert_eq!(
            accepted_trade_id(&create_response(r#"{"needs_mobile_confirmation":true}"#)),
            None
        );
        assert_eq!(accepted_trade_id(&create_response(r#"{"tradeid":"abc"}"#)), None);
    }

    #[test]
    fn needs_follow_the_flags() {
        let needs = |body: &str| posted_accept(&create_response(body)).needs;

        // absent flags: accepted, as node-steam-tradeoffer-manager reads them (e.g. a gift)
        assert_eq!(needs(r#"{"tradeid":"3151905948734426645"}"#), Needs::Nothing);
        assert_eq!(needs(r#"{}"#), Needs::Nothing);
        assert_eq!(
            needs(r#"{"needs_mobile_confirmation":false,"needs_email_confirmation":false}"#),
            Needs::Nothing
        );

        assert_eq!(
            needs(r#"{"needs_mobile_confirmation":true}"#),
            Needs::MobileConfirmation
        );
        assert_eq!(
            needs(r#"{"needs_mobile_confirmation":true,"needs_email_confirmation":true}"#),
            Needs::MobileConfirmation
        );

        assert_eq!(
            needs(r#"{"needs_email_confirmation":true,"email_domain":"example.com"}"#),
            Needs::EmailConfirmation
        );
        assert_eq!(
            needs(r#"{"needs_mobile_confirmation":false,"needs_email_confirmation":true}"#),
            Needs::EmailConfirmation
        );
    }

    #[test]
    fn errors_after_creation_carry_the_offer_id() {
        match offer_created_error(OfferId(7), TradeError::ConfirmationNotFound { offer_id: OfferId(7) }) {
            TradeError::ConfirmationNotFound { offer_id: OfferId(7) } => {}
            other => panic!("{:?}", other),
        }
        match offer_created_error(OfferId(7), TradeError::NoSessionId) {
            TradeError::OfferCreatedButNotConfirmed {
                offer_id: OfferId(7),
                source,
            } => {
                assert!(matches!(*source, TradeError::NoSessionId))
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn each_offer_takes_its_own_confirmation() {
        let (found, confirmed) = confirmations_of(
            &[OfferId(1), OfferId(2), OfferId(3), OfferId(1)],
            vec![10, 3, 1, 1],
            |&confirmation: &u64, offer_id| confirmation == offer_id.0,
        );
        assert_eq!(found, vec![1, 3, 1]);
        assert_eq!(confirmed.confirmed, vec![OfferId(1), OfferId(3), OfferId(1)]);
        assert_eq!(confirmed.not_found, vec![OfferId(2)]);

        let (found, confirmed) = confirmations_of(&[OfferId(1)], vec![], |&c: &u64, id| c == id.0);
        assert!(found.is_empty());
        assert_eq!(
            confirmed,
            Confirmed {
                confirmed: vec![],
                not_found: vec![OfferId(1)],
            }
        );
    }

    fn rate_limited_fetch() -> TradeError {
        steam_mobile::errors::AuthError::from(steam_mobile::errors::InternalError::TooManyRequests {
            url: "https://steamcommunity.com/mobileconf/getlist".to_string(),
        })
        .into()
    }

    fn assert_not_confirmed(result: Result<OfferId, TradeError>, check_source: impl FnOnce(&TradeError) -> bool) {
        match result {
            Err(TradeError::OfferCreatedButNotConfirmed {
                offer_id: OfferId(7),
                source,
            }) => assert!(check_source(&source), "{:?}", source),
            other => panic!("{:?}", other),
        }
    }

    /// What `send` returns for offer 7, created with `needs`, when confirming it gave `confirmed`.
    fn sent(needs: Needs, confirmed: Result<Confirmed, ConfirmFailed>) -> Result<OfferId, TradeError> {
        sent_offer(CreatedOffer { id: OfferId(7), needs }, confirmed)
    }

    fn found() -> Result<Confirmed, ConfirmFailed> {
        Ok(Confirmed {
            confirmed: vec![OfferId(7)],
            not_found: vec![],
        })
    }

    fn none_found() -> Result<Confirmed, ConfirmFailed> {
        Ok(Confirmed {
            confirmed: vec![],
            not_found: vec![OfferId(7)],
        })
    }

    #[test]
    fn sent_offer_needs_and_confirmations() {
        let all = [Needs::Nothing, Needs::MobileConfirmation, Needs::EmailConfirmation];

        // a confirmation found is accepted whatever the needs, and a refused one fails the offer
        for &needs in &all {
            assert_eq!(sent(needs, found()).unwrap(), OfferId(7));
            assert_not_confirmed(
                sent(needs, Err(ConfirmFailed::Process(rate_limited_fetch()))),
                TradeError::is_too_many_requests,
            );
        }

        // needs nothing: sent, found or not, fetched or not
        assert_eq!(sent(Needs::Nothing, none_found()).unwrap(), OfferId(7));
        assert_eq!(
            sent(Needs::Nothing, Err(ConfirmFailed::Fetch(rate_limited_fetch()))).unwrap(),
            OfferId(7)
        );

        // a mobile confirmation is needed: never a success without it
        match sent(Needs::MobileConfirmation, none_found()) {
            Err(TradeError::ConfirmationNotFound { offer_id: OfferId(7) }) => {}
            other => panic!("{:?}", other),
        }
        assert_not_confirmed(
            sent(
                Needs::MobileConfirmation,
                Err(ConfirmFailed::Fetch(rate_limited_fetch())),
            ),
            TradeError::is_too_many_requests,
        );

        // an email one, which this crate can't give
        assert_not_confirmed(sent(Needs::EmailConfirmation, none_found()), |source| {
            matches!(source, TradeError::NeedsEmailConfirmation { offer_id: OfferId(7) })
        });
        assert_not_confirmed(
            sent(
                Needs::EmailConfirmation,
                Err(ConfirmFailed::Fetch(rate_limited_fetch())),
            ),
            TradeError::is_too_many_requests,
        );
    }

    /// A request [`confirm_steps`] sent to the stand-in for Steam.
    #[derive(Debug, PartialEq)]
    enum ConfirmStep {
        Fetch,
        Acknowledge,
        /// The confirmations accepted, by their offer id.
        Process(Vec<u64>),
    }

    /// Runs [`confirm_steps`] for `offer_ids`, with Steam standing in: `fetched` is what the getlist gives (each
    /// confirmation as its offer id), `processed` what the multiajaxop gives. Returns the result and the steps sent.
    fn run_confirm_steps(
        offer_ids: &[u64],
        fetched: Result<Vec<u64>, TradeError>,
        processed: Result<(), TradeError>,
    ) -> (Result<Confirmed, ConfirmFailed>, Vec<ConfirmStep>) {
        let steps = std::cell::RefCell::new(Vec::new());
        let offer_ids: Vec<OfferId> = offer_ids.iter().copied().map(OfferId).collect();
        let result = futures::executor::block_on(confirm_steps(
            &offer_ids,
            || {
                steps.borrow_mut().push(ConfirmStep::Fetch);
                futures::future::ready(fetched)
            },
            |&confirmation: &u64, offer_id| confirmation == offer_id.0,
            || {
                steps.borrow_mut().push(ConfirmStep::Acknowledge);
                futures::future::ready(())
            },
            |found| {
                steps.borrow_mut().push(ConfirmStep::Process(found));
                futures::future::ready(processed)
            },
        ));
        (result, steps.into_inner())
    }

    #[test]
    fn confirm_fetches_then_acknowledges_then_accepts_all_found_at_once() {
        let (result, steps) = run_confirm_steps(&[1, 2, 3], Ok(vec![9, 3, 1]), Ok(()));
        assert_eq!(
            steps,
            vec![
                ConfirmStep::Fetch,
                ConfirmStep::Acknowledge,
                ConfirmStep::Process(vec![1, 3])
            ]
        );
        assert_eq!(
            result.unwrap(),
            Confirmed {
                confirmed: vec![OfferId(1), OfferId(3)],
                not_found: vec![OfferId(2)],
            }
        );
    }

    #[test]
    fn confirm_sends_nothing_more_when_nothing_is_found() {
        for fetched in [vec![], vec![9]] {
            let (result, steps) = run_confirm_steps(&[1], Ok(fetched), Ok(()));
            assert_eq!(steps, vec![ConfirmStep::Fetch]);
            assert_eq!(
                result.unwrap(),
                Confirmed {
                    confirmed: vec![],
                    not_found: vec![OfferId(1)],
                }
            );
        }

        // no offer: no request at all
        let (result, steps) = run_confirm_steps(&[], Ok(vec![1]), Ok(()));
        assert!(steps.is_empty());
        assert_eq!(result.unwrap(), Confirmed::default());
    }

    #[test]
    fn confirm_tells_a_failed_fetch_from_a_refused_confirmation() {
        let (result, steps) = run_confirm_steps(&[7], Err(rate_limited_fetch()), Ok(()));
        assert_eq!(steps, vec![ConfirmStep::Fetch]);
        assert!(matches!(result, Err(ConfirmFailed::Fetch(ref error)) if error.is_too_many_requests()));

        let refused = TradeError::UnexpectedResponse("refused".to_string());
        let (result, steps) = run_confirm_steps(&[7], Ok(vec![7]), Err(refused));
        assert_eq!(
            steps,
            vec![
                ConfirmStep::Fetch,
                ConfirmStep::Acknowledge,
                ConfirmStep::Process(vec![7])
            ]
        );
        assert!(matches!(
            result,
            Err(ConfirmFailed::Process(TradeError::UnexpectedResponse(_)))
        ));
    }

    #[test]
    fn send_of_an_offer_that_needs_nothing_fails_only_on_a_refused_confirmation() {
        let (confirmed, _) = run_confirm_steps(&[7], Err(rate_limited_fetch()), Ok(()));
        assert_eq!(sent(Needs::Nothing, confirmed).unwrap(), OfferId(7));

        let refused = TradeError::UnexpectedResponse("refused".to_string());
        let (confirmed, _) = run_confirm_steps(&[7], Ok(vec![7]), Err(refused));
        assert_not_confirmed(sent(Needs::Nothing, confirmed), |source| {
            matches!(source, TradeError::UnexpectedResponse(_))
        });
    }

    /// Runs the steps of `accept` for offer 7 after Steam answered `posted`, with `confirmed` standing in for the
    /// confirmation; returns the result, and whether it confirmed.
    fn run_accept_steps(
        needs: Needs,
        confirmed: Result<Confirmed, TradeError>,
    ) -> (Result<Option<TradeId>, TradeError>, bool) {
        let confirming = std::cell::Cell::new(false);
        let posted = PostedAccept {
            trade_id: Some(TradeId(9)),
            needs,
        };
        let result = futures::executor::block_on(accept_steps(OfferId(7), posted, || {
            confirming.set(true);
            futures::future::ready(confirmed)
        }));
        (result, confirming.get())
    }

    #[test]
    fn accept_confirms_only_when_steam_asks() {
        let found = || -> Result<Confirmed, TradeError> {
            Ok(Confirmed {
                confirmed: vec![OfferId(7)],
                not_found: vec![],
            })
        };

        let (result, confirmed) = run_accept_steps(Needs::Nothing, found());
        assert_eq!(result.unwrap(), Some(TradeId(9)));
        assert!(!confirmed);

        let (result, confirmed) = run_accept_steps(Needs::EmailConfirmation, found());
        assert!(matches!(
            result,
            Err(TradeError::NeedsEmailConfirmation { offer_id: OfferId(7) })
        ));
        assert!(!confirmed);

        let (result, confirmed) = run_accept_steps(Needs::MobileConfirmation, found());
        assert_eq!(result.unwrap(), None);
        assert!(confirmed);

        let (result, _) = run_accept_steps(Needs::MobileConfirmation, Ok(Confirmed::default()));
        assert!(matches!(
            result,
            Err(TradeError::ConfirmationNotFound { offer_id: OfferId(7) })
        ));

        let (result, _) = run_accept_steps(Needs::MobileConfirmation, Err(rate_limited_fetch()));
        assert!(result.unwrap_err().is_too_many_requests());
    }

    /// The first offer of `GetTradeOffers` for A and for B right after the trade (`ack-cycle.jsonl`), accepted.
    const LIVE_SENT_OFFER: &str = r#"{"tradeofferid":"9438483375","accountid_other":1000002,"message":"steamhelper-rs live test","expiration_time":1793720076,"trade_offer_state":3,"items_to_give":[{"appid":730,"contextid":"2","assetid":"24234602605","classid":"241575744","instanceid":"302028390","amount":"1","missing":true,"est_usd":"4"}],"is_our_offer":true,"time_created":1792510476,"time_updated":1792510484,"tradeid":"695401084301346863","from_real_time_trade":false,"escrow_end_date":0,"confirmation_method":2,"eresult":1,"delay_settlement":true,"settlement_date":1793116800}"#;
    const LIVE_RECEIVED_OFFER: &str = r#"{"tradeofferid":"9438483375","accountid_other":1000001,"message":"steamhelper-rs live test","expiration_time":1793720076,"trade_offer_state":3,"items_to_receive":[{"appid":730,"contextid":"2","assetid":"24234602605","classid":"241575744","instanceid":"302028390","amount":"1","missing":true,"est_usd":"4"}],"is_our_offer":false,"time_created":1792510476,"time_updated":1792510484,"tradeid":"695401084301346863","from_real_time_trade":false,"escrow_end_date":0,"confirmation_method":0,"eresult":1,"delay_settlement":true,"settlement_date":1793116800}"#;
    /// The second, declined.
    const LIVE_DECLINED_OFFER: &str = r#"{"tradeofferid":"9438483046","accountid_other":1000002,"message":"steamhelper-rs live test","expiration_time":1793720064,"trade_offer_state":7,"items_to_give":[{"appid":730,"contextid":"2","assetid":"24234602605","classid":"241575744","instanceid":"302028390","amount":"1","missing":false,"est_usd":"4"}],"is_our_offer":true,"time_created":1792510464,"time_updated":1792510471,"from_real_time_trade":false,"escrow_end_date":0,"confirmation_method":2,"eresult":1,"delay_settlement":true,"settlement_date":0}"#;

    fn live_item(missing: bool) -> OfferedItem {
        OfferedItem {
            asset: cs2(24234602605),
            classid: 241575744,
            instanceid: 302028390,
            amount: 1,
            missing,
        }
    }

    #[test]
    fn live_offers() {
        let response: GetTradeOffersResponse = serde_json::from_str(&format!(
            r#"{{"response":{{"trade_offers_sent":[{},{}],"trade_offers_received":[{}]}}}}"#,
            LIVE_SENT_OFFER, LIVE_DECLINED_OFFER, LIVE_RECEIVED_OFFER
        ))
        .unwrap();
        let offers = offers_of(response).unwrap();

        let sent = Offer {
            id: OfferId(9438483375),
            partner: SteamID::from_steam64(76561197961265730),
            is_ours: true,
            state: OfferState::Accepted,
            give: vec![live_item(true)],
            take: vec![],
            has_malformed_items: false,
            message: "steamhelper-rs live test".to_string(),
            created: 1792510476,
            updated: 1792510484,
            expires: Some(1793720076),
            escrow_end: None,
            trade_id: Some(TradeId(695401084301346863)),
            protected_until: Some(1793116800),
        };
        assert_eq!(offers[0], sent);
        // the same time as its trade's
        let trade = trade_in(live_trade_history_response(), 695401084301346863).unwrap();
        assert_eq!(sent.protected_until, trade.protected_until);
        assert_eq!(
            offers[1],
            Offer {
                id: OfferId(9438483046),
                state: OfferState::Declined,
                give: vec![live_item(false)],
                created: 1792510464,
                updated: 1792510471,
                expires: Some(1793720064),
                trade_id: None,
                protected_until: None,
                ..sent.clone()
            }
        );
        assert_eq!(
            offers[2],
            Offer {
                partner: SteamID::from_steam64(76561197961265729),
                is_ours: false,
                give: vec![],
                take: vec![live_item(true)],
                ..sent
            }
        );
    }

    #[test]
    fn offer_with_an_unreadable_item() {
        let offer: TradeOffer_Trade = serde_json::from_str(
            r#"{"tradeofferid":"4127395150","accountid_other":79925588,"trade_offer_state":3,
              "items_to_give":[{"appid":730,"contextid":"2","assetid":"1","classid":"2","amount":"1"}],
              "items_to_receive":[
                {"appid":730,"contextid":"2","assetid":"24234602605","classid":"241575744","amount":"1"},
                {"appid":730,"contextid":"2","assetid":"not a number","classid":"241575744","amount":"1"},
                {"appid":730,"contextid":"2","assetid":"24234602606","classid":"241575744"}]}"#,
        )
        .unwrap();
        let offer = Offer::of(offer).unwrap();
        assert!(offer.has_malformed_items);
        assert_eq!(offer.give.len(), 1);
        let take: Vec<(Asset, u64)> = offer.take.iter().map(|item| (item.asset, item.classid)).collect();
        assert_eq!(take, vec![(cs2(24234602605), 241575744)]);
    }

    #[test]
    fn offer_that_cannot_be_ours_fails_the_list() {
        let response: GetTradeOffersResponse = serde_json::from_str(
            r#"{"response":{"trade_offers_received":[
                {"tradeofferid":"1","accountid_other":2,"trade_offer_state":2},
                {"tradeofferid":"3","accountid_other":4294967296,"trade_offer_state":2},
                {"tradeofferid":"5","accountid_other":2,"trade_offer_state":2,"tradeid":"x"}]}}"#,
        )
        .unwrap();
        match offers_of(response) {
            Err(TradeError::MalformedOffers { offer_ids, reason }) => {
                assert_eq!(offer_ids, vec![Some(OfferId(3)), Some(OfferId(5))]);
                assert!(reason.contains("accountid_other"), "{}", reason);
            }
            other => panic!("{:?}", other),
        }
    }

    fn one_offer(body: &str) -> Result<Option<Offer>, TradeError> {
        offer_of(OfferId(9438483375), Ok(serde_json::from_str(body).unwrap()))
    }

    #[test]
    fn offer_by_id() {
        let found = one_offer(&format!(r#"{{"response":{{"offer":{}}}}}"#, LIVE_SENT_OFFER)).unwrap();
        assert_eq!(found.map(|offer| offer.id), Some(OfferId(9438483375)));

        // no such offer: no `offer`, or NoMatch
        assert_eq!(one_offer(r#"{"response":{}}"#).unwrap(), None);
        let no_match = SteamAPIError::EResult(EResult::NoMatch, "Value: 42".to_string());
        assert_eq!(offer_of(OfferId(1), Err(no_match)).unwrap(), None);

        // any other error stays one, with Steam's code when it gave one
        let denied = SteamAPIError::EResult(EResult::AccessDenied, "Value: 15".to_string());
        assert!(matches!(
            offer_of(OfferId(1), Err(denied)),
            Err(TradeError::Steam {
                eresult: EResult::AccessDenied,
                message: None
            })
        ));
        let rate_limited = SteamAPIError::SteamHttpError("429".to_string());
        assert!(offer_of(OfferId(1), Err(rate_limited))
            .unwrap_err()
            .is_too_many_requests());

        // an offer that can't be read
        for body in &[
            r#"{"response":{"offer":{"tradeofferid":"9438483375","trade_offer_state":2}}}"#,
            r#"{"response":{"offer":{"tradeofferid":"9438483375","accountid_other":4294967296,"trade_offer_state":2}}}"#,
        ] {
            match one_offer(body) {
                Err(TradeError::MalformedOffer {
                    offer_id: OfferId(9438483375),
                    reason,
                }) => assert!(reason.contains("accountid_other"), "{}", reason),
                other => panic!("{}: {:?}", body, other),
            }
        }
    }

    #[test]
    fn trade_hold_of_the_answer() {
        let hold = |body: &str| hold_of(serde_json::from_str(body).unwrap());
        assert_eq!(
            hold(r#"{"response":{"my_escrow":{"escrow_end_duration_seconds":0},"their_escrow":{"escrow_end_duration_seconds":1296000},"both_escrow":{"escrow_end_duration_seconds":1296000}}}"#).unwrap(),
            Duration::from_secs(1296000)
        );
        assert_eq!(
            hold(r#"{"response":{"both_escrow":{"escrow_end_duration_seconds":0}}}"#).unwrap(),
            Duration::from_secs(0)
        );
        assert!(matches!(hold(r#"{"response":{}}"#), Err(TradeError::TradeHoldUnknown)));
        assert!(matches!(
            hold(r#"{"response":{"both_escrow":{"escrow_end_duration_seconds":-1}}}"#),
            Err(TradeError::UnexpectedResponse(_))
        ));
    }

    #[cfg(feature = "cs2")]
    #[test]
    fn only_an_accepted_offer_has_received_items_to_find() {
        let offer = |state: u32| -> Offer {
            let offer: TradeOffer_Trade = serde_json::from_str(&format!(
                r#"{{"tradeofferid":"4127395150","accountid_other":79925588,"trade_offer_state":{},
                  "items_to_receive":[
                    {{"appid":730,"contextid":"2","assetid":"24234602605","classid":"241575744","amount":"1"}}]}}"#,
                state
            ))
            .unwrap();
            Offer::of(offer).unwrap()
        };
        assert_eq!(accepted_received_items(&offer(3)).unwrap().len(), 1);
        for (state, expected) in [(2, OfferState::Active), (7, OfferState::Declined)] {
            match accepted_received_items(&offer(state)) {
                Err(TradeError::OfferNotAccepted {
                    offer_id: OfferId(4127395150),
                    state,
                }) => assert_eq!(state, expected),
                other => panic!("{:?}", other),
            }
        }
    }

    fn offers_json(sent: &[u64], received: &[u64], next_cursor: Option<u32>) -> String {
        let offers = |ids: &[u64]| {
            ids.iter()
                .map(|id| {
                    format!(
                        r#"{{"tradeofferid":"{}","accountid_other":1,"trade_offer_state":2}}"#,
                        id
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let cursor = next_cursor.map_or(String::new(), |cursor| format!(r#","next_cursor":{}"#, cursor));
        format!(
            r#"{{"response":{{"trade_offers_sent":[{}],"trade_offers_received":[{}]{}}}}}"#,
            offers(sent),
            offers(received),
            cursor
        )
    }

    fn offers_page(sent: &[u64], received: &[u64], next_cursor: Option<u32>) -> GetTradeOffersResponse {
        serde_json::from_str(&offers_json(sent, received, next_cursor)).unwrap()
    }

    fn ids(offers: &Option<Vec<TradeOffer_Trade>>) -> Vec<u64> {
        offers.iter().flatten().map(|offer| offer.tradeofferid).collect()
    }

    /// Runs the paging loop over `pages`, by cursor; returns its result and the cursors requested.
    fn read_pages(
        pages: Vec<(Option<u32>, GetTradeOffersResponse)>,
    ) -> (Result<GetTradeOffersResponse, TradeError>, Vec<Option<u32>>) {
        let mut requested = vec![];
        let merged = futures::executor::block_on(fetch_all_trade_offer_pages(|cursor| {
            requested.push(cursor);
            let page = pages
                .iter()
                .find(|(page_cursor, _)| *page_cursor == cursor)
                .map(|(_, page)| page.clone())
                .ok_or_else(|| GeneralError(format!("no page at {:?}", cursor)));
            async move { page }
        }));
        (merged, requested)
    }

    #[test]
    fn single_page() {
        for next_cursor in &[None, Some(0)] {
            let (merged, requested) = read_pages(vec![(None, offers_page(&[1], &[2], *next_cursor))]);
            let merged = merged.unwrap();
            assert_eq!(requested, vec![None]);
            assert_eq!(ids(&merged.response.trade_offers_sent), vec![1]);
            assert_eq!(ids(&merged.response.trade_offers_received), vec![2]);
            assert_eq!(merged.response.next_page_cursor(), None);
        }
    }

    #[test]
    fn pages_are_merged() {
        let (merged, requested) = read_pages(vec![
            (None, offers_page(&[1], &[10, 11], Some(100))),
            (Some(100), offers_page(&[], &[11, 12], Some(200))),
            (Some(200), offers_page(&[2], &[13], Some(0))),
        ]);
        let merged = merged.unwrap();
        assert_eq!(requested, vec![None, Some(100), Some(200)]);
        assert_eq!(ids(&merged.response.trade_offers_sent), vec![1, 2]);
        assert_eq!(ids(&merged.response.trade_offers_received), vec![10, 11, 12, 13]);
        assert_eq!(merged.response.next_page_cursor(), None);
    }

    #[test]
    fn malformed_offers_of_every_page_are_kept() {
        let page = |json: &str| serde_json::from_str::<GetTradeOffersResponse>(json).unwrap();
        let (merged, _) = read_pages(vec![
            (
                None,
                page(
                    r#"{"response":{"trade_offers_received":[{"tradeofferid":"1","trade_offer_state":2}],"next_cursor":100}}"#,
                ),
            ),
            (
                Some(100),
                page(
                    r#"{"response":{"trade_offers_sent":[{"tradeofferid":"2","accountid_other":3}],
                        "trade_offers_received":[{"tradeofferid":"4","accountid_other":3,"trade_offer_state":2}]}}"#,
                ),
            ),
        ]);
        let merged = merged.unwrap().response;

        assert_eq!(ids(&Some(merged.trade_offers_received.clone().unwrap())), vec![4]);
        let malformed_ids =
            |offers: &[MalformedOffer]| offers.iter().map(|offer| offer.tradeofferid()).collect::<Vec<_>>();
        assert_eq!(malformed_ids(&merged.malformed_offers_received), vec![Some(1)]);
        assert_eq!(malformed_ids(&merged.malformed_offers_sent), vec![Some(2)]);
    }

    fn assert_incomplete(result: Result<GetTradeOffersResponse, TradeError>, expected_cursor: u32) {
        match result {
            Err(TradeError::TradeOffersIncomplete { cursor, .. }) => assert_eq!(cursor, expected_cursor),
            other => panic!("{:?}", other.map(|merged| ids(&merged.response.trade_offers_received))),
        }
    }

    #[test]
    fn repeated_cursor_is_an_error() {
        let (merged, requested) = read_pages(vec![
            (None, offers_page(&[], &[1], Some(100))),
            (Some(100), offers_page(&[], &[2], Some(100))),
        ]);
        assert_eq!(requested, vec![None, Some(100)]);
        assert_incomplete(merged, 100);
    }

    #[test]
    fn cycle_is_an_error() {
        let (merged, requested) = read_pages(vec![
            (None, offers_page(&[], &[1], Some(100))),
            (Some(100), offers_page(&[], &[2], Some(200))),
            (Some(200), offers_page(&[], &[3], Some(100))),
        ]);
        assert_eq!(requested, vec![None, Some(100), Some(200)]);
        assert_incomplete(merged, 100);
    }

    #[test]
    fn page_cap_is_an_error() {
        let mut pages = vec![(None, offers_page(&[], &[0], Some(1)))];
        for cursor in 1..=(MAX_TRADE_OFFER_PAGES as u32 + 10) {
            pages.push((Some(cursor), offers_page(&[], &[cursor as u64], Some(cursor + 1))));
        }
        let (merged, requested) = read_pages(pages);
        assert_eq!(requested.len(), MAX_TRADE_OFFER_PAGES);
        assert_incomplete(merged, MAX_TRADE_OFFER_PAGES as u32);

        // exactly the cap is fine
        let mut pages = vec![(None, offers_page(&[], &[0], Some(1)))];
        for cursor in 1..(MAX_TRADE_OFFER_PAGES as u32) {
            let next = if cursor + 1 < MAX_TRADE_OFFER_PAGES as u32 {
                cursor + 1
            } else {
                0
            };
            pages.push((Some(cursor), offers_page(&[], &[cursor as u64], Some(next))));
        }
        let (merged, requested) = read_pages(pages);
        assert_eq!(requested.len(), MAX_TRADE_OFFER_PAGES);
        assert_eq!(
            ids(&merged.unwrap().response.trade_offers_received).len(),
            MAX_TRADE_OFFER_PAGES
        );
    }

    #[test]
    fn failed_later_page_is_an_error() {
        let (merged, requested) = read_pages(vec![(None, offers_page(&[], &[1], Some(100)))]);
        assert_eq!(requested, vec![None, Some(100)]);
        // the page's own error, so that e.g. `is_too_many_requests` still works
        match merged {
            Err(GeneralError(message)) => assert_eq!(message, "no page at Some(100)"),
            other => panic!("{:?}", other.map(|_| ())),
        }

        let rate_limited = futures::executor::block_on(fetch_all_trade_offer_pages(|cursor| async move {
            match cursor {
                None => Ok(offers_page(&[], &[1], Some(100))),
                Some(_) => Err(web_api_error(SteamAPIError::SteamHttpError("429".to_string()))),
            }
        }));
        assert!(rate_limited.unwrap_err().is_too_many_requests());
    }

    #[test]
    fn failed_first_page_is_an_error() {
        let result = futures::executor::block_on(fetch_all_trade_offer_pages(|_| async {
            Err(GeneralError("down".to_string()))
        }));
        assert!(result.is_err());
    }

    #[test]
    fn unreadable_offers_fail_the_list() {
        let readable = offers_page(&[1], &[2], None);
        assert!(fail_on_malformed_offers(&readable).is_ok());

        let offers = serde_json::from_str::<GetTradeOffersResponse>(
            r#"{"response":{
                "trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2},
                                     {"tradeofferid":"3","trade_offer_state":2}],
                "trade_offers_received":[{"accountid_other":2}]}}"#,
        )
        .unwrap();
        match fail_on_malformed_offers(&offers) {
            Err(error @ TradeError::MalformedOffers { .. }) => {
                let message = error.to_string();
                assert!(message.starts_with("2 trade offer(s) could not be read"), "{}", message);
                assert!(message.contains("accountid_other"), "{}", message);
                match error {
                    TradeError::MalformedOffers { offer_ids, .. } => {
                        assert_eq!(offer_ids, vec![Some(OfferId(3)), None])
                    }
                    _ => unreachable!(),
                }
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn unreadable_offers_are_counted_once_across_pages() {
        let page = |json: &str| serde_json::from_str::<GetTradeOffersResponse>(json).unwrap();
        let (merged, _) = read_pages(vec![
            (
                None,
                page(
                    r#"{"response":{"trade_offers_received":[{"tradeofferid":"1"},{"accountid_other":9}],
                        "next_cursor":100}}"#,
                ),
            ),
            (
                Some(100),
                page(
                    r#"{"response":{"trade_offers_received":[{"tradeofferid":"1","trade_offer_state":2},
                        {"accountid_other":9},{"accountid_other":8}]}}"#,
                ),
            ),
        ]);
        let merged = merged.unwrap();
        match fail_on_malformed_offers(&merged) {
            Err(TradeError::MalformedOffers { offer_ids, .. }) => {
                assert_eq!(offer_ids, vec![Some(OfferId(1)), None, None])
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn readable_copy_of_an_offer_still_fails_the_list() {
        let page = |json: &str| serde_json::from_str::<GetTradeOffersResponse>(json).unwrap();
        for pages in [
            vec![
                (
                    None,
                    page(r#"{"response":{"trade_offers_received":[{"tradeofferid":"1"}],"next_cursor":100}}"#),
                ),
                (Some(100), offers_page(&[], &[1], None)),
            ],
            vec![
                (None, offers_page(&[], &[1], Some(100))),
                (
                    Some(100),
                    page(r#"{"response":{"trade_offers_received":[{"tradeofferid":"1"}]}}"#),
                ),
            ],
        ] {
            let (merged, _) = read_pages(pages);
            let merged = merged.unwrap();
            assert_eq!(ids(&merged.response.trade_offers_received), vec![1]);
            // the list as a whole fails, as Steam did send an unreadable offer
            assert!(matches!(
                fail_on_malformed_offers(&merged),
                Err(TradeError::MalformedOffers { .. })
            ));
        }
    }

    #[test]
    fn unreadable_offers_are_counted_once_within_a_page() {
        let page = |json: &str| serde_json::from_str::<GetTradeOffersResponse>(json).unwrap();
        let first_page = r#"{"response":{
            "trade_offers_sent":[{"tradeofferid":"3"},{"tradeofferid":"3"}],
            "trade_offers_received":[{"tradeofferid":"1"},{"accountid_other":9},{"tradeofferid":"1"},
                                     {"accountid_other":9},{"accountid_other":8}]}}"#;

        // as the only page, and as the first of two
        let (single, _) = read_pages(vec![(None, page(first_page))]);
        let (paged, requested) = read_pages(vec![
            (None, page(&first_page.replace("}]}}", r#"}],"next_cursor":100}}"#))),
            (
                Some(100),
                page(r#"{"response":{"trade_offers_received":[{"tradeofferid":"1"},{"accountid_other":9}]}}"#),
            ),
        ]);
        assert_eq!(requested, vec![None, Some(100)]);
        for merged in [single.unwrap(), paged.unwrap()] {
            match fail_on_malformed_offers(&merged) {
                Err(TradeError::MalformedOffers { offer_ids, .. }) => {
                    assert_eq!(offer_ids, vec![Some(OfferId(3)), Some(OfferId(1)), None, None])
                }
                other => panic!("{:?}", other),
            }
        }
    }

    // Expected values: the URL tappet's `request_tests` captured from the code before pages were read, with the
    // approved changes (https instead of http, the access token instead of the API key: spec change 1). `prepare`
    // builds the request `execute` sends, without sending it.
    const FIRST_PAGE_URL: &str = "https://api.steampowered.com/IEconService/GetTradeOffers/v1?access_token=TOKEN&&get_sent_offers=true&get_received_offers=true&time_historical_cutoff=4294967295&active_only=true";

    fn prepared_url(request: reqwest::Request) -> String {
        assert_eq!(request.method(), Method::GET);
        assert!(request.headers().is_empty(), "{:?}", request.headers());
        assert!(request.body().is_none());
        request.url().to_string()
    }

    #[test]
    fn first_page_request_is_unchanged() {
        let http_client = reqwest::Client::new();
        let mut sent = vec![];

        let merged = futures::executor::block_on(fetch_all_trade_offer_pages(|cursor| {
            let api_client = web_api_with_token(&http_client, "TOKEN".to_string());
            sent.push(prepared_url(
                trade_offers_request(&api_client, OfferQuery::ACTIVE, cursor)
                    .prepare()
                    .unwrap(),
            ));
            let page = match cursor {
                None => offers_page(&[1], &[2], Some(100)),
                Some(_) => offers_page(&[], &[3], Some(0)),
            };
            async move { Ok(page) }
        }))
        .unwrap();

        assert_eq!(ids(&merged.response.trade_offers_received), vec![2, 3]);
        assert_eq!(
            sent,
            vec![FIRST_PAGE_URL.to_string(), FIRST_PAGE_URL.to_string() + "&cursor=100"]
        );
    }

    #[test]
    fn access_token_is_read_for_every_call() {
        let http_client = reqwest::Client::new();

        let sent: Vec<String> = ["first+token", "renewed"]
            .iter()
            .map(|token| {
                let api_client = web_api_with_token(&http_client, token.to_string());
                prepared_url(
                    trade_offers_request(&api_client, OfferQuery::ACTIVE_RECEIVED, None)
                        .prepare()
                        .unwrap(),
                )
            })
            .collect();

        let query =
            "&&get_sent_offers=false&get_received_offers=true&time_historical_cutoff=4294967295&active_only=true";
        assert_eq!(
            sent,
            vec![
                format!(
                    "https://api.steampowered.com/IEconService/GetTradeOffers/v1?access_token=first%2Btoken{}",
                    query
                ),
                format!(
                    "https://api.steampowered.com/IEconService/GetTradeOffers/v1?access_token=renewed{}",
                    query
                ),
            ]
        );
    }

    #[test]
    fn offer_request_is_its_id_only() {
        let api_client = web_api_with_token(&reqwest::Client::new(), "TOKEN".to_string());
        assert_eq!(
            prepared_url(trade_offer_request(&api_client, OfferId(9438483375)).prepare().unwrap()),
            "https://api.steampowered.com/IEconService/GetTradeOffer/v1?access_token=TOKEN&&tradeofferid=9438483375"
        );
    }

    #[test]
    fn trade_hold_request() {
        let api_client = web_api_with_token(&reqwest::Client::new(), "TOKEN".to_string());
        let link = TradeLink::parse(get_tradeoffer_url_with_token()).unwrap();
        assert_eq!(
            prepared_url(super::trade_hold_request(&api_client, &link).prepare().unwrap()),
            "https://api.steampowered.com/IEconService/GetTradeHoldDurations/v1?access_token=TOKEN&&steamid_target=76561198040191316&trade_offer_access_token=Ob27qXzn"
        );
    }

    /// B of the live tests stands in: account id 1, SteamID 76561197960265729.
    fn account() -> SteamID {
        SteamID::from_account_id(1)
    }

    #[test]
    fn trade_link_from_token() {
        let link = trade_link_of(account(), "Ob27qXzn").unwrap();
        assert_eq!(
            link.url(),
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn"
        );
        assert_eq!(link.partner(), account());
        assert_eq!(link.token(), "Ob27qXzn");
        // a token no trade link has
        assert!(matches!(
            trade_link_of(account(), "a&token=b"),
            Err(TradeError::TradeLinkError(_))
        ));
    }

    /// Hand-made, after the input node-steamcommunity and aiosteampy read (no fixture: HTML pages were left out).
    const PRIVACY_PAGE: &str = r#"<html><body>
        <div class="trade_offer_access_url_ctn">
          <input type="text" id="trade_offer_access_url" class="trade_offer_access_url" readonly
            value="https://steamcommunity.com/tradeoffer/new/?partner=1&amp;token=Ob27qXzn">
        </div>
        <a href="https://steamcommunity.com/tradeoffer/new/?partner=79925588&amp;token=other">a friend's</a>
        </body></html>"#;

    #[test]
    fn trade_link_on_the_privacy_page() {
        let link = trade_link_on_privacy_page(PRIVACY_PAGE, account()).unwrap();
        assert_eq!(
            link.url(),
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn"
        );

        // as text, unescaped
        let text = "Trade URL: https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn";
        assert_eq!(trade_link_on_privacy_page(text, account()).unwrap().token(), "Ob27qXzn");
    }

    #[test]
    fn privacy_page_without_our_link() {
        let others = PRIVACY_PAGE.replace("partner=1&", "partner=2&");
        for page in &[others.as_str(), "<html>Sign In</html>", ""] {
            assert!(
                matches!(
                    trade_link_on_privacy_page(page, account()),
                    Err(TradeError::UnexpectedResponse(_))
                ),
                "{}",
                page
            );
        }
    }

    /// A loopback server standing in for Steam Community. `/profiles/1/tradeoffers/privacy` redirects as the privacy
    /// page did live on 2026-10-22 (`fixtures/live/2026-10-22/live-check.jsonl` lines 12-14): to
    /// `market/eligibilitycheck`, which redirects back, then to `login/home`. `/tradeoffer/new/send` redirects to
    /// `login/home`, `/moved` to `/page`, `/profiles/2/tradeoffers/privacy` to the same page under a custom URL that
    /// starts with "login" (`/profiles/…` redirects there for an account with a custom URL, as aiosteampy notes,
    /// `aiosteampy/client/state.py:286`); any other path is a page.
    fn fake_community() -> u16 {
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Read;
        use std::io::Write;

        const LOGIN: &str = "/login/home/?goto=%2Fprofiles%2F1%2Ftradeoffers%2Fprivacy";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let mut eligibility_checked = false;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split(' ').nth(1).unwrap().to_string();
                let mut content_length = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    let Some((name, value)) = line.trim_end().split_once(':') else {
                        break;
                    };
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.trim().parse().unwrap();
                    }
                }
                reader.read_exact(&mut vec![0; content_length]).unwrap();
                let location = match path.as_str() {
                    "/profiles/1/tradeoffers/privacy" if eligibility_checked => Some(LOGIN),
                    "/profiles/1/tradeoffers/privacy" => {
                        Some("/market/eligibilitycheck/?goto=%2Fprofiles%2F1%2Ftradeoffers%2Fprivacy")
                    }
                    "/market/eligibilitycheck/?goto=%2Fprofiles%2F1%2Ftradeoffers%2Fprivacy" => {
                        eligibility_checked = true;
                        Some("/profiles/1/tradeoffers/privacy")
                    }
                    "/tradeoffer/new/send" => Some(LOGIN),
                    "/moved" => Some("/page"),
                    "/profiles/2/tradeoffers/privacy" => Some("/id/loginbot/tradeoffers/privacy"),
                    _ => None,
                };
                let page = "<html><title>Sign In</title></html>";
                let answer = match location {
                    Some(location) => format!("HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\n", location),
                    None => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n", page.len()),
                };
                let body = if location.is_some() { "" } else { page };
                write!(stream, "{}Connection: close\r\n\r\n{}", answer, body).unwrap();
            }
        });
        port
    }

    fn is_session_expired(answer: Result<(StatusCode, Url, String), TradeError>) -> bool {
        matches!(
            answer,
            Err(TradeError::InternalError(InternalError::MobileInternalError(
                steam_mobile::errors::InternalError::SessionExpired
            )))
        )
    }

    #[tokio::test]
    async fn a_redirect_to_the_login_page_is_session_expired() {
        let port = fake_community();
        let base = format!("http://{}:{}", STEAM_COMMUNITY_HOST, port);
        // follows redirects, as steam-mobile's client
        let client = reqwest::Client::builder()
            .no_proxy()
            .resolve(STEAM_COMMUNITY_HOST, std::net::SocketAddr::from(([127, 0, 0, 1], port)))
            .build()
            .unwrap();

        let page = client
            .get(format!("{}/profiles/1/tradeoffers/privacy", base))
            .send()
            .await
            .unwrap();
        assert!(is_session_expired(community_answer(page).await));

        // a POST too: reqwest follows a 302 with a GET
        let posted = client
            .post(format!("{}/tradeoffer/new/send", base))
            .body("sessionid=1")
            .send()
            .await
            .unwrap();
        assert!(is_session_expired(community_answer(posted).await));

        // a redirect elsewhere is read as any answer
        let moved = client.get(format!("{}/moved", base)).send().await.unwrap();
        let (status, url, body) = community_answer(moved).await.unwrap();
        assert_eq!((status, url.path()), (StatusCode::OK, "/page"));
        assert!(body.starts_with("<html>"));

        // a custom URL that starts with "login" is no login page
        let vanity = client
            .get(format!("{}/profiles/2/tradeoffers/privacy", base))
            .send()
            .await
            .unwrap();
        let (status, url, _) = community_answer(vanity).await.unwrap();
        assert_eq!(
            (status, url.path()),
            (StatusCode::OK, "/id/loginbot/tradeoffers/privacy")
        );
    }
}

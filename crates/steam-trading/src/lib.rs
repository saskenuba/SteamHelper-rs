//! Steam trade manager is the module that allows you to automate trade offers, by extending `SteamAuthenticator`.
//!
//! It inherently needs `SteamAuthenticator` as a dependency, since we need cookies from Steam Community and Steam Store
//! to be able to create and accept trade offers, along with mobile confirmations.
//!
//! **IT IS VERY IMPORTANT THAT STEAM GUARD IS ENABLED ON THE ACCOUNT BEING USED, WITH MOBILE CONFIRMATIONS.**
//!
//! A [`SteamTradeManager`] owns the account's `SteamAuthenticator`: make one per account, and share it. The manager
//! reacts to no events, in comparison of alternatives such as Node.js: you keep track of trades and poll yourself, but
//! it won't be much work, since there are convenience functions for almost every need.
//!
//! An authenticator logged in with a password needs nothing running beside it. One logged in through a Steam client
//! (steam-mobile's `login_with_client`, which feature `cs2` turns on) keeps that client running, and needs it
//! connected to renew its access token, about daily, an hour before it expires (steam-mobile's `renew_before` sets
//! another margin). The first request once the renewal is due waits for it, up to the client's call deadline of 30
//! seconds; requests coming meanwhile go at once with the current token. While the client can't renew it, requests
//! go on with the current token until it expires. After, none is sent: requests wait for another attempt (up to the
//! same 30 seconds), and fail if it fails, or fail at once while the last one failed less than 30 seconds before.
//!
//! Perhaps the event based trading experience will be an extension someday, but for now this works fine.
//!
//! Compiles on stable Rust.

#![allow(dead_code)]
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

use std::collections::HashSet;
use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use const_format::concatcp;
pub use errors::ConfirmationError;
pub use errors::InternalError;
pub use errors::OfferError;
pub use errors::TradeError;
pub use errors::TradelinkError;
use futures::lock::Mutex;
use futures::TryFutureExt;
use futures_timer::Delay;
pub use inventory::AssetProperty;
pub use inventory::Inventory;
pub use inventory::InventoryItem;
pub use inventory::InventoryPage;
pub use inventory::PropertyValue;
#[cfg(feature = "cs2")]
pub use received::AcceptedOffer;
#[cfg(feature = "cs2")]
pub use received::Located;
#[cfg(feature = "cs2")]
pub use received::ReceivedItem;
use serde::de::DeserializeOwned;
#[cfg(feature = "cs2")]
use steam_client::gc::cs2::Cs2;
use steam_language::enums::ETradeOfferState;
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
use tappet::endpoints::ieconservice::get::GetTradeOffers;
use tappet::response_types::GetTradeHistoryResponse;
use tappet::response_types::GetTradeOffersResponse;
use tappet::response_types::MalformedOffer;
#[cfg(feature = "cs2")]
use tappet::response_types::OfferItem;
use tappet::response_types::TradeHistory_Trade;
use tappet::response_types::TradeOffer_Trade;
use tappet::Credential;
use tappet::ExecutorResponse;
use tappet::SteamAPI;
use tracing::debug;
use tracing::warn;
pub use types::asset_collection::AssetCollection;
pub use types::context::InventoryContext;
pub use types::trade_link::Tradelink;
pub use types::trade_offer::TradeOffer;

#[cfg(feature = "additional-checks")]
use crate::additional_checks::check_steam_guard_error;
use crate::api_extensions::FilterBy;
use crate::api_extensions::HasAssets;
use crate::errors::tradeoffer_error_from_eresult;
use crate::errors::TradeError::GeneralError;
use crate::inventory::inventory_url;
use crate::inventory::read_all_inventory_pages;
use crate::response::check_response;
use crate::response::status_check;
use crate::types::sessionid::HasSessionID;
use crate::types::trade_offer_web::TradeOfferAcceptRequest;
use crate::types::trade_offer_web::TradeOfferCancelResponse;
use crate::types::trade_offer_web::TradeOfferCommonParameters;
use crate::types::trade_offer_web::TradeOfferCreateRequest;
use crate::types::trade_offer_web::TradeOfferCreateResponse;
use crate::types::trade_offer_web::TradeOfferGenericErrorResponse;
use crate::types::trade_offer_web::TradeOfferGenericRequest;
use crate::types::trade_offer_web::TradeOfferParams;
use crate::types::trade_offer_web::TradeProtectionAcknowledgeRequest;
use crate::types::TradeKind;

#[cfg(feature = "additional-checks")]
mod additional_checks;
pub mod api_extensions;
mod errors;
mod inventory;
#[cfg(feature = "cs2")]
mod received;
mod response;
mod types;

const TRADEOFFER_BASE: &str = "https://steamcommunity.com/tradeoffer/";
const TRADEOFFER_NEW_URL: &str = concatcp!(TRADEOFFER_BASE, "new/send");
const TRADE_PROTECTION_ACKNOWLEDGE_URL: &str = "https://steamcommunity.com/trade/new/acknowledge";
const TRADE_OFFER_ACCESS_TOKEN_URL: &str = "https://api.steampowered.com/IEconService/GetTradeOfferAccessToken/v1/";

/// This is decided upon various factors, mainly stability of Steam servers when dealing with huge
/// trade offers.
///
/// Consider this when creating trade websites.
pub const TRADE_MAX_ITEMS: u8 = u8::MAX;

/// Max trade offers to a single account.
pub const TRADE_MAX_TRADES_PER_SINGLE_USER: u8 = 5;

/// Max total sent trade offers.
pub const TRADE_MAX_ONGOING_TRADES: u8 = 30;

/// Standard delay, in milliseconds
const STANDARD_DELAY: u64 = 1000;

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
    /// How long `accept_offer_and_find_items` waits for the received items.
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

impl SteamTradeManager {
    /// Returns a new `[SteamTradeManager]` for the account `authenticator` is logged in to, which it owns: get it back
    /// with [`SteamTradeManager::authenticator`], [`SteamTradeManager::authenticator_mut`] or
    /// [`SteamTradeManager::into_authenticator`].
    ///
    /// Its Web API calls (`GetTradeOffers`, `GetTradeHistory`) send the session's access token (`access_token=`), read
    /// from `authenticator` for every call, never kept: an authenticator logged in through a Steam client
    /// (steam-mobile's `login_with_client`) renews it about daily, when a call comes within its renewal margin (an
    /// hour by default, steam-mobile's `renew_before`).
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

    /// Checks whether the user of `tradelink` has recently activated his mobile SteamGuard.
    #[cfg(feature = "additional-checks")]
    pub async fn check_steam_guard_recently_activated(&self, tradelink: Tradelink) -> Result<(), TradeError> {
        let Tradelink { partner_id, token, .. } = tradelink;

        check_steam_guard_error(&self.authenticator, partner_id, &token)
            .err_into()
            .await
    }

    /// Call to GetTradeOffers endpoint.
    ///
    /// Convenience function that fetches information about active trades for the current logged in account.
    ///
    /// Steam pages this endpoint: every page is read, and the offers of all pages are merged. The call fails rather
    /// than return an incomplete list, like node-steam-tradeoffer-manager: with a page's error,
    /// [`TradeError::TradeOffersIncomplete`] when Steam repeats a cursor or sends more than 100 pages, and
    /// [`TradeError::MalformedOffers`] when Steam sent an offer that could not be read (`lib/index.js:606-609` fails
    /// on an offer without `accountid_other`). The response's `malformed_offers_*` lists are then always empty.
    ///
    /// Every call fails while Steam keeps sending an offer that can't be read: see [`TradeError::MalformedOffers`]
    /// for how to get past it.
    pub async fn get_trade_offers(
        &self,
        sent: bool,
        received: bool,
        active_only: bool,
    ) -> Result<GetTradeOffersResponse, TradeError> {
        let offers = self.fetch_trade_offers(sent, received, active_only).await?;
        fail_on_malformed_offers(&offers)?;
        Ok(offers)
    }

    /// Every page of `GetTradeOffers`, offers that could not be read included.
    async fn fetch_trade_offers(
        &self,
        sent: bool,
        received: bool,
        active_only: bool,
    ) -> Result<GetTradeOffersResponse, TradeError> {
        fetch_all_trade_offer_pages(|cursor| async move {
            let api_client = self.web_api().await?;
            trade_offers_page(&api_client, sent, received, active_only, cursor).await
        })
        .await
    }

    /// Returns `[GetTradeHistoryResponse]` after a call to `GetTradeHistory` endpoint.
    /// `max_trades` If not set will default to a maximum of 500 trade offers.
    ///
    /// Contains Information about completed trades and recovery of the new asset ids that were generated after the
    /// trade.
    async fn get_trade_offers_history(
        &self,
        max_trades: Option<u32>,
        include_failed: bool,
    ) -> Result<GetTradeHistoryResponse, TradeError> {
        let max_trades = max_trades.unwrap_or(500);

        self.web_api()
            .await?
            .get()
            .IEconService()
            .GetTradeHistory(max_trades, include_failed, false, None, None, None, None, None)
            .execute_with_response()
            .err_into()
            .await
    }

    /// The last `max_trades` trades of this account (500 if `None`), newest first, from `GetTradeHistory`: the
    /// request [`SteamTradeManager::trade_lock_end`] and [`SteamTradeManager::get_new_assetids`] make.
    pub async fn trade_history(
        &self,
        max_trades: Option<u32>,
        include_failed: bool,
    ) -> Result<Vec<TradeHistory_Trade>, TradeError> {
        let history = self.get_trade_offers_history(max_trades, include_failed).await?;
        Ok(history.response.trades)
    }

    /// `steamid`'s inventory in `context`, every page of it, as Steam sent it: read its items with
    /// [`Inventory::pages`] and [`InventoryPage::items`], which allocate nothing per item.
    ///
    /// `GET https://steamcommunity.com/inventory/{steamid}/{appid}/{contextid}?l=english&count=1000` with the session's
    /// cookies, then `&start_assetid=…` while Steam says more items come (not seen live: our inventories fit one page).
    /// Another account's inventory must be public; a private one is an HTTP 403. Only the owner sees
    /// [`InventoryContext::CS2_TRADE_PROTECTED`].
    ///
    /// Steam rate limits this endpoint (HTTP 429, [`TradeError::is_too_many_requests`]), and serves it from a cache of
    /// its own: an item just received may take a moment to show.
    ///
    /// # Errors
    ///
    /// HTTP 429 and other non-2xx answers as for every Steam Community request; [`TradeError::GeneralError`] if the
    /// answer isn't an inventory with `success: 1`, or a page can't be followed (a repeated one, or more than 100):
    /// never an incomplete list.
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

    /// This account's trade link, which others send it offers with.
    ///
    /// Asks the Web API for the account's trade offer token, as ArchiSteamFarm asks over CM
    /// (`ArchiSteamFarm/Steam/Integration/ArchiHandler.cs:489-511`, `IEconService.GetTradeOfferAccessToken`):
    /// `GET https://api.steampowered.com/IEconService/GetTradeOfferAccessToken/v1/?access_token=…`, with the session's
    /// access token. If that fails, reads the link from the account's trade offer
    /// privacy page (`GET https://steamcommunity.com/profiles/{steamid}/tradeoffers/privacy`), as node-steamcommunity
    /// (`index.js:310-324`, `getTradeURL`) and aiosteampy (`client/state.py:279-292`, `sync_trade_token`) do.
    ///
    /// **New requests, not verified against live Steam** through this method: the Web API call has never been sent by
    /// us; the privacy page was read live by the full_trade example on 2026-10-20 (not kept in the fixtures).
    ///
    /// # Errors
    ///
    /// The privacy page's error when both fail (the Web API's is logged), [`TradeError::GeneralError`] if the page
    /// has no trade link of this account.
    pub async fn my_trade_link(&self) -> Result<Tradelink, TradeError> {
        let steam_id = self.authenticator.steam_id();
        let access_token = self.authenticator.access_token().await.map_err(InternalError::from)?;
        let token = self
            .get_body(format!(
                "{}?access_token={}",
                TRADE_OFFER_ACCESS_TOKEN_URL,
                access_token.expose_secret()
            ))
            .await
            .and_then(|body| trade_offer_access_token(&body));
        match token {
            Ok(token) => trade_link_of(steam_id, &token),
            Err(error) => {
                warn!(
                    "GetTradeOfferAccessToken failed, reading the trade offer privacy page instead: {}",
                    error
                );
                let url = format!(
                    "https://steamcommunity.com/profiles/{}/tradeoffers/privacy",
                    steam_id.to_steam64()
                );
                trade_link_on_privacy_page(&self.get_body(url).await?, steam_id)
            }
        }
    }

    /// The body of a GET of `url` with the session's cookies, if Steam answered 2xx.
    async fn get_body(&self, url: String) -> Result<String, TradeError> {
        let response = self
            .authenticator
            .request_custom_endpoint(url, Method::GET, None, None::<()>)
            .err_into::<InternalError>()
            .await?;
        let status = response.status();
        let url = response.url().clone();
        let body = response.text().err_into::<InternalError>().await?;
        status_check(status, &url, &body)?;
        Ok(body)
    }

    /// Returns a single trade offer, among the active ones.
    ///
    /// # Errors
    ///
    /// Fails like [`SteamTradeManager::get_trade_offers`], except for offers that could not be read: only one that
    /// may be this offer fails the call, with [`TradeError::MalformedOffer`] if it has this id, or
    /// [`TradeError::MalformedOffers`] if its id could not be read and this offer is not among the others. An offer
    /// whose items can't be trusted is returned: check `offer.has_malformed_items()` before acting on its items.
    pub async fn get_tradeoffer_by_id(&self, tradeoffer_id: u64) -> Result<Vec<TradeOffer_Trade>, TradeError> {
        let offers = self.fetch_trade_offers(true, true, true).await?;
        offers_with_id(offers, tradeoffer_id)
    }

    /// Returns the new asset ids for a trade of `tradeid`.
    ///
    /// Convenience function that internally calls `get_trade_offers_history` but filters it directly.
    ///
    /// # Errors
    ///
    /// [`TradeError::TradeNotInHistory`] if the trade is not among the last 500 trades, and
    /// [`TradeError::TradeNotSettled`] if an asset has no new asset id yet: never a list that leaves an asset out.
    pub async fn get_new_assetids(&self, tradeid: i64) -> Result<Vec<i64>, TradeError> {
        let history = self.get_trade_offers_history(None, false).await?;
        new_assetids(history, tradeid)
    }

    /// Returns when the items of trade `tradeid` stop being trade protected, as Unix time, or `None` if Steam sends
    /// no such time for the trade.
    ///
    /// The time is Steam's own (`time_settlement` in `GetTradeHistory`), not an estimate. Seen live on 2026-10-20: a
    /// CS2 trade accepted at 15:34:44 UTC ended at 16:00 UTC seven days later, the time its item's inventory
    /// description gave ("until 27 Oct @ 9:00am", US Pacific); trades from 2023 had no time. Whether Steam keeps the
    /// time once the protection is over is not checked yet.
    ///
    /// # Errors
    ///
    /// [`TradeError::TradeNotInHistory`] if the trade is not among the last 500 trades.
    pub async fn trade_lock_end(&self, tradeid: i64) -> Result<Option<i64>, TradeError> {
        let history = self.get_trade_offers_history(None, false).await?;
        lock_end(history, tradeid)
    }

    /// Convenience function to auto decline *received* offers.
    ///
    /// Helps to keep the trade offers log clean of the total trade offer limit, if there is one.
    ///
    /// Declines one offer at a time, waiting a second between them. Returns each declined offer's id with its
    /// result: a failed decline doesn't stop the others.
    ///
    /// Offers whose items can't be trusted are declined like any other.
    ///
    /// # Errors
    ///
    /// Only if the offers could not be fetched, as with [`SteamTradeManager::get_trade_offers`]: also when Steam sent
    /// an offer that could not be read, rather than decline the others and leave an unknown one pending unreported.
    /// It then fails every time until that offer is gone: see [`TradeError::MalformedOffers`].
    pub async fn decline_received_offers(&self) -> Result<Vec<(u64, Result<(), TradeError>)>, TradeError> {
        let active_received_offers: Vec<TradeOffer_Trade> = self
            .get_trade_offers(true, true, true)
            .map_ok(|tradeoffers| {
                tradeoffers.filter_by(|offer| offer.state == ETradeOfferState::Active && !offer.is_our_offer)
            })
            .await?;

        let mut results = Vec::with_capacity(active_received_offers.len());
        for (index, tradeoffer_id) in active_received_offers.iter().map(|x| x.tradeofferid).enumerate() {
            if index > 0 {
                Delay::new(Duration::from_millis(STANDARD_DELAY)).await;
            }
            results.push((tradeoffer_id, self.deny_offer(tradeoffer_id).await));
        }

        debug!(
            "Successfully denied {} of {} received offers.",
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            results.len()
        );

        Ok(results)
    }

    /// Creates a new trade offer, and confirms it with the inner [`SteamAuthenticator`].
    ///
    /// Returns the `tradeoffer_id` on success.
    ///
    /// After creating the offer, it still POSTs an accept of it, as it always did, but a failure there is only logged.
    /// It then looks for the offer's mobile confirmation and accepts it if found. When Steam's answer to the creation
    /// asks for no confirmation, the offer is live: it is returned as created even if confirmations could not be
    /// fetched.
    ///
    /// # Errors
    ///
    /// Once the offer exists, every error carries its id: [`ConfirmationError::NotFoundButTradeCreated`] if Steam
    /// said the offer needs a mobile confirmation and none was found, [`TradeError::OfferCreatedButNotConfirmed`] for
    /// any other failure (with [`TradeError::NeedsEmailConfirmation`] inside if Steam waits for an email one).
    pub async fn create_offer_and_confirm(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        let created: TradeOfferCreateResponse = self.request(TradeKind::Create(tradeoffer), None).await?;
        let tradeoffer_id = created_offer_id(&created)?;
        Delay::new(Duration::from_millis(STANDARD_DELAY)).await;
        self.confirm_created_offer(tradeoffer_id, &created)
            .await
            .map_err(|error| offer_created_error(tradeoffer_id, error))?;
        Ok(tradeoffer_id)
    }

    /// Confirms an offer this account just created, as [`confirm_created_offer_steps`] describes.
    async fn confirm_created_offer(
        &self,
        tradeoffer_id: u64,
        created: &TradeOfferCreateResponse,
    ) -> Result<(), TradeError> {
        confirm_created_offer_steps(
            tradeoffer_id,
            created,
            self.request::<TradeOfferCreateResponse>(TradeKind::Accept, Some(tradeoffer_id)),
            self.find_trade_confirmation(tradeoffer_id),
            |confirmation| self.accept_confirmation(confirmation),
        )
        .await
    }

    /// The mobile confirmation of the trade offer `tradeoffer_id`, if there is one.
    async fn find_trade_confirmation(&self, tradeoffer_id: u64) -> Result<Option<Confirmation>, TradeError> {
        Ok(self
            .authenticator
            .fetch_confirmations()
            .inspect_ok(|_| debug!("Confirmations fetched successfully."))
            .await?
            .into_iter()
            .find(|c| c.has_trade_offer_id(tradeoffer_id)))
    }

    /// Accepts `confirmation`, after acknowledging trade protection if this manager hasn't yet: Steam refuses to
    /// confirm trades of trade-protected items before (seen live 2026-10-20).
    async fn accept_confirmation(&self, confirmation: Confirmation) -> Result<(), TradeError> {
        acknowledge_once(&self.acknowledged, || self.acknowledge_trade_protection()).await;
        self.authenticator
            .process_confirmations(ConfirmationAction::Accept, std::iter::once(confirmation))
            .err_into()
            .await
    }

    /// Creates a new trade offer and return its `tradeoffer_id`.
    pub async fn create_offer(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        let response: TradeOfferCreateResponse = self.request(TradeKind::Create(tradeoffer), None).await?;
        created_offer_id(&response)
    }

    /// Accepts a trade offer made to this account and confirms it with inner [SteamAuthenticator]
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    ///
    /// Confirms only when Steam answers `needs_mobile_confirmation: true`; without it, the accept is done.
    ///
    /// Returns the id of the trade the offer became, from Steam's answer to the accept (`{"tradeid": "…"}`, seen live
    /// on 2026-10-20). `None` when Steam asked for a mobile confirmation: that answer has no trade id, and neither has
    /// the confirmation's. The offer has it then: `get_trade_offers(false, true, false)` lists accepted offers too.
    ///
    /// One accept at a time per manager: a call waits for the accept in progress, also one of
    /// `accept_offer_and_find_items` (feature `cs2`). Use one manager per account.
    ///
    /// # Errors
    ///
    /// [`TradeError::NeedsEmailConfirmation`] if Steam waits for an email confirmation instead.
    pub async fn accept_offer(&self, tradeoffer_id: u64) -> Result<Option<i64>, TradeError> {
        let _accepting = self.accepting.lock().await;
        let partner = self.accept_partner(tradeoffer_id).await?;
        self.accept_with_partner(tradeoffer_id, partner).await
    }

    /// The `partner` sent to accept the offer `tradeoffer_id`, from a lookup among the active offers.
    async fn accept_partner(&self, tradeoffer_id: u64) -> Result<u64, TradeError> {
        Ok(self
            .get_tradeoffer_by_id(tradeoffer_id)
            .await?
            .first()
            .map(accept_partner_of)
            .ok_or(OfferError::NoMatch)?)
    }

    /// [`SteamTradeManager::accept_offer`] once the offer was looked up. The caller holds the accept lock.
    async fn accept_with_partner(&self, tradeoffer_id: u64, partner: u64) -> Result<Option<i64>, TradeError> {
        let resp: TradeOfferCreateResponse = self
            .send_request(TradeKind::Accept, Some(tradeoffer_id), Some(partner))
            .await?;

        match accept_outcome(&resp) {
            AcceptOutcome::Done => return Ok(accepted_trade_id(&resp)),
            AcceptOutcome::NeedsEmailConfirmation => {
                return Err(TradeError::NeedsEmailConfirmation { tradeoffer_id });
            }
            AcceptOutcome::NeedsMobileConfirmation => {}
        }

        let confirmation = self
            .find_trade_confirmation(tradeoffer_id)
            .await?
            .ok_or_else(|| TradeError::from(ConfirmationError::NotFound))?;
        self.accept_confirmation(confirmation).await.map(|()| None)
    }

    /// Denies a trade offer sent to this account.
    ///
    /// # Errors
    ///
    /// Will error if couldn't deny the trade offer.
    pub async fn deny_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        self.request::<TradeOfferCancelResponse>(TradeKind::Decline, Some(tradeoffer_id))
            .await
            .map(|_| ())
    }

    /// Cancel a trade offer sent by this account.
    ///
    /// # Errors
    ///
    /// Will error if couldn't cancel the tradeoffer.
    pub async fn cancel_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        self.request::<TradeOfferCancelResponse>(TradeKind::Cancel, Some(tradeoffer_id))
            .await
            .map(|_| ())
    }

    /// Acknowledges Steam's trade protection notice for this account: `POST /trade/new/acknowledge`, as the
    /// browser does when the notice is dismissed.
    ///
    /// The manager calls it before its first confirmation (in [`SteamTradeManager::create_offer_and_confirm`] and
    /// when an accept needs one); it is public for bots that confirm through steam-mobile directly. Once per account
    /// is enough. Steam requires it to confirm trades of trade-protected items: live (2026-10-20, CS2), confirming
    /// such an offer answered `{"success":false}` until the account acknowledged, then `{"success":true}`. Steam
    /// answered `[]`; any 2xx answer without a `strError` is taken as success. ArchiSteamFarm sends it before trading
    /// items of other games than Steam's own (appid 753); node-steamcommunity has it as
    /// `acknowledgeTradeProtection()`.
    pub async fn acknowledge_trade_protection(&self) -> Result<(), TradeError> {
        let (status, url, text) = self.post_community_form(Self::acknowledge_request()).await?;
        check_response(status, &url, &text)
    }

    fn acknowledge_request() -> PreparedRequest {
        PreparedRequest {
            url: TRADE_PROTECTION_ACKNOWLEDGE_URL.to_string(),
            headers: None,
            form: Box::<TradeProtectionAcknowledgeRequest>::default(),
        }
    }

    /// Check current session health, injects SessionID cookie, and send the request.
    async fn request<OUTPUT>(&self, operation: TradeKind, tradeoffer_id: Option<u64>) -> Result<OUTPUT, TradeError>
    where
        OUTPUT: DeserializeOwned + Send + Sync,
    {
        let accept_partner = match &operation {
            TradeKind::Accept => Some(self.accept_partner(tradeoffer_id.unwrap()).await?),
            _ => None,
        };
        self.send_request(operation, tradeoffer_id, accept_partner).await
    }

    /// [`SteamTradeManager::request`] once the offer to accept was looked up: `accept_partner` is the `partner` sent
    /// to accept.
    async fn send_request<OUTPUT>(
        &self,
        operation: TradeKind,
        tradeoffer_id: Option<u64>,
        accept_partner: Option<u64>,
    ) -> Result<OUTPUT, TradeError>
    where
        OUTPUT: DeserializeOwned + Send + Sync,
    {
        #[cfg(feature = "additional-checks")]
        let partner_id_and_token = match &operation {
            TradeKind::Create(offer) => Some((offer.their_tradelink.partner_id, offer.their_tradelink.token.clone())),
            _ => None,
        };

        let request = Self::prepare_request(operation, tradeoffer_id, accept_partner)?;
        let (status, url, response_text) = self.post_community_form(request).await?;
        check_response(status, &url, &response_text)?;

        match serde_json::from_str::<OUTPUT>(&response_text) {
            Ok(response) => Ok(response),
            Err(_) => {
                // try to match into a generic message
                if let Ok(resp) = serde_json::from_str::<TradeOfferGenericErrorResponse>(&response_text) {
                    if let Some(eresult) = resp.eresult {
                        Err(tradeoffer_error_from_eresult(eresult).into())
                    } else {
                        tracing::error!("Unable to understand Steam Response. Please report it as bug.");
                        Err(OfferError::GeneralFailure(format!(
                            "Steam Response: {}\nThis is a bug, please report it.",
                            response_text
                        ))
                        .into())
                    }
                } else {
                    #[cfg(feature = "additional-checks")]
                    if let Some((steamid, token)) = partner_id_and_token {
                        match check_steam_guard_error(&self.authenticator, steamid, &token).await {
                            Err(error @ TradeError::TradeOfferError(OfferError::SteamGuardRecentlyEnabled)) => {
                                return Err(error)
                            }
                            // The check is a second request; its own failure is not why the first one failed.
                            Err(error) => warn!("Could not check the partner's Steam Guard: {}", error),
                            Ok(()) => {}
                        }
                    }

                    tracing::error!(
                        "Failure to deserialize a valid response Steam Offer response. Maybe Steam Servers are \
                         offline."
                    );
                    Err(OfferError::GeneralFailure(format!("Steam Response: {}", response_text)).into())
                }
            }
        }
    }

    /// The URL, headers and form of a trade offer operation. `accept_partner` is the `partner` sent to accept.
    fn prepare_request(
        operation: TradeKind,
        tradeoffer_id: Option<u64>,
        accept_partner: Option<u64>,
    ) -> Result<PreparedRequest, TradeError> {
        let tradeoffer_endpoint = operation.endpoint(tradeoffer_id);

        let mut header: Option<HeaderMap> = None;

        match &operation {
            TradeKind::Create(_) => {
                header.replace(HeaderMap::new());
                header
                    .as_mut()
                    .unwrap()
                    .insert("Referer", (TRADEOFFER_BASE.to_owned() + "new").parse().unwrap());
            }
            TradeKind::Accept => {
                header.replace(HeaderMap::new());
                header.as_mut().unwrap().insert(
                    "Referer",
                    format!("{}{}/", TRADEOFFER_BASE, tradeoffer_id.unwrap())
                        .parse()
                        .unwrap(),
                );
            }
            _ => {}
        };

        let request: Box<dyn HasSessionID> = match operation {
            TradeKind::Accept => {
                let trade_request_data = TradeOfferAcceptRequest {
                    common: TradeOfferCommonParameters {
                        their_steamid: accept_partner.ok_or(OfferError::NoMatch)?,
                        ..Default::default()
                    },
                    tradeofferid: tradeoffer_id.unwrap(),
                    ..Default::default()
                };

                debug!("{:#}", serde_json::to_string_pretty(&trade_request_data).unwrap());
                Box::new(trade_request_data)
            }

            TradeKind::Cancel | TradeKind::Decline => Box::<TradeOfferGenericRequest>::default(),
            TradeKind::Create(offer) => Box::new(Self::prepare_offer(offer)?),
        };

        Ok(PreparedRequest {
            url: tradeoffer_endpoint,
            headers: header,
            form: request,
        })
    }

    /// Adds the session's `sessionid` to the form, posts it, and returns the answer's status, URL and body.
    async fn post_community_form(&self, request: PreparedRequest) -> Result<(StatusCode, Url, String), TradeError> {
        // TODO: Check if session is ok, then inject cookie
        let session_id_cookie = self
            .authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "sessionid")
            .ok_or_else(|| {
                GeneralError("Somehow you don't have a sessionid cookie. You need to login first.".to_string())
            })?;

        let form = with_session_id(request.form, session_id_cookie);

        let response = self
            .authenticator
            .request_custom_endpoint(request.url, Method::POST, request.headers, Some(form))
            .err_into::<InternalError>()
            .await?;
        let status = response.status();
        let url = response.url().clone();
        let response_text = response.text().err_into::<InternalError>().await?;

        Ok((status, url, response_text))
    }

    /// Checks that the tradeoffer is valid, and process it, getting the trade token and steamid3, into a
    /// `TradeOfferCreateRequest`, ready to send it.
    fn prepare_offer(tradeoffer: TradeOffer) -> Result<TradeOfferCreateRequest, TradeError> {
        TradeOffer::validate(&tradeoffer.my_assets, &tradeoffer.their_assets)?;

        let tradelink = tradeoffer.their_tradelink.clone();

        let their_steamid64 = tradelink.partner_id.to_steam64();
        let trade_offer_params = TradeOfferParams {
            trade_offer_access_token: tradelink.token,
        };

        Ok(TradeOfferCreateRequest::new(
            their_steamid64,
            tradeoffer,
            trade_offer_params,
        ))
    }
}

#[cfg(feature = "cs2")]
impl SteamTradeManager {
    /// Makes [`SteamTradeManager::accept_offer_and_find_items`] wait up to `timeout` for the received items, instead
    /// of 60 s.
    pub fn with_received_items_timeout(mut self, timeout: Duration) -> Self {
        self.received_items_timeout = timeout;
        self
    }

    /// Accepts and finds where the items received went, through CS2's game coordinator: since 2026-04 Steam's trade
    /// history doesn't say for CS2 trades. Use it instead of [`SteamTradeManager::accept_offer`] when the received
    /// items matter. Feature `cs2`.
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    ///
    /// `cs2` is this account's session with CS2's game coordinator, borrowed for the call. Its events are not taken:
    /// the call only looks at [`Cs2::inventory`], which reads what came and leaves them as events (kept up to `Cs2`'s
    /// event capacity, then counted as `Lagged`). Steps:
    ///
    /// 1. Looks the offer up, as `accept_offer` does, and takes the items to receive. Entries that can't be read are
    ///    left out (check `has_malformed_items()` first); stackable items (`amount` > 1) count as one.
    /// 2. Fails with [`TradeError::Cs2InventoryUnavailable`] if the game coordinator's inventory isn't current: nothing
    ///    is accepted.
    /// 3. Only if 2+ received items share a kind (`classid`): reads the partner's web inventory, once per context, for
    ///    their pattern and wear. A failure (e.g. a private inventory) is logged.
    /// 4. Notes the game coordinator's item ids, and accepts, as `accept_offer`.
    /// 5. Waits for the new ids, looking at the local inventory every 250 ms (no request), until there are as many as
    ///    CS2 items received, or 60 s passed ([`SteamTradeManager::with_received_items_timeout`]). While the inventory
    ///    isn't current, it waits for the game coordinator to send it again; nothing greets the game coordinator
    ///    meanwhile, so a session lost during the wait ends it.
    /// 6. Places each received item: see [`Located`]. Only when the game coordinator can't tell kinds apart, reads our
    ///    own web inventory (CS2's context 16, where received items land) for the new ids' kinds, again while it misses
    ///    some of them (Steam's cache), 3 times at most.
    ///
    /// Web reads: none for one item; one per partner context for repeated kinds; 1 to 3 of our own only when needed.
    /// Steam rate limits them. Items that come from elsewhere while waiting can be taken for received ones
    /// ([`Located::Only`], [`Located::Guessed`]).
    ///
    /// One accept at a time per manager: this holds the accept lock of `accept_offer` until the items are placed.
    ///
    /// **Not verified against live Steam yet** (spec `docs/specs/2026-10-21-received-items.md`, live checks).
    ///
    /// # Errors
    ///
    /// Before the accept, the plain error: nothing was accepted. After it, only a failure of the game coordinator
    /// (the client stopped), as [`TradeError::AcceptedButItemsNotFound`] with the trade id. Items not found in time
    /// are no error: [`Located::NotFound`].
    pub async fn accept_offer_and_find_items(
        &self,
        tradeoffer_id: u64,
        cs2: &mut Cs2,
    ) -> Result<AcceptedOffer, TradeError> {
        let _accepting = self.accepting.lock().await;
        let offer = self
            .get_tradeoffer_by_id(tradeoffer_id)
            .await?
            .into_iter()
            .next()
            .ok_or(OfferError::NoMatch)?;
        let partner = SteamID::from_account_id(offer.accountid_other as u32);
        let accept_partner = accept_partner_of(&offer);
        let received = offer
            .items_to_receive
            .iter()
            .filter_map(OfferItem::asset)
            .cloned()
            .collect();

        received::accept_and_locate(
            tradeoffer_id,
            received,
            cs2,
            move |context| self.inventory(partner, context),
            move || self.accept_with_partner(tradeoffer_id, accept_partner),
            move || self.own_inventory(InventoryContext::CS2_TRADE_PROTECTED),
            received::Wait::new(self.received_items_timeout),
        )
        .await
    }
}

/// The form as it is sent.
fn with_session_id(mut form: Box<dyn HasSessionID>, sessionid: String) -> Arc<dyn HasSessionID> {
    form.set_sessionid(sessionid);
    Arc::from(form)
}

/// The `partner` sent to accept `offer`.
///
/// Built from the offer id, not from `accountid_other`: wrong by reading, but it is what bots send today and Steam
/// takes it. Changing it needs a live check.
fn accept_partner_of(offer: &TradeOffer_Trade) -> u64 {
    SteamID::from_steam3(offer.tradeofferid as u32, None, None).to_steam64()
}

fn created_offer_id(response: &TradeOfferCreateResponse) -> Result<u64, TradeError> {
    response
        .tradeofferid
        .as_deref()
        .and_then(|id| u64::from_str(id).ok())
        .ok_or_else(|| {
            OfferError::GeneralFailure(format!("Steam's answer has no trade offer id: {:?}", response)).into()
        })
}

/// The trade id in Steam's answer to an accept. One that isn't a number is logged and left out: the accept is done
/// either way.
fn accepted_trade_id(response: &TradeOfferCreateResponse) -> Option<i64> {
    let tradeid = response.tradeid.as_deref()?;
    match i64::from_str(tradeid) {
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

#[derive(serde::Deserialize)]
struct TradeOfferAccessTokenResponse {
    #[serde(default)]
    response: TradeOfferAccessToken,
}

#[derive(Default, serde::Deserialize)]
struct TradeOfferAccessToken {
    trade_offer_access_token: Option<String>,
}

/// The token in Steam's answer to `GetTradeOfferAccessToken` (`CEcon_GetTradeOfferAccessToken_Response`):
/// `{"response":{"trade_offer_access_token":"…"}}`. Steam's Web API answers `{"response":{}}` when it refuses.
fn trade_offer_access_token(body: &str) -> Result<String, TradeError> {
    let answer: TradeOfferAccessTokenResponse = serde_json::from_str(body).map_err(|error| {
        GeneralError(format!(
            "Steam's GetTradeOfferAccessToken answer can't be read: {}",
            error
        ))
    })?;
    answer
        .response
        .trade_offer_access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| GeneralError("Steam's GetTradeOfferAccessToken answer has no token.".to_string()))
}

/// The trade link of `steam_id` with `token`, validated as any trade link.
fn trade_link_of(steam_id: SteamID, token: &str) -> Result<Tradelink, TradeError> {
    Tradelink::new(format!(
        "https://steamcommunity.com/tradeoffer/new/?partner={}&token={}",
        steam_id.account_id(),
        token
    ))
    .map_err(|error| TradeError::from(errors::OfferValidationError::from(error)))
}

lazy_static::lazy_static! {
    /// The link as the privacy page shows it, in an attribute (`&amp;`) or as text; node-steamcommunity's
    /// (`index.js:316`), ASCII only.
    static ref PRIVACY_PAGE_TRADE_LINK: regex::Regex = regex::Regex::new(
        r"https://steamcommunity\.com/tradeoffer/new/\?partner=([0-9]+)&(?:amp;)?token=([A-Za-z0-9_-]+)"
    )
    .unwrap();
}

/// The trade link of `steam_id` on its trade offer privacy page. A link of another account is not taken.
fn trade_link_on_privacy_page(page: &str, steam_id: SteamID) -> Result<Tradelink, TradeError> {
    PRIVACY_PAGE_TRADE_LINK
        .captures_iter(page)
        .find(|link| link[1].parse::<u32>().ok() == Some(steam_id.account_id()))
        .ok_or_else(|| GeneralError("The trade offer privacy page has no trade link of this account.".to_string()))
        .and_then(|link| trade_link_of(steam_id, &link[2]))
}

/// What is left after Steam answered an accept.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AcceptOutcome {
    Done,
    NeedsMobileConfirmation,
    NeedsEmailConfirmation,
}

/// An absent flag is read as `false`, as node-steam-tradeoffer-manager does: an answer with neither flag set is an
/// accepted offer (`lib/classes/TradeOffer.js:611-615`). This deliberately reverses the earlier "assume a mobile
/// confirmation when Steam doesn't say", which failed with `ConfirmationError::NotFound` after a successful accept,
/// e.g. of a gift. A mobile confirmation wins over an email one: it is the one this crate can give.
fn accept_outcome(response: &TradeOfferCreateResponse) -> AcceptOutcome {
    if response.needs_mobile_confirmation == Some(true) {
        AcceptOutcome::NeedsMobileConfirmation
    } else if response.needs_email_confirmation == Some(true) {
        AcceptOutcome::NeedsEmailConfirmation
    } else {
        AcceptOutcome::Done
    }
}

/// What is left after creating an offer and looking for its confirmation.
#[derive(Debug, PartialEq)]
enum CreatedOfferStep<C> {
    Confirm(C),
    Done,
    ConfirmationNotFound,
    NeedsEmailConfirmation,
}

/// A confirmation found is always accepted, as this crate always did. Without one, the answer to the creation decides:
/// `needs_mobile_confirmation: true` is an error, never a success; `needs_email_confirmation: true` alone too, since
/// this crate can't give it; otherwise the offer needed no confirmation (e.g. we give nothing).
fn created_offer_step<C>(confirmation: Option<C>, created: &TradeOfferCreateResponse) -> CreatedOfferStep<C> {
    match confirmation {
        Some(confirmation) => CreatedOfferStep::Confirm(confirmation),
        None if created.needs_mobile_confirmation == Some(true) => CreatedOfferStep::ConfirmationNotFound,
        None if created.needs_email_confirmation == Some(true) => CreatedOfferStep::NeedsEmailConfirmation,
        None => CreatedOfferStep::Done,
    }
}

/// Whether the answer to the creation says the offer is live: neither flag set, as node-steam-tradeoffer-manager
/// reads it (`lib/classes/TradeOffer.js:452-475`, "sent" rather than "pending").
fn created_offer_is_live(created: &TradeOfferCreateResponse) -> bool {
    created.needs_mobile_confirmation != Some(true) && created.needs_email_confirmation != Some(true)
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

/// The steps of [`SteamTradeManager::create_offer_and_confirm`] after the offer `tradeoffer_id` was created, each
/// request given as a future (or, for the last one, a closure) so that tests can stand in for Steam.
///
/// `accept_own_offer` is the POST of an accept of our own offer that this crate always sent (bug 4 in
/// `workingcontext.md`; the wire is frozen until checked live). It still goes out, but its answer is not used, and
/// its failure is only logged: it never stops the confirmation step.
///
/// Confirmations are then always looked for, and one found is always accepted, even when Steam's answer to the
/// creation has no flag: a confirmable offer is never left unconfirmed because Steam omitted a flag. Whether one was
/// needed comes from that answer, as node-steam-tradeoffer-manager decides (`lib/classes/TradeOffer.js:452-460`),
/// see [`created_offer_step`]. If the confirmations can't be fetched (e.g. HTTP 429, an expired session), the offer
/// is reported as created when the answer has no flag, since it is live then, and as failed otherwise.
async fn confirm_created_offer_steps<C, Accept, Find, Confirm, ConfirmFut>(
    tradeoffer_id: u64,
    created: &TradeOfferCreateResponse,
    accept_own_offer: Accept,
    find_confirmation: Find,
    accept_confirmation: Confirm,
) -> Result<(), TradeError>
where
    Accept: Future<Output = Result<TradeOfferCreateResponse, TradeError>>,
    Find: Future<Output = Result<Option<C>, TradeError>>,
    Confirm: FnOnce(C) -> ConfirmFut,
    ConfirmFut: Future<Output = Result<(), TradeError>>,
{
    if let Err(error) = accept_own_offer.await {
        warn!(
            "Accepting our own trade offer {} failed, looking for its confirmation anyway: {}",
            tradeoffer_id, error
        );
    }

    let confirmation = match find_confirmation.await {
        Ok(confirmation) => confirmation,
        Err(error) if created_offer_is_live(created) => {
            warn!(
                "Could not fetch confirmations for trade offer {}, which Steam said needs none: {}",
                tradeoffer_id, error
            );
            return Ok(());
        }
        Err(error) => return Err(error),
    };

    match created_offer_step(confirmation, created) {
        CreatedOfferStep::Confirm(confirmation) => accept_confirmation(confirmation).await,
        CreatedOfferStep::Done => Ok(()),
        CreatedOfferStep::ConfirmationNotFound => Err(ConfirmationError::NotFound.into()),
        CreatedOfferStep::NeedsEmailConfirmation => Err(TradeError::NeedsEmailConfirmation { tradeoffer_id }),
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
            tradeoffer_ids: malformed.iter().map(|offer| offer.tradeofferid()).collect(),
            reason: first.reason.clone(),
        }),
    }
}

/// The offers of `offers` with id `tradeoffer_id`. An error if none is readable and an offer that could not be read
/// may be this one: it has this id, or its id could not be read.
///
/// A readable copy wins over an unreadable one with the same id: Steam may send an offer readable on one page and
/// unreadable on another.
fn offers_with_id(offers: GetTradeOffersResponse, tradeoffer_id: u64) -> Result<Vec<TradeOffer_Trade>, TradeError> {
    let with_id = malformed_offers(&offers).find(|offer| offer.tradeofferid() == Some(tradeoffer_id));
    let without_id: Vec<&MalformedOffer> = malformed_offers(&offers)
        .filter(|offer| offer.tradeofferid().is_none())
        .collect();
    let unreadable = match (with_id, without_id.first()) {
        (Some(malformed), _) => Some(TradeError::MalformedOffer {
            tradeoffer_id,
            reason: malformed.reason.clone(),
        }),
        (None, Some(first)) => Some(TradeError::MalformedOffers {
            tradeoffer_ids: vec![None; without_id.len()],
            reason: first.reason.clone(),
        }),
        (None, None) => None,
    };

    let found = offers.filter_by(|offer| offer.tradeofferid == tradeoffer_id);
    match unreadable {
        Some(error) if found.is_empty() => Err(error),
        _ => Ok(found),
    }
}

/// The error of `create_offer_and_confirm` after the offer `tradeoffer_id` was created.
fn offer_created_error(tradeoffer_id: u64, error: TradeError) -> TradeError {
    match error {
        TradeError::ConfirmationError(ConfirmationError::NotFound) => {
            ConfirmationError::NotFoundButTradeCreated(tradeoffer_id).into()
        }
        error => TradeError::OfferCreatedButNotConfirmed {
            tradeoffer_id,
            source: Box::new(error),
        },
    }
}

/// The new asset ids of the trade `tradeid` in `history`, in the order of [`HasAssets::every_asset`].
fn new_assetids(history: GetTradeHistoryResponse, tradeid: i64) -> Result<Vec<i64>, TradeError> {
    let found_trade = history
        .filter_by(|trade| trade.tradeid == tradeid)
        .into_iter()
        .next()
        .ok_or(TradeError::TradeNotInHistory(tradeid))?;

    found_trade
        .every_asset()
        .into_iter()
        .map(|traded_asset| {
            traded_asset.new_assetid.ok_or(TradeError::TradeNotSettled {
                tradeid,
                assetid: traded_asset.assetid,
            })
        })
        .collect()
}

/// The end of the trade protection of the trade `tradeid` in `history`.
fn lock_end(history: GetTradeHistoryResponse, tradeid: i64) -> Result<Option<i64>, TradeError> {
    history
        .filter_by(|trade| trade.tradeid == tradeid)
        .into_iter()
        .next()
        .map(|trade| trade.time_settlement)
        .ok_or(TradeError::TradeNotInHistory(tradeid))
}

/// One page of `GetTradeOffers`.
async fn trade_offers_page(
    api_client: &SteamAPI,
    sent: bool,
    received: bool,
    active_only: bool,
    cursor: Option<u32>,
) -> Result<GetTradeOffersResponse, TradeError> {
    trade_offers_request(api_client, sent, received, active_only, cursor)
        .execute_with_response()
        .err_into()
        .await
}

/// The request of one page of `GetTradeOffers`. Without a cursor, it is the request this crate always sent.
fn trade_offers_request(
    api_client: &SteamAPI,
    sent: bool,
    received: bool,
    active_only: bool,
    cursor: Option<u32>,
) -> GetTradeOffers<'_> {
    let request = api_client.get().IEconService().GetTradeOffers(
        sent,
        received,
        MAX_HISTORICAL_CUTOFF,
        Some(active_only),
        None,
        None,
        None,
    );
    match cursor {
        Some(cursor) => request.cursor(cursor),
        None => request,
    }
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

    fn get_tradeoffer_url_with_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn"
    }

    fn get_tradeoffer_url_without_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=79925588"
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

    #[test]
    fn new_assets() {
        let raw_response = sample_trade_history_response();
        let filtered = raw_response.filter_by(|x| x.tradeid == 3622543526924228084).remove(0);
        let asset = filtered.every_asset().remove(0);
        assert_eq!(asset.assetid, 15319724006);
        assert_eq!(asset.new_assetid, Some(19793871926));
    }

    #[test]
    fn new_assetids_of_a_trade() {
        assert_eq!(
            new_assetids(sample_trade_history_response(), 3151905948734426645).unwrap(),
            vec![19019879428, 19019879441]
        );
    }

    #[test]
    fn new_assetids_trade_not_in_history() {
        match new_assetids(sample_trade_history_response(), 1) {
            Err(TradeError::TradeNotInHistory(1)) => {}
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn new_assetids_unsettled_trade() {
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

        match new_assetids(history, 42) {
            Err(TradeError::TradeNotSettled {
                tradeid: 42,
                assetid: 101,
            }) => {}
            other => panic!("{:?}", other),
        }
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

    #[test]
    fn lock_end_of_a_protected_trade() {
        // 2026-10-27 16:00:00 UTC
        assert_eq!(
            lock_end(live_trade_history_response(), 695401084301346863).unwrap(),
            Some(1793116800)
        );
    }

    #[test]
    fn lock_end_of_an_old_trade() {
        assert_eq!(
            lock_end(live_trade_history_response(), 4726007333531040160).unwrap(),
            None
        );
    }

    #[test]
    fn lock_end_trade_not_in_history() {
        match lock_end(live_trade_history_response(), 1) {
            Err(TradeError::TradeNotInHistory(1)) => {}
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

    // Expected values were captured from the code before this change (`prepare_offer`, `TradeKind::endpoint`, the
    // header and form building of the old `request()`), with the same inputs.

    #[test]
    fn create_request_is_unchanged() {
        let mut mine = AssetCollection::default();
        mine.add(InventoryContext::CS2, 17034419698);
        mine.add(InventoryContext::DOTA2, 1);
        let mut theirs = AssetCollection::default();
        theirs.add(InventoryContext::CS2, 18116227588);
        let offer = TradeOffer::new(
            get_tradeoffer_url_with_token().to_string(),
            mine,
            theirs,
            "hi & bye, ção=1".to_string(),
        )
        .unwrap();

        let request = SteamTradeManager::prepare_request(TradeKind::Create(offer), None, None).unwrap();
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/new/send");
        assert_eq!(referer(&request), Some("https://steamcommunity.com/tradeoffer/new"));
        assert_eq!(
            sent_form(request),
            "sessionid=SESSION123&serverid=1&partner=76561198040191316&captcha=&tradeoffermessage=hi+%26+bye%2C+%C3%A7%C3%A3o%3D1&json_tradeoffer=%7B%22newversion%22%3Atrue%2C%22version%22%3A2%2C%22me%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A730%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%2217034419698%22%7D%2C%7B%22appid%22%3A570%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%221%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%2C%22them%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A730%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%2218116227588%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%7D&trade_offer_create_params=%7B%22trade_offer_access_token%22%3A%22Ob27qXzn%22%7D"
        );

        let mut mine = AssetCollection::default();
        mine.add(InventoryContext::TF2, 5);
        let offer = TradeOffer::new(
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=a-b_c".to_string(),
            mine,
            None,
            None,
        )
        .unwrap();
        let request = SteamTradeManager::prepare_request(TradeKind::Create(offer), None, None).unwrap();
        assert_eq!(
            sent_form(request),
            "sessionid=SESSION123&serverid=1&partner=76561197960265729&captcha=&tradeoffermessage=&json_tradeoffer=%7B%22newversion%22%3Atrue%2C%22version%22%3A2%2C%22me%22%3A%7B%22assets%22%3A%5B%7B%22appid%22%3A440%2C%22contextid%22%3A%222%22%2C%22amount%22%3A1%2C%22assetid%22%3A%225%22%7D%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%2C%22them%22%3A%7B%22assets%22%3A%5B%5D%2C%22currency%22%3A%5B%5D%2C%22ready%22%3Afalse%7D%7D&trade_offer_create_params=%7B%22trade_offer_access_token%22%3A%22a-b_c%22%7D"
        );
    }

    #[test]
    fn accept_request_is_unchanged() {
        let offer: TradeOffer_Trade =
            serde_json::from_str(r#"{"tradeofferid":"4127395150","accountid_other":79925588,"trade_offer_state":2}"#)
                .unwrap();
        let partner = accept_partner_of(&offer);
        assert_eq!(partner, 76561202087660878);

        let request = SteamTradeManager::prepare_request(TradeKind::Accept, Some(4127395150), Some(partner)).unwrap();
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
        let request = SteamTradeManager::prepare_request(TradeKind::Decline, Some(4127395150), None).unwrap();
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/4127395150/decline");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123");

        let request = SteamTradeManager::prepare_request(TradeKind::Cancel, Some(4127395150), None).unwrap();
        assert_eq!(request.url, "https://steamcommunity.com/tradeoffer/4127395150/cancel");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123");
    }

    #[test]
    fn acknowledge_trade_protection_request() {
        let request = SteamTradeManager::acknowledge_request();
        assert_eq!(request.url, "https://steamcommunity.com/trade/new/acknowledge");
        assert_eq!(referer(&request), None);
        assert_eq!(sent_form(request), "sessionid=SESSION123&message=1");
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
    fn created_offer_without_id() {
        assert_eq!(
            created_offer_id(&create_response(
                r#"{"tradeofferid":"4112828817","needs_mobile_confirmation":true}"#
            ))
            .unwrap(),
            4112828817
        );
        for body in &[
            r#"{}"#,
            r#"{"success":16}"#,
            r#"{"tradeofferid":"abc"}"#,
            r#"{"tradeofferid":null}"#,
        ] {
            match created_offer_id(&create_response(body)) {
                Err(TradeError::TradeOfferError(OfferError::GeneralFailure(_))) => {}
                other => panic!("{}: {:?}", body, other),
            }
        }
    }

    #[test]
    fn trade_id_of_an_accept() {
        // Steam's answer to the accept of `ack-cycle.jsonl`
        assert_eq!(
            accepted_trade_id(&create_response(r#"{"tradeid":"695401084301346863"}"#)),
            Some(695401084301346863)
        );
        assert_eq!(
            accepted_trade_id(&create_response(r#"{"needs_mobile_confirmation":true}"#)),
            None
        );
        assert_eq!(accepted_trade_id(&create_response(r#"{"tradeid":"abc"}"#)), None);
    }

    #[test]
    fn accept_outcome_follows_the_flags() {
        let outcome = |body: &str| accept_outcome(&create_response(body));

        // absent flags: accepted, as node-steam-tradeoffer-manager reads them (e.g. a gift)
        assert_eq!(outcome(r#"{"tradeid":"3151905948734426645"}"#), AcceptOutcome::Done);
        assert_eq!(outcome(r#"{}"#), AcceptOutcome::Done);
        assert_eq!(
            outcome(r#"{"needs_mobile_confirmation":false,"needs_email_confirmation":false}"#),
            AcceptOutcome::Done
        );

        assert_eq!(
            outcome(r#"{"needs_mobile_confirmation":true}"#),
            AcceptOutcome::NeedsMobileConfirmation
        );
        assert_eq!(
            outcome(r#"{"needs_mobile_confirmation":true,"needs_email_confirmation":true}"#),
            AcceptOutcome::NeedsMobileConfirmation
        );

        assert_eq!(
            outcome(r#"{"needs_email_confirmation":true,"email_domain":"example.com"}"#),
            AcceptOutcome::NeedsEmailConfirmation
        );
        assert_eq!(
            outcome(r#"{"needs_mobile_confirmation":false,"needs_email_confirmation":true}"#),
            AcceptOutcome::NeedsEmailConfirmation
        );
    }

    #[test]
    fn errors_after_creation_carry_the_offer_id() {
        match offer_created_error(7, ConfirmationError::NotFound.into()) {
            TradeError::ConfirmationError(ConfirmationError::NotFoundButTradeCreated(7)) => {}
            other => panic!("{:?}", other),
        }
        match offer_created_error(7, OfferError::NoMatch.into()) {
            TradeError::OfferCreatedButNotConfirmed {
                tradeoffer_id: 7,
                source,
            } => {
                assert!(matches!(*source, TradeError::TradeOfferError(OfferError::NoMatch)))
            }
            other => panic!("{:?}", other),
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
                Some(_) => Err(tappet::errors::SteamAPIError::SteamHttpError("429".to_string()).into()),
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
    fn offer_by_id() {
        let offers = serde_json::from_str::<GetTradeOffersResponse>(
            r#"{"response":{
                "trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2},
                                     {"tradeofferid":"3","trade_offer_state":2}],
                "trade_offers_received":[{"tradeofferid":"5","accountid_other":2,"trade_offer_state":2}]}}"#,
        )
        .unwrap();

        assert_eq!(ids(&Some(offers_with_id(offers.clone(), 1).unwrap())), vec![1]);
        assert_eq!(ids(&Some(offers_with_id(offers.clone(), 5).unwrap())), vec![5]);
        assert!(offers_with_id(offers.clone(), 7).unwrap().is_empty());
        match offers_with_id(offers, 3) {
            Err(TradeError::MalformedOffer {
                tradeoffer_id: 3,
                reason,
            }) => {
                assert!(reason.contains("accountid_other"), "{}", reason)
            }
            other => panic!("{:?}", other),
        }

        // an offer without a readable id may be any missing one, not a readable one
        let offers = serde_json::from_str::<GetTradeOffersResponse>(
            r#"{"response":{"trade_offers_received":[
                {"tradeofferid":"5","accountid_other":2,"trade_offer_state":2},
                {"accountid_other":2,"trade_offer_state":2}]}}"#,
        )
        .unwrap();
        assert_eq!(ids(&Some(offers_with_id(offers.clone(), 5).unwrap())), vec![5]);
        match offers_with_id(offers, 7) {
            Err(TradeError::MalformedOffers { tradeoffer_ids, .. }) => assert_eq!(tradeoffer_ids, vec![None]),
            other => panic!("{:?}", other),
        }
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
                    TradeError::MalformedOffers { tradeoffer_ids, .. } => {
                        assert_eq!(tradeoffer_ids, vec![Some(3), None])
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
            Err(TradeError::MalformedOffers { tradeoffer_ids, .. }) => {
                assert_eq!(tradeoffer_ids, vec![Some(1), None, None])
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn created_offer_confirmation() {
        let step = |found: bool, body: &str| created_offer_step(found.then(|| ()), &create_response(body));

        // a confirmation found is always accepted
        for body in &[
            r#"{"tradeofferid":"1","needs_mobile_confirmation":true}"#,
            r#"{"tradeofferid":"1","needs_mobile_confirmation":false}"#,
            r#"{"tradeofferid":"1"}"#,
            r#"{"tradeofferid":"1","needs_email_confirmation":true}"#,
        ] {
            assert_eq!(step(true, body), CreatedOfferStep::Confirm(()), "{}", body);
        }

        // Steam said one is needed: never a success without it
        assert_eq!(
            step(false, r#"{"tradeofferid":"1","needs_mobile_confirmation":true}"#),
            CreatedOfferStep::ConfirmationNotFound
        );
        assert_eq!(
            step(
                false,
                r#"{"tradeofferid":"1","needs_mobile_confirmation":true,"needs_email_confirmation":true}"#
            ),
            CreatedOfferStep::ConfirmationNotFound
        );
        assert_eq!(
            step(false, r#"{"tradeofferid":"1","needs_email_confirmation":true}"#),
            CreatedOfferStep::NeedsEmailConfirmation
        );

        // no confirmation needed
        for body in &[
            r#"{"tradeofferid":"1"}"#,
            r#"{"tradeofferid":"1","needs_mobile_confirmation":false,"needs_email_confirmation":false}"#,
        ] {
            assert_eq!(step(false, body), CreatedOfferStep::Done, "{}", body);
        }

        // and the error the caller sees
        match offer_created_error(7, ConfirmationError::NotFound.into()) {
            TradeError::ConfirmationError(ConfirmationError::NotFoundButTradeCreated(7)) => {}
            other => panic!("{:?}", other),
        }
    }

    const ACCEPT_OWN_OFFER: &str = "accept own offer";
    const FETCH_CONFIRMATIONS: &str = "fetch confirmations";
    const ACCEPT_CONFIRMATION: &str = "accept confirmation";

    /// Runs the steps after creating offer 7 against stand-ins for Steam's answers; returns the result as
    /// `create_offer_and_confirm` returns it, and the steps taken.
    fn run_created_offer_steps(
        created: &str,
        accept_own_offer: Result<TradeOfferCreateResponse, TradeError>,
        confirmations: Result<Option<u64>, TradeError>,
        accept_confirmation: Result<(), TradeError>,
    ) -> (Result<(), TradeError>, Vec<&'static str>) {
        let created = create_response(created);
        let steps = std::cell::RefCell::new(vec![]);
        let steps_ref = &steps;

        let result = futures::executor::block_on(confirm_created_offer_steps(
            7,
            &created,
            async move {
                steps_ref.borrow_mut().push(ACCEPT_OWN_OFFER);
                accept_own_offer
            },
            async move {
                steps_ref.borrow_mut().push(FETCH_CONFIRMATIONS);
                confirmations
            },
            |confirmation: u64| {
                assert_eq!(confirmation, 7);
                steps_ref.borrow_mut().push(ACCEPT_CONFIRMATION);
                async move { accept_confirmation }
            },
        ))
        .map_err(|error| offer_created_error(7, error));

        (result, steps.into_inner())
    }

    fn rate_limited_fetch() -> TradeError {
        steam_mobile::errors::AuthError::from(steam_mobile::errors::InternalError::TooManyRequests {
            url: "https://steamcommunity.com/mobileconf/getlist".to_string(),
        })
        .into()
    }

    fn assert_not_confirmed(result: Result<(), TradeError>, check_source: impl FnOnce(&TradeError) -> bool) {
        match result {
            Err(TradeError::OfferCreatedButNotConfirmed {
                tradeoffer_id: 7,
                source,
            }) => assert!(check_source(&source), "{:?}", source),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn failed_accept_of_own_offer_still_confirms() {
        let url = Url::parse("https://steamcommunity.com/tradeoffer/7/accept").unwrap();
        let accept_failures = || -> Vec<TradeError> {
            vec![
                check_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &url,
                    r#"{"strError":"There was an error accepting this trade offer. Please try again later. (28)"}"#,
                )
                .unwrap_err(),
                check_response(StatusCode::BAD_GATEWAY, &url, "<html></html>").unwrap_err(),
                check_response(StatusCode::TOO_MANY_REQUESTS, &url, "").unwrap_err(),
                // the lookup before the POST
                OfferError::NoMatch.into(),
                TradeError::MalformedOffer {
                    tradeoffer_id: 7,
                    reason: "missing field `accountid_other`".to_string(),
                },
                TradeError::MalformedOffers {
                    tradeoffer_ids: vec![None],
                    reason: "missing field `tradeofferid`".to_string(),
                },
                // no POST at all
                GeneralError("Somehow you don't have a sessionid cookie. You need to login first.".to_string()),
            ]
        };

        for accept_failure in accept_failures() {
            let failure = accept_failure.to_string();
            let (result, steps) = run_created_offer_steps(
                r#"{"tradeofferid":"7","needs_mobile_confirmation":true}"#,
                Err(accept_failure),
                Ok(Some(7)),
                Ok(()),
            );
            assert!(result.is_ok(), "{}: {:?}", failure, result);
            assert_eq!(
                steps,
                vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS, ACCEPT_CONFIRMATION],
                "{}",
                failure
            );
        }

        // and the answer to the creation still decides without one
        for accept_failure in accept_failures() {
            let (result, steps) = run_created_offer_steps(
                r#"{"tradeofferid":"7","needs_mobile_confirmation":true}"#,
                Err(accept_failure),
                Ok(None),
                Ok(()),
            );
            assert!(matches!(
                result,
                Err(TradeError::ConfirmationError(
                    ConfirmationError::NotFoundButTradeCreated(7)
                ))
            ));
            assert_eq!(steps, vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS]);
        }
        for accept_failure in accept_failures() {
            let (result, _) = run_created_offer_steps(r#"{"tradeofferid":"7"}"#, Err(accept_failure), Ok(None), Ok(()));
            assert!(result.is_ok(), "{:?}", result);
        }

        // its answer is not used either
        let (result, steps) = run_created_offer_steps(
            r#"{"tradeofferid":"7","needs_mobile_confirmation":true}"#,
            Ok(create_response(r#"{"needs_mobile_confirmation":false}"#)),
            Ok(Some(7)),
            Ok(()),
        );
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(steps, vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS, ACCEPT_CONFIRMATION]);
    }

    #[test]
    fn created_offer_flags_and_confirmations() {
        let accepted = || -> Result<_, TradeError> { Ok(create_response(r#"{"tradeid":"1"}"#)) };
        let live = [
            r#"{"tradeofferid":"7"}"#,
            r#"{"tradeofferid":"7","needs_mobile_confirmation":false,"needs_email_confirmation":false}"#,
        ];
        let mobile = [
            r#"{"tradeofferid":"7","needs_mobile_confirmation":true}"#,
            r#"{"tradeofferid":"7","needs_mobile_confirmation":true,"needs_email_confirmation":true}"#,
        ];
        let email = r#"{"tradeofferid":"7","needs_email_confirmation":true}"#;
        let all = live.iter().chain(&mobile).chain(std::iter::once(&email));

        // a confirmation found is accepted whatever the flags, and its failure is the offer's
        for created in all.clone() {
            let (result, steps) = run_created_offer_steps(created, accepted(), Ok(Some(7)), Ok(()));
            assert!(result.is_ok(), "{}: {:?}", created, result);
            assert_eq!(steps, vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS, ACCEPT_CONFIRMATION]);

            let (result, _) = run_created_offer_steps(created, accepted(), Ok(Some(7)), Err(rate_limited_fetch()));
            assert_not_confirmed(result, TradeError::is_too_many_requests);
        }

        // no flag: the offer is live, found or not, fetched or not
        for created in &live {
            let (result, steps) = run_created_offer_steps(created, accepted(), Ok(None), Ok(()));
            assert!(result.is_ok(), "{}: {:?}", created, result);
            assert_eq!(steps, vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS]);

            let (result, steps) = run_created_offer_steps(created, accepted(), Err(rate_limited_fetch()), Ok(()));
            assert!(result.is_ok(), "{}: {:?}", created, result);
            assert_eq!(steps, vec![ACCEPT_OWN_OFFER, FETCH_CONFIRMATIONS]);
        }

        // a mobile confirmation is needed: never a success without it
        for created in &mobile {
            let (result, _) = run_created_offer_steps(created, accepted(), Ok(None), Ok(()));
            assert!(matches!(
                result,
                Err(TradeError::ConfirmationError(
                    ConfirmationError::NotFoundButTradeCreated(7)
                ))
            ));

            let (result, _) = run_created_offer_steps(created, accepted(), Err(rate_limited_fetch()), Ok(()));
            assert_not_confirmed(result, TradeError::is_too_many_requests);
        }

        // an email one, which this crate can't give
        let (result, _) = run_created_offer_steps(email, accepted(), Ok(None), Ok(()));
        assert_not_confirmed(result, |source| {
            matches!(source, TradeError::NeedsEmailConfirmation { tradeoffer_id: 7 })
        });
        let (result, _) = run_created_offer_steps(email, accepted(), Err(rate_limited_fetch()), Ok(()));
        assert_not_confirmed(result, TradeError::is_too_many_requests);
    }

    #[test]
    fn readable_copy_of_an_offer_wins() {
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
            assert_eq!(ids(&Some(offers_with_id(merged.clone(), 1).unwrap())), vec![1]);
            // the list as a whole still fails, as Steam did send an unreadable offer
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
                Err(TradeError::MalformedOffers { tradeoffer_ids, .. }) => {
                    assert_eq!(tradeoffer_ids, vec![Some(3), Some(1), None, None])
                }
                other => panic!("{:?}", other),
            }
        }
    }

    // Expected values: the URL tappet's `request_tests` captured from the code before pages were read, with the
    // approved changes (https instead of http, the access token instead of the API key: spec change 1). `prepare`
    // builds the request `execute` sends, without sending it.
    const FIRST_PAGE_URL: &str = "https://api.steampowered.com/IEconService/GetTradeOffers/v1?access_token=TOKEN&&get_sent_offers=true&get_received_offers=true&time_historical_cutoff=4294967295&active_only=true";

    fn prepared_url(request: GetTradeOffers<'_>) -> String {
        let request = request.prepare().unwrap();
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
            sent.push(prepared_url(trade_offers_request(
                &api_client,
                true,
                true,
                true,
                cursor,
            )));
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
                prepared_url(trade_offers_request(&api_client, false, true, true, None))
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

    /// B of the live tests stands in: account id 1, SteamID 76561197960265729.
    fn account() -> SteamID {
        SteamID::from_account_id(1)
    }

    #[test]
    fn trade_offer_access_token_answer() {
        assert_eq!(
            trade_offer_access_token(r#"{"response":{"trade_offer_access_token":"Ob27qXzn"}}"#).unwrap(),
            "Ob27qXzn"
        );
        for refused in &[
            r#"{"response":{}}"#,
            r#"{}"#,
            r#"{"response":{"trade_offer_access_token":""}}"#,
            "<html>",
        ] {
            assert!(
                matches!(trade_offer_access_token(refused), Err(GeneralError(_))),
                "{}",
                refused
            );
        }
    }

    #[test]
    fn trade_link_from_token() {
        let link = trade_link_of(account(), "Ob27qXzn").unwrap();
        assert_eq!(
            link.link,
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn"
        );
        assert_eq!(link.partner_id, account());
        assert_eq!(link.token, "Ob27qXzn");
        // a token no trade link has
        assert!(matches!(
            trade_link_of(account(), "a&token=b"),
            Err(TradeError::TradeValidationError(_))
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
            link.link,
            "https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn"
        );

        // as text, unescaped
        let text = "Trade URL: https://steamcommunity.com/tradeoffer/new/?partner=1&token=Ob27qXzn";
        assert_eq!(trade_link_on_privacy_page(text, account()).unwrap().token, "Ob27qXzn");
    }

    #[test]
    fn privacy_page_without_our_link() {
        let others = PRIVACY_PAGE.replace("partner=1&", "partner=2&");
        for page in &[others.as_str(), "<html>Sign In</html>", ""] {
            assert!(
                matches!(trade_link_on_privacy_page(page, account()), Err(GeneralError(_))),
                "{}",
                page
            );
        }
    }
}

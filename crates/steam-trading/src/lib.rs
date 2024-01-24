//! Steam trade manager is the module that allows you to automate trade offers, by extending `SteamAuthenticator`.
//!
//! It inherently needs `SteamAuthenticator` as a dependency, since we need cookies from Steam Community and Steam Store
//! to be able to create and accept trade offers, along with mobile confirmations.
//!
//! **IT IS VERY IMPORTANT THAT STEAM GUARD IS ENABLED ON THE ACCOUNT BEING USED, WITH MOBILE CONFIRMATIONS.**
//!
//! Currently, `SteamAuthenticator` is "stateless", in comparison of alternatives such as Node.js.
//! This means that it does not need to stay up running and react to events.
//!
//! But this also means that you will need to keep track of trades and polling yourself, but it won't be much work,
//! since there are convenience functions for almost every need.
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

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use const_format::concatcp;
pub use errors::ConfirmationError;
pub use errors::InternalError;
pub use errors::OfferError;
pub use errors::TradeError;
pub use errors::TradelinkError;
use futures::stream::FuturesOrdered;
use futures::StreamExt;
use futures::TryFutureExt;
use futures_timer::Delay;
use serde::de::DeserializeOwned;
use steam_language_gen::generated::enums::ETradeOfferState;
use steam_mobile::user::PresentMaFile;
use steam_mobile::Authenticated;
use steam_mobile::ConfirmationAction;
use steam_mobile::HeaderMap;
use steam_mobile::Method;
use steam_mobile::SteamAuthenticator;
use steam_mobile::STEAM_COMMUNITY_HOST;
use steamid_parser::SteamID;
use tappet::response_types::GetTradeHistoryResponse;
use tappet::response_types::GetTradeOffersResponse;
use tappet::response_types::TradeHistory_Trade;
use tappet::response_types::TradeOffer_Trade;
use tappet::ExecutorResponse;
use tappet::SteamAPI;
use tracing::debug;
pub use types::asset_collection::AssetCollection;
pub use types::trade_link::Tradelink;
pub use types::trade_offer::TradeOffer;

use crate::additional_checks::check_steam_guard_error;
use crate::api_extensions::FilterBy;
use crate::api_extensions::HasAssets;
use crate::errors::error_from_strmessage;
use crate::errors::tradeoffer_error_from_eresult;
use crate::errors::TradeError::GeneralError;
use crate::types::sessionid::HasSessionID;
use crate::types::trade_offer_web::TradeOfferAcceptRequest;
use crate::types::trade_offer_web::TradeOfferCancelResponse;
use crate::types::trade_offer_web::TradeOfferCommonParameters;
use crate::types::trade_offer_web::TradeOfferCreateRequest;
use crate::types::trade_offer_web::TradeOfferCreateResponse;
use crate::types::trade_offer_web::TradeOfferGenericErrorResponse;
use crate::types::trade_offer_web::TradeOfferGenericRequest;
use crate::types::trade_offer_web::TradeOfferParams;
use crate::types::TradeKind;

mod additional_checks;
pub mod api_extensions;
mod errors;
#[cfg(feature = "time")]
pub mod time;
mod types;

const TRADEOFFER_BASE: &str = "https://steamcommunity.com/tradeoffer/";
const TRADEOFFER_NEW_URL: &str = concatcp!(TRADEOFFER_BASE, "new/send");

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

pub(crate) type SteamCompleteAuthenticator = SteamAuthenticator<Authenticated, PresentMaFile>;

#[derive(Debug)]
pub struct SteamTradeManager<'a> {
    authenticator: &'a SteamCompleteAuthenticator,
    api_client: SteamAPI,
}

impl<'a> SteamTradeManager<'a> {
    /// Returns a new `[SteamTradeManager]`.
    ///
    /// # Errors
    ///
    /// Returns an error if API Key is not cached by `authenticator`.
    pub fn new(
        authenticator: &'a SteamAuthenticator<Authenticated, PresentMaFile>,
    ) -> Result<SteamTradeManager<'a>, TradeError> {
        let api_key = authenticator
            .api_key()
            .ok_or_else(|| GeneralError("Can't build without an API Key cached.".to_string()))?;

        Ok(Self {
            authenticator,
            api_client: SteamAPI::new(api_key),
        })
    }

    /// Checks whether the user of `tradelink` has recently activated his mobile SteamGuard.
    pub async fn check_steam_guard_recently_activated(&self, tradelink: Tradelink) -> Result<(), TradeError> {
        let Tradelink { partner_id, token, .. } = tradelink;

        check_steam_guard_error(self.authenticator, partner_id, &*token)
            .err_into()
            .await
    }

    /// Call to GetTradeOffers endpoint.
    ///
    /// Convenience function that fetches information about active trades for the current logged in account.
    pub async fn get_trade_offers(
        &self,
        sent: bool,
        received: bool,
        active_only: bool,
    ) -> Result<GetTradeOffersResponse, TradeError> {
        self.api_client
            .get()
            .IEconService()
            .GetTradeOffers(
                sent,
                received,
                MAX_HISTORICAL_CUTOFF,
                Some(active_only),
                None,
                None,
                None,
            )
            .execute_with_response()
            .err_into()
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

        self.api_client
            .get()
            .IEconService()
            .GetTradeHistory(max_trades, include_failed, false, None, None, None, None, None)
            .execute_with_response()
            .err_into()
            .await
    }

    /// Returns a single trade offer.
    pub async fn get_tradeoffer_by_id(&self, tradeoffer_id: u64) -> Result<Vec<TradeOffer_Trade>, TradeError> {
        self.get_trade_offers(true, true, true)
            .map_ok(|tradeoffers| tradeoffers.filter_by(|offer| offer.tradeofferid == tradeoffer_id))
            .await
    }

    /// Returns the new asset ids for a trade of `tradeid`.
    ///
    /// Convenience function that internally calls `get_trade_offers_history` but filters it directly.
    pub async fn get_new_assetids(&self, tradeid: i64) -> Result<Vec<i64>, TradeError> {
        let found_trade: TradeHistory_Trade = self
            .get_trade_offers_history(None, false)
            .map_ok(|tradeoffers| tradeoffers.filter_by(|trade| trade.tradeid == tradeid))
            .await?
            .swap_remove(0);

        Ok(found_trade
            .every_asset()
            .into_iter()
            .map(|traded_asset| traded_asset.new_assetid)
            .collect::<Vec<_>>())
    }

    /// Convenience function to auto decline *received* offers.
    ///
    /// Helps to keep the trade offers log clean of the total trade offer limit, if there is one.
    pub async fn decline_received_offers(&self) -> Result<(), TradeError> {
        let mut deny_offers_fut = FuturesOrdered::new();

        let active_received_offers: Vec<TradeOffer_Trade> = self
            .get_trade_offers(true, true, true)
            .map_ok(|tradeoffers| {
                tradeoffers.filter_by(|offer| offer.state == ETradeOfferState::Active && !offer.is_our_offer)
            })
            .await?;

        let total = active_received_offers.len();
        active_received_offers
            .into_iter()
            .map(|x| x.tradeofferid)
            .for_each(|x| {
                deny_offers_fut.push_back(
                    self.deny_offer(x)
                        .map_ok(|_| Delay::new(Duration::from_millis(STANDARD_DELAY))),
                );
            });

        while let Some(result) = deny_offers_fut.next().await {
            match result {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }

        debug!("Successfully denied a total of {} received offers.", total);

        Ok(())
    }

    /// Creates a new trade offer, and confirms it with the inner [`SteamAuthenticator`].
    ///
    /// Returns the `tradeoffer_id` on success and if the confirmation was not found but the trade created.
    pub async fn create_offer_and_confirm(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        let tradeoffer_id = self.create_offer(tradeoffer).await?;
        Delay::new(Duration::from_millis(STANDARD_DELAY));
        self.accept_offer(tradeoffer_id).await?;
        Ok(tradeoffer_id)
    }

    /// Creates a new trade offer and return its `tradeoffer_id`.
    pub async fn create_offer(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        self.request::<TradeOfferCreateResponse>(TradeKind::Create(tradeoffer), None)
            .map_ok(|c| {
                c.tradeofferid
                    .and_then(|id| u64::from_str(&id).ok())
                    .expect("Safe to unwrap.")
            })
            .await
    }

    /// Accepts a trade offer made to this account and confirms it with inner [SteamAuthenticator]
    ///
    /// **Note: This is irreversable, be extra careful when accepting any trade offer.**
    pub async fn accept_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        let resp: TradeOfferCreateResponse = self.request(TradeKind::Accept, Some(tradeoffer_id)).await?;

        if resp.needs_mobile_confirmation.is_none() && !resp.needs_mobile_confirmation.unwrap() {
            return Ok(());
        }

        let confirmation = self
            .authenticator
            .fetch_confirmations()
            .inspect_ok(|_| debug!("Confirmations fetched successfully."))
            .await?
            .into_iter()
            .find(|c| c.has_trade_offer_id(tradeoffer_id))
            .ok_or_else(|| TradeError::from(ConfirmationError::NotFound))?;

        self.authenticator
            .process_confirmations(ConfirmationAction::Accept, std::iter::once(confirmation))
            .err_into()
            .await
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

    /// Check current session health, injects SessionID cookie, and send the request.
    async fn request<OUTPUT>(&self, operation: TradeKind, tradeoffer_id: Option<u64>) -> Result<OUTPUT, TradeError>
    where
        OUTPUT: DeserializeOwned + Send + Sync,
    {
        let tradeoffer_endpoint = operation.endpoint(tradeoffer_id);

        let mut header: Option<HeaderMap> = None;
        let mut partner_id_and_token = None;

        match &operation {
            TradeKind::Create(offer) => {
                header.replace(HeaderMap::new());
                header
                    .as_mut()
                    .unwrap()
                    .insert("Referer", (TRADEOFFER_BASE.to_owned() + "new").parse().unwrap());

                partner_id_and_token = Some((
                    offer.their_tradelink.partner_id.clone(),
                    offer.their_tradelink.token.clone(),
                ));
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

        let mut request: Box<dyn HasSessionID> = match operation {
            TradeKind::Accept => {
                let partner_id = self
                    .get_tradeoffer_by_id(tradeoffer_id.unwrap())
                    .await?
                    .first()
                    .map(|c| SteamID::from_steam3(c.tradeofferid as u32, None, None))
                    .map(|steamid| steamid.to_steam64())
                    .ok_or(OfferError::NoMatch)?;

                let trade_request_data = TradeOfferAcceptRequest {
                    common: TradeOfferCommonParameters {
                        their_steamid: partner_id,
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

        // TODO: Check if session is ok, then inject cookie
        let session_id_cookie = self
            .authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "sessionid")
            .ok_or_else(|| {
                GeneralError("Somehow you don't have a sessionid cookie. You need to login first.".to_string())
            })?;

        request.set_sessionid(session_id_cookie);
        let request: Arc<dyn HasSessionID> = Arc::from(request);

        let response = self
            .authenticator
            .request_custom_endpoint(tradeoffer_endpoint, Method::POST, header, Some(request))
            .err_into::<InternalError>()
            .await?;
        let response_text = response.text().err_into::<InternalError>().await?;

        match serde_json::from_str::<OUTPUT>(&response_text) {
            Ok(response) => Ok(response),
            Err(_) => {
                // try to match into a generic message
                if let Ok(resp) = serde_json::from_str::<TradeOfferGenericErrorResponse>(&response_text) {
                    if resp.error_message.is_some() {
                        let err_msg = resp.error_message.unwrap();
                        Err(error_from_strmessage(&*err_msg).unwrap().into())
                    } else if resp.eresult.is_some() {
                        let eresult = resp.eresult.unwrap();
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
                    if let Some((steamid, token)) = partner_id_and_token {
                        check_steam_guard_error(self.authenticator, steamid, &token).await?;
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
        assert_eq!(asset.new_assetid, 19793871926);
    }

    #[cfg(feature = "time")]
    #[test]
    fn estimate_time() {
        use crate::time::estimate_tradelock_end;
        use crate::time::ONE_WEEK_SECONDS;

        let raw_response = sample_trade_history_response();
        let filtered_trade = raw_response.filter_by(|x| x.tradeid == 3622543526924228084).remove(0);
        let trade_completed_time = filtered_trade.time_init;
        assert_eq!(
            estimate_tradelock_end(trade_completed_time, ONE_WEEK_SECONDS).timestamp(),
            1604649600
        );
    }
}

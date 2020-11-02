//! Steam trade manager is the module that allows you to automate trade offers, by extending `SteamAuthenticator`.
//!
//! It inherently needs SteamAuthenticator as a dependency, since we need cookies from Steam Community and Steam Store to be able to
//! create and accept trade offers, along with mobile confirmations.
//!
//! **IT IS VERY IMPORTANT THAT STEAM GUARD IS ENABLED ON THE ACCOUNT BEING USED, WITH MOBILE CONFIRMATIONS.**
//!
//! Currently, `SteamAuthenticator` is "stateless", in comparison of alternatives such as Node.js.
//! This means that it does not need to stay up running and react to events.
//!
//! But this also means that you will need to keep track of trades and polling yourself, but it won't be much work, since there are
//! convenience functions for almost every need.
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

use std::cell::RefCell;
use std::rc::Rc;

use const_format::concatcp;
use futures::stream::FuturesOrdered;
use futures::{StreamExt, TryFutureExt};
use serde::de::DeserializeOwned;
use tokio::time::Duration;
use tracing::debug;

use steam_auth::{
    client::SteamAuthenticator, ConfirmationMethod, Confirmations, HeaderMap, Method, STEAM_COMMUNITY_HOST,
};
use steam_language_gen::generated::enums::ETradeOfferState;
use steam_web_api::{Executor, SteamAPI};
use steamid_parser::SteamID;
pub use types::{asset_collection::AssetCollection, trade_offer::TradeOffer};

use crate::api_extensions::{FilterBy, HasAssets};
use crate::types::trade_api::AssetIDHistory;
use crate::{
    errors::TradeError,
    errors::TradeError::PayloadError,
    errors::{tradeoffer_error_from_eresult, ConfirmationError, TradeOfferError},
    types::{
        sessionid::HasSessionID,
        trade_api::GetTradeHistoryResponse,
        trade_api::GetTradeOffersResponse,
        trade_api::TradeHistory_Trade,
        trade_api::TradeOffer_Trade,
        trade_offer_web::{
            TradeOfferAcceptRequest, TradeOfferCommonParameters, TradeOfferCreateRequest, TradeOfferCreateResponse,
            TradeOfferGenericErrorResponse, TradeOfferGenericRequest, TradeOfferParams,
        },
        TradeKind,
    },
};

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

#[derive(Debug)]
pub struct SteamTradeManager<'a> {
    authenticator: &'a SteamAuthenticator,
    api_client: Rc<RefCell<Option<SteamAPI>>>,
}

impl<'a> SteamTradeManager<'a> {
    pub fn new(authenticator: &'a SteamAuthenticator) -> SteamTradeManager<'a> {
        Self {
            authenticator: &authenticator,
            api_client: Rc::new(RefCell::new(None)),
        }
    }

    /// SteamAPI only gets created if API methods are needed.
    /// Returns a reference to the `api_client`.
    fn lazy_web_api_client<T: ToString>(&self, api_key: T) -> &Rc<RefCell<Option<SteamAPI>>> {
        {
            let mut api_client = self.api_client.borrow_mut();

            match *api_client {
                Some(_) => {}
                None => *api_client = Some(SteamAPI::new(api_key.to_string())),
            };
        }

        &self.api_client
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
        let api_key = self
            .authenticator
            .api_key()
            .expect("Api should be cached for this method to work.");
        let api_client = self.lazy_web_api_client(api_key).borrow();

        let response = api_client
            .as_ref()
            .unwrap()
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
            .execute()
            .await?;

        Ok(serde_json::from_str::<GetTradeOffersResponse>(&response).unwrap())
    }

    /// Call to GetTradeHistory endpoint.
    /// If not set, defaults to a max of 500 trade offers.
    ///
    /// Information about completed trades, and recover new asset ids.
    async fn get_trade_offers_history(
        &self,
        max_trades: Option<u32>,
        include_failed: bool,
    ) -> Result<GetTradeHistoryResponse, TradeError> {
        let api_key = self
            .authenticator
            .api_key()
            .expect("API key must be cached in order to use this.");
        let max_trades = max_trades.unwrap_or(500);
        let api_client = self.lazy_web_api_client(api_key).borrow();

        let response = api_client
            .as_ref()
            .unwrap()
            .get()
            .IEconService()
            .GetTradeHistory(max_trades, include_failed, false, None, None, None, None, None)
            .execute()
            .await?;

        Ok(serde_json::from_str::<GetTradeHistoryResponse>(&response).unwrap())
    }

    /// Returns a single raw trade offer by its id.
    pub async fn get_tradeoffer_by_id(&self, tradeoffer_id: u64) -> Result<Vec<TradeOffer_Trade>, TradeError> {
        self.get_trade_offers(true, true, true)
            .map_ok(|tradeoffers| tradeoffers.filter_by(|offer| offer.tradeofferid == tradeoffer_id))
            .await
    }

    pub async fn get_new_assetids(&self, tradeid: u64) -> Result<Vec<AssetIDHistory>, TradeError> {
        let found_trade: TradeHistory_Trade = self
            .get_trade_offers_history(None, false)
            .map_ok(|tradeoffers| tradeoffers.filter_by(|trade| trade.tradeid == tradeid))
            .await?
            .swap_remove(0);

        Ok(found_trade
            .every_asset()
            .into_iter()
            .map(|traded_asset| traded_asset.assetids)
            .collect::<Vec<_>>())
    }

    /// Convenience function to auto decline offers received.
    ///
    /// This will help keep the trade offers log clean of the total trade offer limit, if there is one.
    pub async fn decline_received_offers(&self) -> Result<(), TradeError> {
        let mut deny_offers_fut = FuturesOrdered::new();

        let active_received_offers: Vec<TradeOffer_Trade> = self
            .get_trade_offers(true, true, true)
            .map_ok(|tradeoffers| {
                tradeoffers.filter_by(|offer| offer.state == ETradeOfferState::Active && !offer.is_our_offer)
            })
            .await?;

        let total = active_received_offers.len();
        println!("{:#?}", active_received_offers);

        active_received_offers
            .into_iter()
            .map(|x| x.tradeofferid)
            .for_each(|x| {
                deny_offers_fut.push(
                    self.deny_offer(x)
                        .map_ok(|_| tokio::time::delay_for(Duration::from_millis(STANDARD_DELAY))),
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

    /// Creates a new trade offer, and confirms it with mobile authenticator.
    /// Returns the trade offer id on success and if the confirmation was not found but the trade created.
    ///
    /// It makes the assumption that the user has set up their ma file correctly.
    pub async fn create_offer_and_confirm(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        let tradeoffer_id = self.create_offer(tradeoffer).await?;

        tokio::time::delay_for(Duration::from_millis(STANDARD_DELAY)).await;

        let confirmations: Option<Confirmations> = self
            .authenticator
            .fetch_confirmations()
            .inspect_ok(|_| debug!("Confirmations fetched successfully."))
            .await?
            .map(|mut conf: Confirmations| {
                conf.filter_by_trade_offer_ids(&[tradeoffer_id]);
                conf
            });

        // If for some reason we end up not finding the confirmation, return an error
        if confirmations.is_none() {
            return Err(ConfirmationError::NotFoundButTradeCreated(tradeoffer_id).into());
        }

        self.authenticator
            .process_confirmations(ConfirmationMethod::Accept, confirmations.unwrap())
            .await
            .map(|_| tradeoffer_id)
            .map_err(|e| e.into())
    }

    /// Convenience function to create a trade offer.
    /// Returns the trade offer id.
    pub async fn create_offer(&self, tradeoffer: TradeOffer) -> Result<u64, TradeError> {
        self.request(TradeKind::Create(tradeoffer), None)
            .map_ok(|c: TradeOfferCreateResponse| c.tradeofferid)
            .await
    }

    /// Convenience function to accept a single trade offer that was made to this account.
    pub async fn accept_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        self.request(TradeKind::Accept, Some(tradeoffer_id)).await?;
        Ok(())
    }

    /// Convenience function to deny a single trade offer that was made to this account.
    pub async fn deny_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        self.request(TradeKind::Decline, Some(tradeoffer_id)).await?;
        Ok(())
    }

    /// Convenience function to cancel a single trade offer that was created by this account.
    pub async fn cancel_offer(&self, tradeoffer_id: u64) -> Result<(), TradeError> {
        self.request(TradeKind::Cancel, Some(tradeoffer_id)).await?;
        Ok(())
    }

    #[allow(dead_code)]
    /// Calling the API is more efficient.
    async fn deny_offer_api() {}

    #[allow(dead_code)]
    /// Calling the API is more efficient.
    async fn cancel_offer_api() {}

    /// Check current session health, injects SessionID cookie, and send the request.
    async fn request<T: DeserializeOwned>(
        &self,
        operation: TradeKind,
        tradeoffer_id: Option<u64>,
    ) -> Result<T, TradeError> {
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

        let mut request: Box<dyn HasSessionID> = match operation {
            TradeKind::Accept => {
                let partner_id = self
                    .get_tradeoffer_by_id(tradeoffer_id.unwrap())
                    .await?
                    .first()
                    .map(|c| SteamID::from_steam3(c.tradeofferid as u32, None, None))
                    .map(|steamid| steamid.to_steam64())
                    .unwrap();

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

            TradeKind::Cancel | TradeKind::Decline => Box::new(TradeOfferGenericRequest::default()),
            TradeKind::Create(offer) => Box::new(Self::prepare_offer(offer)?),
        };

        // TODO: Check if session is ok, then inject cookie
        let session_id_cookie = self
            .authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "sessionid")
            .ok_or_else(|| {
                PayloadError("Somehow you don't have a sessionid cookie. You need to login first.".to_string())
            })?;

        request.set_sessionid(session_id_cookie);

        let response_text = self
            .authenticator
            .request_custom_endpoint(tradeoffer_endpoint, Method::POST, header, Some(request))
            .and_then(|response| response.text())
            .inspect_ok(|resp_text: &String| debug!("{}", resp_text))
            .await?;

        match serde_json::from_str::<T>(&response_text) {
            Ok(response) => Ok(response),
            Err(_) => {
                // try to match into a generic message
                if let Ok(resp) = serde_json::from_str::<TradeOfferGenericErrorResponse>(&response_text) {
                    // FIXME: There is also the "strError" that returns an eResult inside ().
                    Err(tradeoffer_error_from_eresult(resp.eresult).into())
                } else {
                    Err(TradeOfferError::GeneralFailure("Something went terribly wrong.".to_string()).into())
                }
            }
        }
    }

    /// Checks that the tradeoffer is valid, and process it, getting the trade token and steamid3, into a `TradeOfferCreateRequest`,
    /// ready to send it.
    fn prepare_offer(tradeoffer: TradeOffer) -> Result<TradeOfferCreateRequest, TradeError> {
        TradeOffer::validate(&tradeoffer.my_assets, &tradeoffer.their_assets)?;

        let (steamid3, trade_token) = TradeOffer::parse_url(&tradeoffer.their_trade_url)?;

        let their_steamid64 = SteamID::from_steam3((&*steamid3).parse().unwrap(), None, None).to_steam64();
        let trade_offer_params = trade_token.map(|token| TradeOfferParams {
            trade_offer_access_token: token,
        });

        Ok(TradeOfferCreateRequest::new(
            their_steamid64,
            tradeoffer,
            trade_offer_params,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::time::estimate_tradelock_end;

    use super::*;

    fn get_tradeoffer_url_with_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn"
    }

    fn get_tradeoffer_url_without_token() -> &'static str {
        "https://steamcommunity.com/tradeoffer/new/?partner=79925588"
    }

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
    fn tradeoffer_url() {
        let parsed = TradeOffer::parse_url(get_tradeoffer_url_with_token()).unwrap();
        assert_eq!((String::from("79925588"), Some(String::from("Ob27qXzn"))), parsed)
    }

    #[test]
    fn new_assets() {
        let raw_response = sample_trade_history_response();
        let filtered = raw_response.filter_by(|x| x.tradeid == 3622543526924228084).remove(0);
        let asset = filtered.every_asset().remove(0);
        assert_eq!(asset.assetids.old_assetid, 15319724006);
        assert_eq!(asset.assetids.new_assetid, 19793871926);
    }

    #[cfg(feature = "time")]
    #[test]
    fn estimate_time() {
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

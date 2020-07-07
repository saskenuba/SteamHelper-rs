//! Steam trade manager is the module that allows you to automate trade offers.
//!
//! It inherently needs SteamAuthenticator as a dependency, since we need cookies from Steam
//! Community and Steam Store to be able to create and accept those, along with mobile
//! confirmations.

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

use const_concat::const_concat;
use reqwest::Url;

use steam_auth::{client::SteamAuthenticator, HeaderMap, Method, STEAM_COMMUNITY_HOST};
use steamid_parser::SteamID;
pub use types::{asset_collection::AssetCollection, trade_offer::TradeOffer, TradeKind};

use crate::{
    errors::{TradeOfferError, TradeOfferError::PayloadError},
    types::{
        sessionid::{HasSessionID, SessionID},
        trade_api::CEcon_GetTradeOffers_Response_Base,
        trade_offer_web::{
            JsonTradeOffer, TradeOfferAcceptRequest, TradeOfferCreateRequest,
            TradeOfferGenericParameters, TradeOfferGenericRequest, TradeOfferParams,
        },
    },
};
use tracing::{debug, info};

mod errors;
mod types;

const TRADEOFFER_BASE: &str = "https://steamcommunity.com/tradeoffer/";
const TRADEOFFER_NEW_URL: &str = const_concat!(TRADEOFFER_BASE, "new/send");

/// This is decided upon various factors, mainly stability of Steam servers when dealing with huge
/// trade offers.
///
/// Consider this when creating trade websites.
const TRADE_MAX_ITEMS: u8 = u8::max_value();

/// Limit introduced by Valve
const TRADE_MAX_TRADES_PER_ACCOUNT: u8 = 5;

#[derive(Debug)]
pub struct SteamTradeManager<'a> {
    authenticator: &'a SteamAuthenticator,
}

impl<'a> SteamTradeManager<'a> {
    pub fn new(authenticator: &'a SteamAuthenticator) -> SteamTradeManager<'a> {
        Self {
            authenticator: &authenticator,
        }
    }

    /// Parses and validates the TradeOffer, and send it to Steam.
    /// Returns the trade offer id, or an error.
    pub async fn send_tradeoffer(&self, tradeoffer: TradeOffer) -> Result<String, TradeOfferError> {
        Self::validate_tradeoffer(&tradeoffer.my_assets, &tradeoffer.their_assets)?;

        let mut request = self.prepare_tradeoffer(&tradeoffer.url, &tradeoffer.message)?;

        // Convert Option<AssetCollection> to an AssetList
        request.json_tradeoffer = JsonTradeOffer {
            my_account: tradeoffer
                .my_assets
                .or_else(|| Some(AssetCollection::default()))
                .map(|a| a.dump_to_asset_list())
                .unwrap(),
            their_account: tradeoffer
                .their_assets
                .or_else(|| Some(AssetCollection::default()))
                .map(|a| a.dump_to_asset_list())
                .unwrap(),
            ..Default::default()
        };

        let mut header = HeaderMap::new();
        header.insert("Referer", TRADEOFFER_REFERER_URL.parse().unwrap());

        let response: TradeOfferResponse = self
            .authenticator
            .request_custom_endpoint(
                TRADEOFFER_URL.to_string(),
                Method::POST,
                Some(header),
                Some(request),
            )
            .and_then(|c| c.json::<TradeOfferResponse>())
            .await?;

        Ok(response.tradeofferid)
    }

    /// Placeholder
    ///
    /// Ex: https://steamcommunity.com/tradeoffer/new/?partner=79925588&token=Ob27qXzn
    fn prepare_tradeoffer<'b>(
        &self,
        tradeoffer_url: &str,
        message: &'b str,
    ) -> Result<TradeOfferRequest<'b>, TradeOfferError> {
        let (steamid3, trade_token) = Self::parse_tradeoffer_url(tradeoffer_url).unwrap();

        // FIXME: SteamID should be +1
        let their_steamid =
            SteamID::from_steam3((&*steamid3).parse().unwrap(), None, None).to_steam64();
        let trade_offer_params = trade_token.map(|token| TradeOfferParams {
            trade_offer_access_token: token,
        });

        // FIXME: If we get the sessionid before checking if the session is still up, we run into a
        // risk of sending an invalid sessionid cookie.

        let session_id_cookie = self
            .authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "sessionid")
            .ok_or_else(|| {
                PayloadError(
                    "Somehow you don't have a sessionid cookie. You need to login first."
                        .to_string(),
                )
            })?;

        Ok(TradeOfferRequest {
            sessionid: session_id_cookie,
            their_steamid,
            message,
            trade_offer_create_params: trade_offer_params,
            ..Default::default()
        })
    }

    fn validate_tradeoffer(
        my_items: &Option<AssetCollection>,
        their_items: &Option<AssetCollection>,
    ) -> Result<(), TradeOfferError> {
        if my_items.is_none() && their_items.is_none() {
            return Err(TradeOfferError::InvalidTrade(
                "There can't be a trade offer with no items being traded.".to_string(),
            ));
        }

        Ok(())
    }

    fn parse_tradeoffer_url(url: &str) -> Result<(String, Option<String>), TradeOfferError> {
        let parsed_url = Url::parse(url).unwrap();

        // Partner ID is mandatory
        let steam_id3 = parsed_url
            .query_pairs()
            .find(|(param, arg)| param == "partner")
            .ok_or_else(|| TradeOfferError::InvalidTradeOfferUrl)?
            .1
            .to_string();

        // If the recipient is your friend, you don't need a token
        let trade_token = parsed_url
            .query_pairs()
            .find(|(param, arg)| param == "token")
            .map(|(_, c)| c.to_string());

        Ok((steam_id3, trade_token))
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

    #[test]
    fn tradeoffer_url() {
        let parsed =
            SteamTradeManager::parse_tradeoffer_url(get_tradeoffer_url_with_token()).unwrap();
        assert_eq!(
            (String::from("79925588"), Some(String::from("Ob27qXzn"))),
            parsed
        )
    }
}

use serde::{Deserialize, Serialize};

use steam_language_gen::generated::enums::EResult;

use crate::types::sessionid::{HasSessionID, SessionID};
use crate::{AssetCollection, TradeOffer};

macro_rules! impl_sessionid {
    ($name:ident) => {
        impl HasSessionID for $name {
            fn set_sessionid(&mut self, sessionid: String) {
                self.sessionid.sessionid = sessionid;
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferCommonParameters {
    pub serverid: i32,
    #[serde(rename = "partner")]
    /// Recipient STEAMID64. Ex: 76561198040191316
    pub their_steamid: u64,
    pub captcha: String,
}

impl<'a> Default for TradeOfferCommonParameters {
    fn default() -> Self {
        Self {
            serverid: 1,
            their_steamid: 0,
            captcha: "".parse().unwrap(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Used for decline, and cancelling an offer.
/// Url: https://steamcommunity.com/tradeoffer/4127395150/accept
pub(crate) struct TradeOfferGenericRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct TradeOfferGenericErrorResponse {
    // #[serde(with = "serde_with::rust::display_fromstr")]
    // pub tradeofferid: Option<u64>,
    #[serde(rename = "success")]
    pub eresult: EResult,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Url: https://steamcommunity.com/tradeoffer/4127395150/accept
pub(crate) struct TradeOfferAcceptRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
    #[serde(flatten)]
    pub common: TradeOfferCommonParameters,
    pub tradeofferid: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Response after the creation of a new trade offer.
///
/// There is no need of confirmations if not trading items from self account.
pub struct TradeOfferCreateResponse {
    /// This is the trade offer ID of our offer. We can use this to mobile confirm.
    /// Ex: 4112828817
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeofferid: u64,
    pub needs_mobile_confirmation: Option<bool>,
    pub needs_email_confirmation: Option<bool>,
    pub email_domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// We create a trade offer from a Steam Trade link the user shares with us.
/// The "partner" number, is the SteamID3. In order to send the trade offer, first we need to to
/// convert it to a SteamID64.
pub(crate) struct TradeOfferCreateRequest {
    /// Session ID cookie from Steam Community.
    #[serde(flatten)]
    pub sessionid: SessionID,
    #[serde(flatten)]
    pub common: TradeOfferCommonParameters,
    #[serde(rename = "tradeoffermessage")]
    /// Message to be sent to trade offer recipient along with the trade.
    /// The message needs to be form url encoded.
    pub message: String,
    #[serde(serialize_with = "serde_with::json::nested::serialize")]
    pub json_tradeoffer: JsonTradeOffer,
    #[serde(serialize_with = "serde_with::json::nested::serialize")]
    /// If we intend to create a trade offer based on a trade partner link, we need to send the
    /// trade access token with it.
    pub trade_offer_create_params: Option<TradeOfferParams>,
}

impl TradeOfferCreateRequest {
    pub(crate) fn new<T: Into<Option<TradeOfferParams>>>(
        their_steamid64: u64,
        tradeoffer: TradeOffer,
        trade_token: T,
    ) -> TradeOfferCreateRequest {
        Self {
            sessionid: Default::default(),
            common: TradeOfferCommonParameters {
                their_steamid: their_steamid64,
                ..Default::default()
            },
            message: tradeoffer.message.clone(),
            json_tradeoffer: tradeoffer.into(),
            trade_offer_create_params: trade_token.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferParams {
    /// A trade offer link has an unique token that the user can invalidate at any time.
    /// We need to insert this token correct at the request.
    pub trade_offer_access_token: String,
}

impl Default for TradeOfferCreateRequest {
    fn default() -> Self {
        Self {
            message: "".to_string(),
            json_tradeoffer: Default::default(),
            trade_offer_create_params: None,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Trade offer format to be sent through Steam.
pub(crate) struct JsonTradeOffer {
    pub newversion: bool,
    pub version: i32,
    #[serde(rename = "me")]
    pub my_account: AssetList,
    #[serde(rename = "them")]
    pub their_account: AssetList,
}

impl From<TradeOffer> for JsonTradeOffer {
    fn from(tradeoffer: TradeOffer) -> Self {
        let my_account = tradeoffer
            .my_assets
            .unwrap_or_else(|| AssetCollection::default())
            .dump_to_asset_list();

        let their_account = tradeoffer
            .their_assets
            .unwrap_or_else(|| AssetCollection::default())
            .dump_to_asset_list();

        Self {
            my_account,
            their_account,
            ..Default::default()
        }
    }
}

impl Default for JsonTradeOffer {
    fn default() -> Self {
        Self {
            newversion: true,
            version: 2,
            my_account: Default::default(),
            their_account: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// The correct format for assets inside the trade offer.
pub(crate) struct AssetList {
    pub assets: Vec<Asset>,
    pub currency: Vec<String>,
    pub ready: bool,
}

impl Default for AssetList {
    fn default() -> Self {
        Self {
            assets: vec![],
            currency: vec![],
            ready: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// An unique item at this particular time, of any kind.
pub struct Asset {
    /// Game APPID.
    pub appid: u32,
    /// Inventory ContextID. A game may have one or more.
    // u32
    pub contextid: String,
    /// Amount if the item is stackable.
    pub(crate) amount: i64,
    // u64
    pub assetid: String,
}

impl_sessionid!(TradeOfferGenericRequest);
impl_sessionid!(TradeOfferAcceptRequest);
impl_sessionid!(TradeOfferCreateRequest);

#[cfg(test)]
mod tests {
    use super::*;

    fn get_offer() -> JsonTradeOffer {
        let json_request = r#"{
  "newversion":true,
  "version":4,
  "me":{
    "assets":[
      {
        "appid":730,
        "contextid":"2",
        "amount":1,
        "assetid":"17034419698"
      },
      {
        "appid":730,
        "contextid":"2",
        "amount":1,
        "assetid":"16889698077"
      }
    ],
    "currency":[],
    "ready":false
  },
  "them":{
    "assets":[
      {
        "appid":730,
        "contextid":"2",
        "amount":1,
        "assetid":"18116227588"
      }
    ],
    "currency":[],
    "ready":false
  }
}"#;
        serde_json::from_str::<JsonTradeOffer>(json_request).unwrap()
    }
}

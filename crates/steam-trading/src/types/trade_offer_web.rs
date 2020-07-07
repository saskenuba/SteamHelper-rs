use crate::types::sessionid::{HasSessionID, SessionID};
use serde::{Deserialize, Serialize};

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
pub(crate) struct TradeOfferGenericParameters {
    pub serverid: i32,
    #[serde(rename = "partner")]
    /// Recipient STEAMID64. Ex: 76561198040191316
    pub their_steamid: u64,
    pub captcha: String,
}

impl<'a> Default for TradeOfferGenericParameters {
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Url: https://steamcommunity.com/tradeoffer/4127395150/accept
pub(crate) struct TradeOfferAcceptRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
    #[serde(flatten)]
    pub common: TradeOfferGenericParameters,
    pub tradeofferid: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Response after the creation of a new trade offer.
///
/// There is no need of confirmations if not trading items from self account.
pub struct TradeOfferCreateResponse {
    /// This is the trade offer ID of our offer. We can use this to mobile confirm.
    /// Ex: 4112828817
    pub tradeofferid: String,
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
    pub common: TradeOfferGenericParameters,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferParams {
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
pub struct JsonTradeOffer {
    pub newversion: bool,
    pub version: i32,
    #[serde(rename = "me")]
    pub my_account: AssetList,
    #[serde(rename = "them")]
    pub their_account: AssetList,
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
pub struct AssetList {
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
    pub amount: i64,
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
        serde_json::from_str::<JsonTradeOffer>(
            r#"{
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
}"#,
        )
        .unwrap()
    }

    #[test]
    fn trade_offer_serialize() {
        let json_trade_offer = get_offer();
        let new_trade_offer = TradeOfferCreateRequest {
            message: "5+e+onibus".to_string(),
            json_tradeoffer: json_trade_offer,
            trade_offer_create_params: None,
            common: Default::default(),
            ..Default::default()
        };

        println!(
            "{:#?}",
            serde_json::to_string_pretty(&new_trade_offer).unwrap()
        );
    }
}

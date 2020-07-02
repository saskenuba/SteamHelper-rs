use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Response after the creation of a new trade offer.
///
/// There is no need of confirmations if not trading items from self account.
pub struct TradeOfferResponse {
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
pub(crate) struct TradeOfferRequest<'a> {
    /// Session ID cookie from Steam Community.
    pub sessionid: String,
    pub serverid: i32,
    #[serde(rename = "partner")]
    /// Recipient STEAMID64. Ex: 76561198040191316
    pub their_steamid: u64,
    #[serde(rename = "tradeoffermessage")]
    /// Message to be sent to trade offer recipient along with the trade.
    /// The message needs to be form url encoded.
    pub message: &'a str,
    #[serde(serialize_with = "serde_with::json::nested::serialize")]
    pub json_tradeoffer: JsonTradeOffer,
    pub captcha: &'a str,
    #[serde(serialize_with = "serde_with::json::nested::serialize")]
    /// If we intend to create a trade offer based on a trade partner link, we need to send the
    /// trade access token with it.
    pub trade_offer_create_params: Option<TradeOfferParams>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferParams {
    pub trade_offer_access_token: String,
}

impl<'a> Default for TradeOfferRequest<'a> {
    fn default() -> Self {
        Self {
            sessionid: "".to_string(),
            serverid: 1,
            their_steamid: 0,
            message: "",
            json_tradeoffer: Default::default(),
            captcha: "",
            trade_offer_create_params: None,
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
        let new_trade_offer = TradeOfferRequest {
            sessionid: "1123safsdasd".to_string(),
            serverid: 1,
            their_steamid: 78922817233441,
            message: "5+e+onibus",
            json_tradeoffer: json_trade_offer,
            captcha: "",
            trade_offer_create_params: None,
        };

        println!(
            "{:#?}",
            serde_json::to_string_pretty(&new_trade_offer).unwrap()
        );
    }
}

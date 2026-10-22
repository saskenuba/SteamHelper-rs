use serde::Deserialize;
use serde::Serialize;
use steam_language::enums::EResult;

use crate::types::sessionid::HasSessionID;
use crate::types::sessionid::SessionID;
use crate::Asset;
use crate::OfferRequest;

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

/// Used for decline, and cancelling an offer.
/// Url: `https://steamcommunity.com/tradeoffer/4127395150/accept`
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferGenericRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
}

/// `strError` is checked before, see `response::check_response`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct TradeOfferGenericErrorResponse {
    #[serde(rename = "success")]
    pub eresult: Option<EResult>,
}

/// Url: `https://steamcommunity.com/trade/new/acknowledge`
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct TradeProtectionAcknowledgeRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
    pub message: u8,
}

impl Default for TradeProtectionAcknowledgeRequest {
    fn default() -> Self {
        Self {
            sessionid: Default::default(),
            message: 1,
        }
    }
}

/// Url: https://steamcommunity.com/tradeoffer/4127395150/accept
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferAcceptRequest {
    #[serde(flatten)]
    pub sessionid: SessionID,
    #[serde(flatten)]
    pub common: TradeOfferCommonParameters,
    pub tradeofferid: u64,
}

/// Response after the creation of a new trade offer.
///
/// There is no need of confirmations if not trading items from self account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeOfferCreateResponse {
    /// This is the trade offer ID of our offer. We can use this to mobile confirm.
    /// Ex: 4112828817
    pub tradeofferid: Option<String>,
    pub needs_mobile_confirmation: Option<bool>,
    pub needs_email_confirmation: Option<bool>,
    pub email_domain: Option<String>,
    /// The trade an accepted offer became, in the answer to an accept that needed no confirmation.
    /// Ex: 695401084301346863
    pub tradeid: Option<String>,
}

/// Response after we cancel an trade offer we've sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeOfferCancelResponse {
    pub tradeofferid: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The form of `offer`: to the partner and with the token of its trade link.
    pub(crate) fn new(offer: &OfferRequest) -> TradeOfferCreateRequest {
        let partner = offer.partner();
        Self {
            sessionid: Default::default(),
            common: TradeOfferCommonParameters {
                their_steamid: partner.partner().to_steam64(),
                ..Default::default()
            },
            message: offer.message_text().to_string(),
            json_tradeoffer: JsonTradeOffer::new(offer),
            trade_offer_create_params: Some(TradeOfferParams {
                trade_offer_access_token: partner.token().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TradeOfferParams {
    /// A trade offer link has an unique token that the user can invalidate at any time.
    /// We need to insert this token correct at the request.
    pub trade_offer_access_token: String,
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

impl JsonTradeOffer {
    fn new(offer: &OfferRequest) -> Self {
        Self {
            my_account: AssetList::new(offer.gives()),
            their_account: AssetList::new(offer.takes()),
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
    pub assets: Vec<JsonAsset>,
    pub currency: Vec<String>,
    pub ready: bool,
}

impl AssetList {
    fn new(items: &[(Asset, u32)]) -> Self {
        Self {
            assets: items
                .iter()
                .map(|&(asset, amount)| JsonAsset::new(asset, amount))
                .collect(),
            ..Default::default()
        }
    }
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
/// An item of an offer, as the form sends it: ids as strings, the amount as a number.
pub(crate) struct JsonAsset {
    /// Game APPID.
    pub appid: u32,
    /// Inventory ContextID. A game may have one or more.
    pub contextid: String,
    /// Amount if the item is stackable.
    pub amount: u32,
    pub assetid: String,
}

impl JsonAsset {
    fn new(asset: Asset, amount: u32) -> Self {
        Self {
            appid: asset.context.appid(),
            contextid: asset.context.contextid().to_string(),
            amount,
            assetid: asset.id.to_string(),
        }
    }
}

impl_sessionid!(TradeOfferGenericRequest);
impl_sessionid!(TradeOfferAcceptRequest);
impl_sessionid!(TradeOfferCreateRequest);
impl_sessionid!(TradeProtectionAcknowledgeRequest);

//! Types returned by Steam trade operations.
//!
//! The models described here can be found at:
//! https://developer.valvesoftware.com/wiki/Steam_Web_API/IEconService

use serde::Deserialize;
use serde_repr::Deserialize_repr;

use steam_language_gen::generated::enums::{ETradeOfferConfirmationMethod, ETradeOfferState};

pub trait HasAssets {
    fn filter_by<T: Fn(&&CEcon_TradeOffer) -> bool>(self, by: T) -> Vec<CEcon_TradeOffer>;
}

impl HasAssets for CEcon_GetTradeOffers_Response_Base {
    fn filter_by<T: Fn(&&CEcon_TradeOffer) -> bool>(self, by: T) -> Vec<CEcon_TradeOffer> {

        match (
            self.response.trade_offers_sent,
            self.response.trade_offers_received,
        ) {
            (Some(sent), Some(received)) => {
                let mut all_tradeoffers = vec![];
                all_tradeoffers.extend(sent.iter());
                all_tradeoffers.extend(received.iter());
                all_tradeoffers
                    .into_iter()
                    .filter(|c| by(c))
                    .map(|c: &CEcon_TradeOffer| c.to_owned())
                    .collect::<Vec<CEcon_TradeOffer>>()
            }
            (None, Some(trades)) | (Some(trades), None) => trades
                .iter()
                .filter(|c| by(c))
                .map(|c: &CEcon_TradeOffer| c.to_owned())
                .collect::<Vec<CEcon_TradeOffer>>(),
            _ => unreachable!(),
        }
    }
}

// impl HasAssets for CEcon_GetTradeHistory_Response_Trade_Base {
//     fn filter_tradeoffer_id(&self) {
//         unimplemented!()
//     }
// }

/// Tracks the status of a trade after a trade offer has been accepted.
/// Received at GetTradeHistory at status field on
/// CEcon_GetTradeHistory_Response_Trade
#[derive(Debug, Clone, PartialEq, Eq, Deserialize_repr)]
#[repr(i32)]
pub enum ETradeStatus {
    /// Trade has just been accepted/confirmed, but no work has been done yet
    Init = 0,
    /// Steam is about to start committing the trade
    PreCommitted = 1,
    /// The items have been exchanged
    Committed = 2,
    /// All work is finished
    Complete = 3,
    /// Something went wrong after Init, but before Committed, and the trade has been rolled back
    Failed = 4,
    /// A support person rolled back the trade for one side
    PartialSupportRollback = 5,
    /// A support person rolled back the trade for both sides
    FullSupportRollback = 6,
    /// A support person rolled back the trade for some set of items
    SupportRollbackSelective = 7,
    /// We tried to roll back the trade when it failed, but haven't managed to do that for all
    /// items yet
    RollbackFailed = 8,
    /// We tried to roll back the trade, but some failure didn't go away and we gave up */
    RollbackAbandoned = 9,
    /// Trade is in escrow
    InEscrow = 10,
    /// A trade in escrow was rolled back
    EscrowRollback = 11,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Descriptions {
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub appid: u32,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub marketable: bool,
    pub tradable: String,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct CEcon_GetTradeOffers_Response_Base {
    response: CEcon_GetTradeOffers_Response,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
pub struct CEcon_GetTradeOffers_Response {
    pub trade_offers_sent: Option<Vec<CEcon_TradeOffer>>,
    pub trade_offers_received: Option<Vec<CEcon_TradeOffer>>,
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug, Clone)]
/// Represents a steam trade offer.
/// Returned by GetTradeOffers (vector) and GetTradeOffer.
pub struct CEcon_TradeOffer {
    /// Unique ID generated when a trade offer is created
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tradeofferid: u64,
    /// SteamID3
    pub accountid_other: u64,
    /// Message included by the creator of the trade offer
    message: String,
    /// Unix time when the offer will expire (or expired, if it is in the past)
    /// TODO: Maybe convert?
    expiration_time: u64,
    /// State of trade offer
    trade_offer_state: ETradeOfferState,
    items_to_give: Option<Vec<CEcon_Asset>>,
    items_to_receive: Option<Vec<CEcon_Asset>>,
    /// Indicates the account binded with the api key requested this trade
    is_our_offer: bool,
    time_created: u64,
    time_updated: u64,
    /// Tradeid is the historical number of the trade. It is used, for example to find the new
    /// generated asset ids after the trade is complete.
    /// Shows up only for completed trades.
    tradeid: Option<String>,
    from_real_time_trade: bool,
    escrow_end_date: u8,
    confirmation_method: ETradeOfferConfirmationMethod,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_Asset {
    #[serde(flatten)]
    pub asset: BasicAssetParameters,
    pub missing: bool,
    pub est_usd: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_GetTradeHistory_Response_Trade_Base {
    #[serde(flatten)]
    pub response: Response,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Response {
    pub more: bool,
    pub trades: Vec<CEcon_GetTradeHistory_Response_Trade>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_GetTradeHistory_Response_Trade {
    pub tradeid: String,
    pub steamid_other: String,
    pub time_init: i64,
    pub status: ETradeStatus,
    pub assets_received: Option<Vec<CEcon_GetTradeHistory_Response_Trade_TradedAsset>>,
    pub assets_given: Option<Vec<CEcon_GetTradeHistory_Response_Trade_TradedAsset>>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CEcon_GetTradeHistory_Response_Trade_TradedAsset {
    #[serde(flatten)]
    pub asset: BasicAssetParameters,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub new_assetid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub new_contextid: u64,
    pub rollback_new_assetid: Option<String>,
    pub rollback_new_contextid: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BasicAssetParameters {
    pub appid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub contextid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub assetid: u64,

    // #[serde(with = "serde_with::rust::display_fromstr")]
    // pub currencyid: Option<u64>,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub classid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub instanceid: u64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub amount: u64,
}

impl CEcon_GetTradeHistory_Response_Trade_TradedAsset {
    fn get_old_new_assetids(&self) -> (u64, u64) {
        (self.asset.assetid, self.new_assetid)
    }
}

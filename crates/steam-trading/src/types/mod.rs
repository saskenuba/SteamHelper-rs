use crate::{TradeOffer, TRADEOFFER_BASE, TRADEOFFER_NEW_URL};

pub mod asset_collection;
pub mod asset_history;
pub mod sessionid;
pub(crate) mod trade_api;
pub mod trade_offer;
pub(super) mod trade_offer_web;
pub mod trade_operation;

#[derive(Debug)]
pub enum TradeKind {
    Accept,
    Cancel,
    Create(TradeOffer),
    Decline,
}

impl TradeKind {
    pub fn endpoint(&self, tradeofferid: Option<u64>) -> String {
        if let TradeKind::Create(_) = self {
            return TRADEOFFER_NEW_URL.to_string();
        }

        let tradeofferid = tradeofferid.unwrap();
        match self {
            TradeKind::Accept => format!("{}{}/accept", TRADEOFFER_BASE, tradeofferid),
            TradeKind::Cancel => format!("{}{}/cancel", TRADEOFFER_BASE, tradeofferid),
            TradeKind::Decline => format!("{}{}/decline", TRADEOFFER_BASE, tradeofferid),
            _ => unreachable!(),
        }
    }
}

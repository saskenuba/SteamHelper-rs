use crate::{TradeOffer, TRADEOFFER_BASE, TRADEOFFER_NEW_URL};

pub mod asset_collection;
pub mod sessionid;
pub mod trade_link;
pub mod trade_offer;
pub mod trade_offer_web;

#[derive(Debug, PartialEq)]
pub enum TradeKind {
    Accept,
    Cancel,
    Create(TradeOffer),
    Decline,
}

impl TradeKind {
    pub fn endpoint(&self, tradeofferid: Option<i64>) -> String {
        if let TradeKind::Create(_) = self {
            return TRADEOFFER_NEW_URL.to_string();
        }

        let tradeofferid = tradeofferid.unwrap();
        let url_path = match self {
            Self::Accept => "/accept",
            Self::Cancel => "/cancel",
            Self::Decline => "/decline",
            _ => unreachable!(),
        };
        TRADEOFFER_BASE.to_owned() + &*tradeofferid.to_string() + url_path
    }
}

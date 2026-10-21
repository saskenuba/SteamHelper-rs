use tappet::response_types::GetTradeHistoryResponse;
use tappet::response_types::GetTradeOfferResponse;
use tappet::response_types::GetTradeOffersResponse;
use tappet::response_types::OfferItem;
use tappet::response_types::TradeHistory_Trade;
use tappet::response_types::TradeHistory_TradedAsset;
use tappet::response_types::TradeOffer_Trade;

pub trait HasAssets {
    type Asset;
    fn every_asset(self) -> Vec<Self::Asset>;
}

pub trait FilterBy<C> {
    fn filter_by<T: Fn(&C) -> bool>(self, filter_fn: T) -> Vec<C>;

    fn partition_by<T: Fn(&C) -> bool>(self, filter_fn: T) -> (Vec<C>, Vec<C>);
}

/// Offers that could not be read (`response.malformed_offers_sent` / `malformed_offers_received`) are left out: check
/// `response.has_malformed_offers()` first. Offers whose items can't be trusted are passed to `filter_fn` like any
/// other; check `offer.has_malformed_items()` there before judging their items.
impl FilterBy<TradeOffer_Trade> for GetTradeOffersResponse {
    /// Filters every trade, both sent and received, by `filter_fn`.
    fn filter_by<T: Fn(&TradeOffer_Trade) -> bool>(self, filter_fn: T) -> Vec<TradeOffer_Trade> {
        self.partition_by(filter_fn).0
    }

    /// Partition trades, both sent and received, by `filter_fn`.
    fn partition_by<T: Fn(&TradeOffer_Trade) -> bool>(
        self,
        filter_fn: T,
    ) -> (Vec<TradeOffer_Trade>, Vec<TradeOffer_Trade>) {
        let trades_sent = self.response.trade_offers_sent;
        let trades_received = self.response.trade_offers_received;

        match (trades_sent, trades_received) {
            (Some(sent), Some(received)) => sent.into_iter().chain(received.into_iter()).partition(|c| filter_fn(c)),
            (None, Some(trades)) | (Some(trades), None) => trades.into_iter().partition(|c| filter_fn(c)),
            _ => (vec![], vec![]),
        }
    }
}

impl HasAssets for GetTradeOffersResponse {
    type Asset = OfferItem;

    /// Returns every item of every trade, sent and received, given and received. Malformed items are kept: see
    /// [OfferItem]. An offer without items adds nothing, and the items of offers that could not be read are not
    /// here: check `response.has_malformed_offers()` and each offer's `has_malformed_items()`.
    fn every_asset(self) -> Vec<OfferItem> {
        let trades_sent = self.response.trade_offers_sent.unwrap_or_default();
        let trades_received = self.response.trade_offers_received.unwrap_or_default();

        trades_sent
            .into_iter()
            .chain(trades_received)
            .flat_map(|offer| offer.items_to_give.into_iter().chain(offer.items_to_receive))
            .collect()
    }
}

impl FilterBy<TradeHistory_Trade> for GetTradeHistoryResponse {
    /// Filter trades for the whole `CEcon_GetTradeHistory_Response_Trade_Base`.
    fn filter_by<T: Fn(&TradeHistory_Trade) -> bool>(self, filter_fn: T) -> Vec<TradeHistory_Trade> {
        self.response.trades.into_iter().filter(|x| filter_fn(x)).collect()
    }

    fn partition_by<T: Fn(&TradeHistory_Trade) -> bool>(
        self,
        filter_fn: T,
    ) -> (Vec<TradeHistory_Trade>, Vec<TradeHistory_Trade>) {
        self.response.trades.into_iter().partition(|x| filter_fn(x))
    }
}

impl HasAssets for GetTradeHistoryResponse {
    type Asset = TradeHistory_TradedAsset;

    /// Returns every asset given or received for the `GetTradeHistory`, also known as
    /// `CEcon_GetTradeHistory_Response_Trade_Base`.
    fn every_asset(self) -> Vec<TradeHistory_TradedAsset> {
        let mut traded_assets = vec![];

        self.response
            .trades
            .into_iter()
            .for_each(|trade| traded_assets.push(trade.every_asset()));

        traded_assets.into_iter().flatten().collect()
    }
}

impl FilterBy<TradeHistory_TradedAsset> for TradeHistory_Trade {
    /// Filter a trade by an asset condition
    fn filter_by<T: Fn(&TradeHistory_TradedAsset) -> bool>(self, filter_fn: T) -> Vec<TradeHistory_TradedAsset> {
        self.every_asset().into_iter().filter(|x| filter_fn(x)).collect()
    }

    fn partition_by<T: Fn(&TradeHistory_TradedAsset) -> bool>(
        self,
        filter_fn: T,
    ) -> (Vec<TradeHistory_TradedAsset>, Vec<TradeHistory_TradedAsset>) {
        self.every_asset().into_iter().partition(|x| filter_fn(x))
    }
}

impl HasAssets for TradeHistory_Trade {
    type Asset = TradeHistory_TradedAsset;

    /// Returns every asset that a trade has. Given or Received.
    fn every_asset(self) -> Vec<TradeHistory_TradedAsset> {
        let mut traded_assets = vec![];

        let given = self.assets_given;
        let received = self.assets_received;

        if let Some(asset) = given {
            traded_assets.push(asset);
        }
        if let Some(asset) = received {
            traded_assets.push(asset);
        }

        traded_assets.into_iter().flatten().collect()
    }
}

impl FilterBy<TradeOffer_Trade> for GetTradeOfferResponse {
    fn filter_by<T: Fn(&TradeOffer_Trade) -> bool>(self, filter_fn: T) -> Vec<TradeOffer_Trade> {
        let offer = &self.response.offer;

        if filter_fn(offer) {
            return vec![self.response.offer];
        }
        vec![]
    }

    fn partition_by<T: Fn(&TradeOffer_Trade) -> bool>(
        self,
        filter_fn: T,
    ) -> (Vec<TradeOffer_Trade>, Vec<TradeOffer_Trade>) {
        let offer = self.response.offer;

        if filter_fn(&offer) {
            return (vec![offer], vec![]);
        }
        (vec![], vec![offer])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offers() -> GetTradeOffersResponse {
        serde_json::from_str(
            r#"{"response":{
              "trade_offers_sent":[{"tradeofferid":"1","accountid_other":2,"trade_offer_state":2,
                "items_to_give":[{"appid":730,"contextid":"2","assetid":"10","classid":"1","instanceid":"0","amount":"1"}]}],
              "trade_offers_received":[{"tradeofferid":"3","accountid_other":4,"trade_offer_state":2,
                "items_to_give":[{"appid":730,"contextid":"2","assetid":"11","classid":"1","instanceid":"0","amount":"1"}],
                "items_to_receive":[{"appid":730,"contextid":"2","assetid":"12","classid":"1","instanceid":"0","amount":"1"},
                                    {"appid":730,"contextid":"2"}]}]
            }}"#,
        )
        .unwrap()
    }

    #[test]
    fn every_asset_of_trade_offers() {
        let items = offers().every_asset();
        let assetids: Vec<Option<i64>> = items.iter().map(|item| item.asset().map(|a| a.assetid)).collect();
        assert_eq!(assetids, vec![Some(10), Some(11), Some(12), None]);
    }

    #[test]
    fn partition_trade_offers() {
        let (matching, rest) = offers().partition_by(|offer| offer.tradeofferid == 3);
        assert_eq!(matching.len(), 1);
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].tradeofferid, 1);
    }
}

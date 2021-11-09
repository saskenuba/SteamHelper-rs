use steam_web_api::response_types::{
    CEcon_Asset, GetTradeHistoryResponse, GetTradeOfferResponse, GetTradeOffersResponse, TradeHistory_Trade,
    TradeHistory_TradedAsset, TradeOffer_Trade,
};

pub trait HasAssets {
    type Asset;
    fn every_asset(self) -> Vec<Self::Asset>;
}

pub trait FilterBy<C> {
    fn filter_by<T: Fn(&C) -> bool>(self, filter_fn: T) -> Vec<C>;

    fn partition_by<T: Fn(&C) -> bool>(self, filter_fn: T) -> (Vec<C>, Vec<C>);
}

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
    type Asset = CEcon_Asset;

    fn every_asset(self) -> Vec<CEcon_Asset> {
        unimplemented!()
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
        unimplemented!()
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
        unimplemented!()
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
        unimplemented!()
    }
}

use steam_web_api_derive::{interface, Parameters};

import!();

new_type!(IEconService);
impl_conversions!(@GetQueryBuilder -> @IEconService);
convert_with_endpoint!(@GetQueryBuilder -> @IEconService);

#[interface(IEconService)]
#[derive(Parameters, Debug, Default)]
pub struct GetTradeHistoryParameters {
    max_trades: u32,
    start_after_time: u32,
    start_after_tradeid: u64,
    navigating_back: bool,
    get_descriptions: bool,
    language: String,
    include_failed: bool,
    include_total: bool,
}

#[interface(IEconService)]
#[derive(Parameters, Debug, Default)]
pub struct GetTradeOffersParameters {
    get_sent_offers: bool,
    get_received_offers: bool,
    get_descriptions: bool,
    time_historical_cutoff: u32,
    active_only: Option<bool>,
    historical_only: Option<bool>,
    language: Option<String>,
}

#[interface(IEconService)]
#[derive(Parameters, Debug, Default)]
pub struct GetTradeOfferParameters {
    tradeofferid: u64,
    language: Option<String>,
}

convert_with_endpoint!(@IEconService -> GetTradeHistory |> "GetTradeHistory/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffers |> "GetTradeOffers/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffer |> "GetTradeOffer/v1");

exec!(GetTradeHistory);
exec!(GetTradeOffers);
exec!(GetTradeOffer);

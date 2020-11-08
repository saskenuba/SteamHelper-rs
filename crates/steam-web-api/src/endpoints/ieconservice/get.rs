use steam_web_api_derive::{interface, Parameters};

import!();

use crate::response_types::GetTradeHoldDurationsResponseBase;

new_type!(IEconService);

impl_conversions!(@GetQueryBuilder -> @IEconService);
convert_with_endpoint!(@GetQueryBuilder -> @IEconService);

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct GetTradeHistoryParameters {
    max_trades: u32,
    include_failed: bool,
    include_total: bool,
    start_after_time: Option<u32>,
    start_after_tradeid: Option<u64>,
    navigating_back: Option<bool>,
    get_descriptions: Option<bool>,
    language: Option<String>,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct GetTradeHoldDurationsParameters {
    steamid_target: u64,
    trade_offer_access_token: String,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct GetTradeOffersParameters {
    get_sent_offers: bool,
    get_received_offers: bool,
    time_historical_cutoff: u32,
    active_only: Option<bool>,
    historical_only: Option<bool>,
    get_descriptions: Option<bool>,
    language: Option<String>,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct GetTradeOfferParameters {
    tradeofferid: u64,
    language: Option<String>,
}

convert_with_endpoint!(@IEconService -> GetTradeHistory |> "GetTradeHistory/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffers |> "GetTradeOffers/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffer |> "GetTradeOffer/v1");
convert_with_endpoint!(@IEconService -> GetTradeHoldDurations |> "GetTradeHoldDurations/v1");

exec!(GetTradeHistory);
exec!(GetTradeOffers);
exec!(GetTradeOffer);
exec!(GetTradeHoldDurations -> GetTradeHoldDurationsResponseBase);

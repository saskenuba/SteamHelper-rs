use tappet_derive::{interface, Parameters};

use crate::response_types::GetTradeHoldDurationsResponseBase;

import!();

new_type!(IEconService);

impl_conversions!(@GetQueryBuilder -> @IEconService);
convert_with_endpoint!(@GetQueryBuilder -> @IEconService);

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetTradeHistoryParameters {
    max_trades: u32,
    include_failed: bool,
    include_total: bool,
    start_after_time: Option<u32>,
    start_after_tradeid: Option<i64>,
    navigating_back: Option<bool>,
    get_descriptions: Option<bool>,
    language: Option<String>,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetTradeHoldDurationsParameters {
    steamid_target: u64,
    trade_offer_access_token: String,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
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
#[doc(hidden)]
pub struct GetTradeOfferParameters {
    tradeofferid: i64,
    language: Option<String>,
}

convert_with_endpoint!(@IEconService -> GetTradeHistory |> "GetTradeHistory/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffers |> "GetTradeOffers/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffer |> "GetTradeOffer/v1");
convert_with_endpoint!(@IEconService -> GetTradeHoldDurations |> "GetTradeHoldDurations/v1");

impl_executor!(GetTradeHoldDurations -> GetTradeHoldDurationsResponseBase);

cfg_if::cfg_if! {
    if #[cfg(feature = "trading")] {
        use crate::response_types::{ GetTradeHistoryResponse, GetTradeOffersResponse, GetTradeOfferResponse };
        impl_executor!(GetTradeHistory -> GetTradeHistoryResponse);
        impl_executor!(GetTradeOffers -> GetTradeOffersResponse);
        impl_executor!(GetTradeOffer -> GetTradeOfferResponse);
    } else {
        impl_executor!(GetTradeHistory);
        impl_executor!(GetTradeOffers);
        impl_executor!(GetTradeOffer);
    }
}

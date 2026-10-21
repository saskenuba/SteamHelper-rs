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

// Not `#[interface]`: it would add `cursor` to the constructor's arguments. `cursor` is set with
// `GetTradeOffers::cursor` instead, and stays last so that the query is unchanged when it is not set.
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
    cursor: Option<u32>,
}

impl<'a> IEconService<'a> {
    /// A new request to the `GetTradeOffers` endpoint.
    ///
    /// The returning struct implements:
    /// - `execute`, for a raw `String` response; Requires import of trait `Executor`;
    /// - `execute_with_response`, for a jsonified response into a struct. Requires import of trait `ExecutorResponse`;
    /// - `inject_custom_key`, for injecting a custom api key different than the one used for instantiating `SteamAPI`,
    ///   which then you can execute with the options above;
    /// - `cursor`, to request the page after the first one.
    #[allow(clippy::too_many_arguments)]
    pub fn GetTradeOffers(
        self,
        get_sent_offers: bool,
        get_received_offers: bool,
        time_historical_cutoff: u32,
        active_only: Option<bool>,
        historical_only: Option<bool>,
        get_descriptions: Option<bool>,
        language: Option<String>,
    ) -> GetTradeOffers<'a> {
        let mut into: GetTradeOffers = self.into();
        into.parameters = GetTradeOffersParameters {
            get_sent_offers,
            get_received_offers,
            time_historical_cutoff,
            active_only,
            historical_only,
            get_descriptions,
            language,
            cursor: None,
        };
        into
    }
}

impl<'a> GetTradeOffers<'a> {
    /// Requests the page that starts at `cursor`, the `next_cursor` of the previous page (see
    /// `CEcon_GetTradeOffers_Response::next_page_cursor`). Steam pages this endpoint;
    /// node-steam-tradeoffer-manager requests pages until `next_cursor` is 0 or absent (`lib/index.js:589-619`).
    pub fn cursor(mut self, cursor: u32) -> Self {
        self.parameters.cursor = Some(cursor);
        self
    }
}

// Not `#[interface]`, for the same reason as `GetTradeOffersParameters`.
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetTradeOfferParameters {
    tradeofferid: i64,
    language: Option<String>,
    get_descriptions: Option<bool>,
}

impl<'a> IEconService<'a> {
    /// A new request to the `GetTradeOffer` endpoint.
    ///
    /// The returning struct implements:
    /// - `execute`, for a raw `String` response; Requires import of trait `Executor`;
    /// - `execute_with_response`, for a jsonified response into a struct. Requires import of trait `ExecutorResponse`;
    /// - `inject_custom_key`, for injecting a custom api key different than the one used for instantiating `SteamAPI`,
    ///   which then you can execute with the options above;
    /// - `get_descriptions`, to also request the item descriptions.
    pub fn GetTradeOffer(self, tradeofferid: i64, language: Option<String>) -> GetTradeOffer<'a> {
        let mut into: GetTradeOffer = self.into();
        into.parameters = GetTradeOfferParameters {
            tradeofferid,
            language,
            get_descriptions: None,
        };
        into
    }
}

impl<'a> GetTradeOffer<'a> {
    pub fn get_descriptions(mut self, get_descriptions: bool) -> Self {
        self.parameters.get_descriptions = Some(get_descriptions);
        self
    }
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetTradeOffersSummaryParameters {
    // Unix time; a plain comment because `Parameters` rejects doc attributes on fields.
    time_last_visit: Option<u32>,
}

convert_with_endpoint!(@IEconService -> GetTradeHistory |> "GetTradeHistory/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffers |> "GetTradeOffers/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffer |> "GetTradeOffer/v1");
convert_with_endpoint!(@IEconService -> GetTradeHoldDurations |> "GetTradeHoldDurations/v1");
convert_with_endpoint!(@IEconService -> GetTradeOffersSummary |> "GetTradeOffersSummary/v1");

impl_executor!(GetTradeHoldDurations -> GetTradeHoldDurationsResponseBase);

cfg_if::cfg_if! {
    if #[cfg(feature = "trading")] {
        use crate::response_types::{
            GetTradeHistoryResponse, GetTradeOfferResponse, GetTradeOffersResponse, GetTradeOffersSummaryResponse,
        };
        impl_executor!(GetTradeHistory -> GetTradeHistoryResponse);
        impl_executor!(GetTradeOffers -> GetTradeOffersResponse);
        impl_executor!(GetTradeOffer -> GetTradeOfferResponse);
        impl_executor!(GetTradeOffersSummary -> GetTradeOffersSummaryResponse);
    } else {
        impl_executor!(GetTradeHistory);
        impl_executor!(GetTradeOffers);
        impl_executor!(GetTradeOffer);
        impl_executor!(GetTradeOffersSummary);
    }
}

use steam_web_api_derive::{interface, Parameters};

import!();

new_type!(IEconService);

impl_conversions!(@PostQueryBuilder -> @IEconService);
convert_with_endpoint!(@PostQueryBuilder -> @IEconService);

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct DeclineTradeOfferParameters {
    tradeofferid: i64,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct CancelTradeOfferParameters {
    tradeofferid: i64,
}

convert_with_endpoint!(@IEconService -> CancelTradeOffer |> "CancelTradeOffer/v1");
convert_with_endpoint!(@IEconService -> DeclineTradeOffer |> "DeclineTradeOffer/v1");

impl_executor!(CancelTradeOffer);
impl_executor!(DeclineTradeOffer);
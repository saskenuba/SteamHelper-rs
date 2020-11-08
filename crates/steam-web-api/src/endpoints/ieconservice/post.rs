use steam_web_api_derive::{interface, Parameters};

import!();

new_type!(IEconService);

impl_conversions!(@PostQueryBuilder -> @IEconService);
convert_with_endpoint!(@PostQueryBuilder -> @IEconService);

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct DeclineTradeOfferParameters {
    tradeofferid: u64,
}

#[interface(IEconService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct CancelTradeOfferParameters {
    tradeofferid: u64,
}

convert_with_endpoint!(@IEconService -> CancelTradeOffer |> "CancelTradeOffer/v1");
convert_with_endpoint!(@IEconService -> DeclineTradeOffer |> "DeclineTradeOffer/v1");

exec!(CancelTradeOffer);
exec!(DeclineTradeOffer);
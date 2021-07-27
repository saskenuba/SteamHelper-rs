use steam_web_api_derive::{interface, Parameters};

import!();

use crate::response_types::GetCMListResponseBase;

new_type!(ISteamDirectory);
impl_conversions!(@GetQueryBuilder -> @ISteamDirectory);
convert_with_endpoint!(@GetQueryBuilder -> @ISteamDirectory);

#[interface(ISteamDirectory)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetCMListParameters {
    cellid: Option<u32>,
    maxcount: Option<u32>,
}

#[interface(ISteamDirectory)]
#[derive(Parameters, Serialize, Debug, Default)]
#[doc(hidden)]
pub struct GetCSListParameters {
    cellid: Option<u32>,
    maxcount: Option<u32>,
}

convert_with_endpoint!(@ISteamDirectory -> GetCMList |> "GetCMList/v1");
convert_with_endpoint!(@ISteamDirectory -> GetCSList |> "GetCSList/v1");

impl_executor!(GetCMList -> GetCMListResponseBase);
impl_executor!(GetCSList);

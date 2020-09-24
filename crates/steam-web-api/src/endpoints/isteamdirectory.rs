use steam_web_api_derive::{interface, Parameters};

use crate::{
    Executor,
    ExecutorResponse,
    GetQueryBuilder, helpers::{comma_delimited, indexed_array, querify}, response_types::{GetPlayerBansResponseBase, GetPlayerSummariesResponseBase},
};
use crate::response_types::GetCMListResponseBase;

new_type!(ISteamDirectory);
impl_conversions!(@GetQueryBuilder -> @ISteamDirectory);
convert_with_endpoint!(@GetQueryBuilder -> @ISteamDirectory);

#[interface(ISteamDirectory)]
#[derive(Parameters, Debug, Default)]
pub struct GetCMListParameters {
    cellid: u32,
    maxcount: Option<u32>,
}

#[interface(ISteamDirectory)]
#[derive(Parameters, Debug, Default)]
pub struct GetCSListParameters {
    cellid: u32,
    maxcount: Option<u32>,
}

convert_with_endpoint!(@ISteamDirectory -> GetCMList |> "GetCMList/v1");
convert_with_endpoint!(@ISteamDirectory -> GetCSList |> "GetCSList/v1");

exec!(GetCMList -> GetCMListResponseBase);
exec!(GetCSList);

use anyhow::Result;
use reqwest::Error;
use steam_web_api::response_types::GetCMListResponseBase;
use steam_web_api::ExecutorResponse;
use tokio_compat_02::FutureExt;

use crate::API_CLIENT;

pub async fn dump_tcp_servers() -> Result<Vec<String>> {
    let cm_list: GetCMListResponseBase = API_CLIENT
        .get()
        .ISteamDirectory()
        .GetCMList(Some(25), None)
        .execute_with_response()
        .compat()
        .await?;

    Ok(cm_list.response.serverlist)
}

pub async fn fetch_servers_fallback() -> Result<String, Error> {
    let url = "cm0.steampowered.com";
    unimplemented!()
}

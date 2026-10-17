use anyhow::Result;
use tappet::response_types::GetCMListResponseBase;
use tappet::ExecutorResponse;

use crate::API_CLIENT;

pub async fn dump_tcp_servers() -> Result<Vec<String>> {
    let cm_list: GetCMListResponseBase = API_CLIENT
        .get()
        .ISteamDirectory()
        .GetCMList(Some(25), None)
        .execute_with_response()
        .await?;

    Ok(cm_list.response.serverlist)
}

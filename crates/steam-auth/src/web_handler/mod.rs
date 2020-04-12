use reqwest::Method;

use crate::{
    errors::ApiKeyError, steam_scraper::api_key_resolve_status, types::ApiKeyRegisterRequest,
    MobileClient, STEAM_COMMUNITY_BASE,
};

mod login;
mod confirmation;

async fn session_refresh() {}

async fn parental_unlock() {}

async fn confirmation_retrieve() {
    let my_steamid = "123";
    let confirmation_hash = ""; // url_encoded
    let device_id = ""; // url_encoded
    let time = ""; //?

    let teste = format!(
        "{}/mobileconf/conf?a={}&k={}&l=english&m=android&p={}&t={}&tag=conf",
        STEAM_COMMUNITY_BASE, my_steamid, confirmation_hash, device_id, time
    );
}

async fn api_key_retrieve(client: &MobileClient) -> Result<String, ApiKeyError> {
    let api_key_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/apikey?l=english");
    let doc = client.get_html(&api_key_url).await.unwrap();
    let api = match api_key_resolve_status(doc) {
        Ok(api) => api,
        Err(ApiKeyError::NotRegistered) => {
            // in this case we want to register it
            api_key_register(&client).await?
        }
        Err(e) => return Err(e),
    };
    Ok(api)
}

/// Request access to an API Key
/// The account should be validated before.
async fn api_key_register(client: &MobileClient) -> Result<String, ApiKeyError> {
    let api_register_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/registerkey");
    let register_request = ApiKeyRegisterRequest::default();

    let response =
        client.request(
            &api_register_url,
            Method::POST,
            None,
            Some(register_request)
        ).await?;

    Ok("".to_string())
}

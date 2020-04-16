use reqwest::Method;

use const_concat::const_concat;

use crate::{
    client::MobileClient, errors::ApiKeyError, page_scraper::api_key_resolve_status,
    types::ApiKeyRegisterRequest, User, STEAM_COMMUNITY_BASE,
};

/// used to refresh session
const MOBILE_AUTH_GETWGTOKEN: &str =
    const_concat!(crate::STEAM_API_BASE, "/IMobileAuthService/GetWGToken/v0001");

mod confirmation;
mod login;
mod trade;

async fn session_refresh() {}

async fn parental_unlock() {}

async fn cache_operations() {}

async fn confirmation_retrieve_active(client: &MobileClient, user: &User) {
    // the api key needs to be set for this to work
    let api_key = user.cached_info.api_key.as_ref().unwrap();

    let my_steamid = user.steam_id().unwrap();

    let secret = user.identity_secret().expect(
        "You need to have a linked ma file to \
    recover confirmations",
    );
    let time = steam_totp::time::Time::with_offset().await.unwrap();
    let confirmation_hash = steam_totp::generate_confirmation_key(secret, time, Some("conf")).unwrap();
    let device_id = user.device_id().expect("You need a linked device id");

    let mobile_confirmation_url = format!(
        "{}/mobileconf/conf?a={}&k={}&l=english&m=android&p={}&t={}&tag=conf",
        STEAM_COMMUNITY_BASE, my_steamid, confirmation_hash, device_id, time
    );
}

async fn api_key_retrieve(client: &MobileClient) -> Result<String, ApiKeyError> {
    let api_key_url = format!("{}{}", STEAM_COMMUNITY_BASE, "/dev/apikey?l=english");
    let doc = client.get_html(&api_key_url).await?;
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
        client.request(&api_register_url, Method::POST, None, Some(register_request)).await?;

    Ok("".to_string())
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    #[test]
    fn test_path_handling() {
        let lost_auth = Url::parse("steammobile://lostauth/login").unwrap();
        assert_eq!("steammobile", lost_auth.scheme());
        assert_eq!("lostauth", lost_auth.host_str().unwrap());
        assert_eq!("/login", lost_auth.path());
    }
}

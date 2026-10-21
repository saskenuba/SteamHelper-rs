//! Requests as they go out, built without sending them.
//!
//! The wire format is frozen: with an API key, every existing call must send exactly what it sent before
//! credentials, cursors and descriptions were added. The expected URLs were captured from the code before that change
//! (they include its `key=...&&` double ampersand), and each test also replays the old request building code.
//!
//! The one approved change is the scheme: `https://api.steampowered.com` instead of `http://`, as
//! node-steam-tradeoffer-manager does. The captured URLs were edited for it, nothing else.

use tappet_derive::{interface, Parameters};

#[cfg(feature = "blocking")]
use crate::blocking::SteamAPI;
use crate::macros::HttpRequest;
use crate::Credential;
#[cfg(feature = "async")]
use crate::SteamAPI;

import!();

/// The request building code before access tokens, for GET.
macro_rules! old_get_url {
    ($endpoint:expr) => {{
        let endpoint = $endpoint;
        let query: String = endpoint.recover_params();
        let mut req = endpoint.request;
        let api_key_parameter = format!("key={}", endpoint.key);
        req.url_mut().set_query(Some(&(api_key_parameter + "&" + &query)));
        req.url().to_string()
    }};
}

fn assert_unchanged(request: HttpRequest, old_url: String, captured: &str) {
    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().as_str(), captured);
    assert_eq!(request.url().as_str(), old_url);
    assert!(request.headers().is_empty(), "{:?}", request.headers());
}

/// `$endpoint` builds the endpoint struct; it is called twice, for the old and the new code.
macro_rules! check_unchanged {
    ($endpoint:expr, $captured:literal) => {{
        let endpoint = $endpoint;
        let old_url = old_get_url!(endpoint());
        assert_unchanged(endpoint().prepare().unwrap(), old_url, $captured);
    }};
}

#[test]
fn get_requests_with_a_key_are_unchanged() {
    let api = SteamAPI::new("APIKEY");

    check_unchanged!(
        || api.get().IEconService().GetTradeOffers(true, true, 1000, Some(true), None, None, None),
        "https://api.steampowered.com/IEconService/GetTradeOffers/v1?key=APIKEY&&get_sent_offers=true&get_received_offers=true&time_historical_cutoff=1000&active_only=true"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeOffers(
            true,
            false,
            4294967295,
            Some(false),
            Some(true),
            Some(false),
            Some("english".to_string())
        ),
        "https://api.steampowered.com/IEconService/GetTradeOffers/v1?key=APIKEY&&get_sent_offers=true&get_received_offers=false&time_historical_cutoff=4294967295&active_only=false&historical_only=true&get_descriptions=false&language=english"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeOffer(4000000000, None),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=APIKEY&&tradeofferid=4000000000"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeOffer(5, Some("brazilian portuguese".to_string())),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=APIKEY&&tradeofferid=5&language=brazilian%20portuguese"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeHistory(500, false, false, None, None, None, None, None),
        "https://api.steampowered.com/IEconService/GetTradeHistory/v1?key=APIKEY&&max_trades=500&include_failed=false&include_total=false"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeHistory(
            10,
            true,
            true,
            Some(1),
            Some(2),
            Some(true),
            Some(false),
            Some("english".to_string())
        ),
        "https://api.steampowered.com/IEconService/GetTradeHistory/v1?key=APIKEY&&max_trades=10&include_failed=true&include_total=true&start_after_time=1&start_after_tradeid=2&navigating_back=true&get_descriptions=false&language=english"
    );
    check_unchanged!(
        || api.get().IEconService().GetTradeHoldDurations(76561198040191316, "Ob27qXzn".to_string()),
        "https://api.steampowered.com/IEconService/GetTradeHoldDurations/v1?key=APIKEY&&steamid_target=76561198040191316&trade_offer_access_token=Ob27qXzn"
    );
    check_unchanged!(
        || api
            .get()
            .ISteamUser()
            .GetPlayerSummaries(vec!["1".to_string(), "2".to_string()]),
        "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2?key=APIKEY&&steamids=1,2"
    );
    check_unchanged!(
        || api
            .get()
            .ISteamUser()
            .GetPlayerSummaries(vec!["1".to_string()])
            .inject_custom_key("OTHER"),
        "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2?key=OTHER&&steamids=1"
    );
    check_unchanged!(
        || api.get().ISteamDirectory().GetCMList(Some(0), None),
        "https://api.steampowered.com/ISteamDirectory/GetCMList/v1?key=APIKEY&&cellid=0"
    );
    check_unchanged!(
        || api.get().ISteamEconomy().GetAssetClassInfo(730, 2, vec![1, 2], Some(vec![0, 3]), None),
        "https://api.steampowered.com/ISteamEconomy/GetClassInfo/v1?key=APIKEY&&appid=730&class_count=2&classid[0]=1&classid[1]=2&instanceid[0]=0&instanceid[1]=3"
    );
}

#[test]
fn with_client_and_an_api_key_is_new() {
    let api = SteamAPI::with_client(Default::default(), Credential::ApiKey("APIKEY".to_string()));
    let request = api.get().IEconService().GetTradeOffer(5, None).prepare().unwrap();
    assert_eq!(
        request.url().as_str(),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=APIKEY&&tradeofferid=5"
    );
}

#[test]
fn cursor_is_appended_only_when_set() {
    let api = SteamAPI::new("APIKEY");
    let offers = || {
        api.get()
            .IEconService()
            .GetTradeOffers(true, true, 1000, Some(true), None, None, None)
    };
    let without = "https://api.steampowered.com/IEconService/GetTradeOffers/v1?key=APIKEY&&get_sent_offers=true&get_received_offers=true&time_historical_cutoff=1000&active_only=true";

    assert_eq!(offers().prepare().unwrap().url().as_str(), without);
    assert_eq!(
        offers().cursor(42).prepare().unwrap().url().as_str(),
        without.to_string() + "&cursor=42"
    );
}

#[test]
fn get_descriptions_is_appended_only_when_set() {
    let api = SteamAPI::new("APIKEY");
    let offer = || api.get().IEconService().GetTradeOffer(5, Some("english".to_string()));
    let without =
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=APIKEY&&tradeofferid=5&language=english";

    assert_eq!(offer().prepare().unwrap().url().as_str(), without);
    assert_eq!(
        offer().get_descriptions(true).prepare().unwrap().url().as_str(),
        without.to_string() + "&get_descriptions=true"
    );
}

#[test]
fn trade_offers_summary() {
    let api = SteamAPI::new("APIKEY");
    let summary = |time_last_visit| {
        api.get()
            .IEconService()
            .GetTradeOffersSummary(time_last_visit)
            .prepare()
            .unwrap()
            .url()
            .to_string()
    };

    assert_eq!(
        summary(None),
        "https://api.steampowered.com/IEconService/GetTradeOffersSummary/v1?key=APIKEY&"
    );
    assert_eq!(
        summary(Some(1700000000)),
        "https://api.steampowered.com/IEconService/GetTradeOffersSummary/v1?key=APIKEY&&time_last_visit=1700000000"
    );
}

#[test]
fn access_token_replaces_key() {
    let api = SteamAPI::with_access_token("eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiI3NjU2MSJ9.c2ln-_x");
    let request = api
        .get()
        .IEconService()
        .GetTradeOffers(true, true, 1000, Some(true), None, None, None)
        .prepare()
        .unwrap();

    assert_eq!(
        request.url().as_str(),
        "https://api.steampowered.com/IEconService/GetTradeOffers/v1?access_token=eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiI3NjU2MSJ9.c2ln-_x&&get_sent_offers=true&get_received_offers=true&time_historical_cutoff=1000&active_only=true"
    );
    assert!(request.headers().is_empty(), "{:?}", request.headers());
}

#[test]
fn access_token_is_encoded() {
    let api = SteamAPI::with_access_token("a+b/c=d&key=x");
    let request = api.get().IEconService().GetTradeOffer(5, None).prepare().unwrap();

    assert_eq!(
        request.url().as_str(),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?access_token=a%2Bb%2Fc%3Dd%26key%3Dx&&tradeofferid=5"
    );
}

#[test]
fn injected_key_wins_over_access_token() {
    let api = SteamAPI::with_access_token("TOKEN");
    let request = api
        .get()
        .IEconService()
        .GetTradeOffer(5, None)
        .inject_custom_key("OTHER")
        .prepare()
        .unwrap();

    assert_eq!(
        request.url().as_str(),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=OTHER&&tradeofferid=5"
    );
    assert!(request.headers().is_empty(), "{:?}", request.headers());
}

#[test]
fn set_credential() {
    let mut api = SteamAPI::new("APIKEY");
    api.set_credential(Credential::AccessToken("TOKEN".to_string()));
    let url = |api: &SteamAPI| {
        api.get()
            .IEconService()
            .GetTradeOffer(5, None)
            .prepare()
            .unwrap()
            .url()
            .to_string()
    };
    assert_eq!(
        url(&api),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?access_token=TOKEN&&tradeofferid=5"
    );

    api.set_api_key("APIKEY");
    assert_eq!(
        url(&api),
        "https://api.steampowered.com/IEconService/GetTradeOffer/v1?key=APIKEY&&tradeofferid=5"
    );
}

#[test]
fn debug_never_shows_secrets() {
    let token = "SECRET-TOKEN";
    let api = SteamAPI::with_access_token(token);
    let endpoint = api
        .get()
        .IEconService()
        .GetTradeOffers(true, true, 1000, None, None, None, None);

    for debug in &[
        format!("{:?}", Credential::AccessToken(token.to_string())),
        format!("{:?}", api),
        format!("{:?}", endpoint),
    ] {
        assert!(!debug.contains(token), "{}", debug);
    }

    let key = "SECRET-KEY";
    let api = SteamAPI::new(key);
    let endpoint = api
        .get()
        .IEconService()
        .GetTradeOffers(true, true, 1000, None, None, None, None);
    let injected = api.get().IEconService().GetTradeOffer(5, None).inject_custom_key(key);

    for debug in &[
        format!("{:?}", Credential::ApiKey(key.to_string())),
        format!("{:?}", api),
        format!("{:?}", endpoint),
        format!("{:?}", injected),
    ] {
        assert!(!debug.contains(key), "{}", debug);
    }
    // still useful
    let debug = format!("{:?}", injected);
    assert!(debug.starts_with("GetTradeOffer {"), "{}", debug);
    assert!(debug.contains("tradeofferid: 5"), "{}", debug);
}

#[test]
fn every_request_is_https() {
    let key = SteamAPI::new("APIKEY");
    let token = SteamAPI::with_access_token("TOKEN");

    for api in &[&key, &token] {
        let get = api.get().ISteamDirectory().GetCMList(None, None).prepare().unwrap();
        let post = PostQueryBuilder::from(*api)
            .ITestService()
            .TestPost(5, None, None)
            .prepare()
            .unwrap();

        for request in &[get, post] {
            assert_eq!(request.url().scheme(), "https", "{}", request.url());
            assert_eq!(request.url().host_str(), Some("api.steampowered.com"));
            assert_eq!(request.url().port_or_known_default(), Some(443));
        }
    }
}

/// A proxy address where nothing listens, so every request fails without leaving the machine.
fn dead_proxy() -> reqwest::Proxy {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    reqwest::Proxy::all(format!("http://{}", address)).unwrap()
}

fn assert_no_secret(error: crate::errors::SteamAPIError, secret: &str) {
    assert!(
        matches!(error, crate::errors::SteamAPIError::HttpError(_)),
        "{:?}",
        error
    );
    for text in &[error.to_string(), format!("{:?}", error)] {
        assert!(!text.contains(secret), "{}", text);
        // the endpoint is still there
        assert!(text.contains("api.steampowered.com"), "{}", text);
    }
}

#[cfg(feature = "async")]
#[tokio::test]
async fn http_errors_never_show_secrets() {
    let client = || reqwest::Client::builder().proxy(dead_proxy()).build().unwrap();
    let secret = "SECRET-CREDENTIAL";

    for credential in &[
        Credential::ApiKey(secret.to_string()),
        Credential::AccessToken(secret.to_string()),
    ] {
        let api = SteamAPI::with_client(client(), credential.clone());

        let raw = api.get().IEconService().GetTradeOffer(5, None).execute().await;
        assert_no_secret(raw.unwrap_err(), secret);

        let typed: crate::Result<crate::response_types::GetCMListResponseBase> = api
            .get()
            .ISteamDirectory()
            .GetCMList(None, None)
            .execute_with_response()
            .await;
        assert_no_secret(typed.unwrap_err(), secret);

        let post = PostQueryBuilder::from(&api).ITestService().TestPost(5, None, None).execute().await;
        assert_no_secret(post.unwrap_err(), secret);
    }

    // a trade token of the partner, also in the query
    let api = SteamAPI::with_client(client(), Credential::ApiKey("APIKEY".to_string()));
    let error = api
        .get()
        .IEconService()
        .GetTradeHoldDurations(76561198040191316, secret.to_string())
        .execute()
        .await
        .unwrap_err();
    assert_no_secret(error, secret);
}

#[cfg(feature = "blocking")]
#[test]
fn http_errors_never_show_secrets() {
    let client = || {
        reqwest::blocking::Client::builder()
            .proxy(dead_proxy())
            .build()
            .unwrap()
    };
    let secret = "SECRET-CREDENTIAL";

    for credential in &[
        Credential::ApiKey(secret.to_string()),
        Credential::AccessToken(secret.to_string()),
    ] {
        let api = SteamAPI::with_client(client(), credential.clone());

        let raw = api.get().IEconService().GetTradeOffer(5, None).execute();
        assert_no_secret(raw.unwrap_err(), secret);

        let typed: crate::Result<crate::response_types::GetCMListResponseBase> = api
            .get()
            .ISteamDirectory()
            .GetCMList(None, None)
            .execute_with_response();
        assert_no_secret(typed.unwrap_err(), secret);

        let post = PostQueryBuilder::from(&api).ITestService().TestPost(5, None, None).execute();
        assert_no_secret(post.unwrap_err(), secret);
    }
}

// No POST endpoint exists yet, so this one only exists to check the form.
new_type!(ITestService);
impl_conversions!(@PostQueryBuilder -> @ITestService);
convert_with_endpoint!(@PostQueryBuilder -> @ITestService);

#[interface(ITestService)]
#[derive(Parameters, Serialize, Debug, Default)]
pub struct TestPostParameters {
    tradeofferid: u64,
    partner: Option<String>,
    flag: Option<bool>,
}

convert_with_endpoint!(@ITestService -> TestPost |> "TestPost/v1");
impl_executor!(TestPost);

/// The form wrapper before access tokens.
#[derive(Serialize)]
struct OldFormWrapper<'a, T: Serialize> {
    #[serde(flatten)]
    parameters: T,
    key: &'a str,
}

fn body(request: &HttpRequest) -> String {
    String::from_utf8(request.body().unwrap().as_bytes().unwrap().to_vec()).unwrap()
}

#[test]
fn post_form_with_a_key_is_unchanged() {
    let api = SteamAPI::new("APIKEY");
    let endpoint = || {
        PostQueryBuilder::from(&api)
            .ITestService()
            .TestPost(4000000000, Some("a b&c".to_string()), None)
    };

    let old = {
        let endpoint = endpoint();
        let url = endpoint.request.url().to_owned();
        let data = OldFormWrapper {
            parameters: endpoint.recover_params_as_form(),
            key: endpoint.key,
        };
        endpoint.client.post(url).form(&data).build().unwrap()
    };
    let new = endpoint().prepare().unwrap();

    assert_eq!(new.method(), reqwest::Method::POST);
    assert_eq!(new.url(), old.url());
    assert_eq!(new.headers(), old.headers());
    assert_eq!(body(&new), body(&old));
    assert_eq!(body(&new), "tradeofferid=4000000000&partner=a+b%26c&key=APIKEY");
}

#[test]
fn post_form_with_an_access_token() {
    let api = SteamAPI::with_access_token("a+b/c");
    let request = PostQueryBuilder::from(&api)
        .ITestService()
        .TestPost(5, None, Some(true))
        .prepare()
        .unwrap();

    assert_eq!(
        request.url().as_str(),
        "https://api.steampowered.com/ITestService/TestPost/v1"
    );
    assert!(request.headers().get(crate::macros::ACCESS_TOKEN_CARRIER).is_none());
    assert_eq!(body(&request), "tradeofferid=5&flag=true&access_token=a%2Bb%2Fc");
}

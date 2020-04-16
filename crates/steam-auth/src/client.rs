use std::cell::RefCell;

use cookie::{Cookie, CookieJar};
use reqwest::{Client, header::HeaderMap, Method, Response, Url};
use scraper::Html;
use serde::Serialize;

use crate::{
    User,
    utils::dump_cookies_by_domain,
};

#[derive(Debug)]
/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades, and some other operations
/// that may or not be related to only mobile operations.
pub struct SteamAuthenticator {
    client: MobileClient,
    user: User,
}

/// Generate a Steam Authenticator that you use to login into the Steam Community / Steam Store.
impl SteamAuthenticator {
    pub fn new(user: User) -> Self {
        Self { client: MobileClient::default(), user }
    }

    pub fn client(&self) -> &MobileClient {
        &self.client
    }
}

#[derive(Debug)]
pub struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub inner_http_client: reqwest::Client,
    /// We manually handle cookies, because reqwest is just not good at it.
    pub cookie_store: RefCell<CookieJar>,
}

impl MobileClient {

    /// Wraps the request function with some guards in case we lose the session.
    /// Basically only the login operation do some interaction without requiring a session.
    pub(crate) async fn request_with_session_guard<T>(
        &self,
        url: &str,
        method: reqwest::Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error> where T: Serialize {

        // implement guards

        Ok(self.request(
            url,
            method,
            custom_headers,
            data
        ).await?)
    }

    // remember that we need to disallow redirects
    // because it may redirect us to weird paths, such as steamobile://xxx
    pub(crate) async fn request<T>(
        &self,
        url: &str,
        method: reqwest::Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error> where T: Serialize {
        let parsed_url = Url::parse(url).unwrap();
        let mut header_map = custom_headers.unwrap_or_default();

        // Send cookies from jar, based on domain
        let domain = &format!(".{}", parsed_url.host_str().unwrap());
        let domain_cookies = dump_cookies_by_domain(
            &self.cookie_store.borrow(),
            domain,
        );
        header_map.insert(
            reqwest::header::COOKIE,
            domain_cookies.unwrap_or_else(|| "".to_string()).parse().unwrap(),
        );

        let req_builder = self.inner_http_client.request(method, parsed_url).headers(header_map);

        let request = match data {
            None => req_builder.build().unwrap(),
            Some(data) => req_builder.form(&data).build().unwrap(),
        };

        // dbg!(&request);
        Ok(self.inner_http_client.execute(request).await?)
    }

    /// Convenience function to retrieve HTML
    pub(crate) async fn get_html(&self, url: &str) -> Result<Html, reqwest::Error> {
        let response = self.request_with_session_guard(url, Method::GET, None, None::<&str>).await?;
        let response_text = response.text().await?;
        let _html_document = Html::parse_document(&response_text);

        Ok(_html_document)
    }

    /// Initiate cookie jar with mobile cookies
    fn init_cookie_jar() -> CookieJar {
        let mut mobile_cookies = CookieJar::new();
        mobile_cookies.add(Cookie::build("Steam_Language", "english").domain(crate::STEAM_COMMUNITY_HOST).finish());
        mobile_cookies.add(Cookie::build("mobileClient", "android").domain(crate::STEAM_COMMUNITY_HOST).finish());
        mobile_cookies.add(Cookie::build("mobileClientVersion", "0 (2.1.3)").domain(crate::STEAM_COMMUNITY_HOST).finish());
        mobile_cookies
    }

    /// Initiate mobile client with default headers
    fn init_mobile_client() -> Client {
        let user_agent = "Mozilla/5.0 (Linux; U; Android 4.1.1; en-us; Google Nexus 4 - 4.1.1 - \
    API 16 - 768x1280 Build/JRO03S) AppleWebKit/534.30 (KHTML, like Gecko) Version/4.0 Mobile Safari/534.30";
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            reqwest::header::ACCEPT,
            "text/javascript, text/html, application/xml, text/xml, */*".parse().unwrap(),
        );
        default_headers.insert(reqwest::header::REFERER, crate::MOBILE_REFERER.parse().unwrap());
        default_headers.insert(
            "X-Requested-With", "com.valvesoftware.android.steam.community".parse().unwrap(),
        );

        let no_redirect_policy = reqwest::redirect::Policy::none();
        reqwest::Client::builder()
            // we need to take out cookie store, and use only our jar
            .cookie_store(true)
            .user_agent(user_agent)
            .redirect(no_redirect_policy)
            .default_headers(default_headers)
            .referer(false)
            .build()
            .unwrap()
    }
}

impl Default for MobileClient {
    fn default() -> Self {
        Self { inner_http_client: Self::init_mobile_client(), cookie_store: RefCell::new(Self::init_cookie_jar()) }
    }
}
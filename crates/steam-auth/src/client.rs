use std::{cell::RefCell, rc::Rc};

use cookie::{Cookie, CookieJar};
use reqwest::{header::HeaderMap, Client, Method, Response, Url};
use scraper::Html;
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::{
    errors::AuthError,
    utils::{dump_cookies_by_domain, retrieve_header_location},
    web_handler::{
        confirmation::Confirmations, confirmations_retrieve_all, confirmations_send,
        login::login_website, parental_unlock,
    },
    CachedInfo, ConfirmationMethod, User,
};

#[derive(Debug)]
/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades, and some other operations that may or not be related
/// to mobile operations.
///
/// # Example: Fetch mobile notifications
///
/// ```rust
/// use steam_auth::{client::SteamAuthenticator, User};
/// ```
pub struct SteamAuthenticator {
    /// Inner client with cookie storage
    client: MobileClient,
    user: User,
    cached_data: Rc<RefCell<CachedInfo>>,
}

impl SteamAuthenticator {
    /// Constructs a Steam Authenticator that you use to login into the Steam Community / Steam
    /// Store.
    pub fn new(user: User) -> Self {
        Self {
            client: MobileClient::default(),
            user,
            cached_data: Rc::new(RefCell::new(Default::default())),
        }
    }

    fn client(&self) -> &MobileClient {
        &self.client
    }

    /// Login into Steam, and unlock parental control if needed.
    pub async fn login(&self) -> Result<(), AuthError> {
        login_website(&self.client, &self.user, self.cached_data.borrow_mut()).await?;
        info!("Login to Steam successfully.");
        parental_unlock(&self.client, &self.user).await?;
        info!("Parental unlock successfully.");

        Ok(())
    }

    /// Fetch confirmations with the authenticator.
    pub async fn fetch_confirmations(&self) -> Result<Option<Confirmations>, AuthError> {
        // TODO: With details? Maybe we need to check if there is a need to gather more details.
        let steamid = self.cached_data.borrow().steam_id().unwrap();
        let confirmations = confirmations_retrieve_all(&self.client, &self.user, steamid, false)
            .await?
            .map(Confirmations::from);
        Ok(confirmations)
    }

    async fn process_tradeoffers(
        &self,
        operation: ConfirmationMethod,
        trade_offers_ids: &[&u64],
    ) -> Result<(), AuthError> {
        let steamid = self.cached_data.borrow().steam_id().unwrap();

        let confirmations = confirmations_retrieve_all(&self.client, &self.user, steamid, false)
            .await?
            .map(Confirmations::from);

        Ok(())
    }

    /// Accept or deny confirmations.
    pub async fn process_confirmations(
        &self,
        operation: ConfirmationMethod,
        confirmations: Confirmations,
    ) -> Result<(), AuthError> {
        let steamid = self.cached_data.borrow().steam_id().unwrap();
        confirmations_send(
            &self.client,
            &self.user,
            steamid,
            operation,
            confirmations.0,
        )
        .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub inner_http_client: Client,
    /// Cookie jar that manually handle cookies, because reqwest doens't let us handle its
    /// cookies.
    pub cookie_store: Rc<RefCell<CookieJar>>,
}

impl MobileClient {
    /// Wrapper to make requests while preemptively checking if the session is still valid.
    pub(crate) async fn request_with_session_guard<T>(
        &self,
        url: String,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error>
    where
        T: Serialize,
    {
        // We check preemptively if the session is still working.
        if self.session_is_expired().await? {
            warn!("Session was lost. Trying to reconnect.");
            unimplemented!()
        };

        self.request(url, method, custom_headers, data).await
    }

    /// Simple wrapper to allow generic requests to be made.
    pub(crate) async fn request<T>(
        &self,
        url: String,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error>
    where
        T: Serialize,
    {
        let parsed_url = Url::parse(&url).unwrap();
        let mut header_map = custom_headers.unwrap_or_default();

        // Send cookies stored on jar, based on the domain that we are requesting
        let domain = &format!(".{}", parsed_url.host_str().unwrap());
        let domain_cookies = dump_cookies_by_domain(&self.cookie_store.borrow(), domain);
        header_map.insert(
            reqwest::header::COOKIE,
            domain_cookies
                .unwrap_or_else(|| "".to_string())
                .parse()
                .unwrap(),
        );

        let req_builder = self
            .inner_http_client
            .request(method, parsed_url)
            .headers(header_map);

        let request = match data {
            None => req_builder.build().unwrap(),
            Some(data) => req_builder.form(&data).build().unwrap(),
        };

        debug!("{:?}", &request);
        self.inner_http_client.execute(request).await
    }

    /// Checks if session is expired by parsing the the redirect URL for "steamobile:://lostauth"
    /// or a path that starts with "/login".
    ///
    /// This is the most reliable way to find out, since we check the session by requesting our
    /// account page at Steam Store, which is not going to be deprecated anytime soon.
    async fn session_is_expired(&self) -> Result<bool, reqwest::Error> {
        let account_url = format!("{}/account", crate::STEAM_STORE_BASE);

        // FIXME: Not sure if we should request from client directly
        let response = self
            .request(account_url, Method::HEAD, None, None::<&str>)
            .await?;

        if let Some(location) = retrieve_header_location(&response) {
            return Ok(Url::parse(location).map(Self::url_expired_check).unwrap());
        }
        Ok(false)
    }

    fn url_expired_check(redirect_url: Url) -> bool {
        redirect_url.host_str().unwrap() == "lostauth" || redirect_url.path().starts_with("/login")
    }

    /// Convenience function to retrieve HTML w/ session
    pub(crate) async fn get_html(&self, url: String) -> Result<Html, reqwest::Error> {
        let response = self
            .request_with_session_guard(url, Method::GET, None, None::<&str>)
            .await?;
        let response_text = response.text().await?;
        let html_document = Html::parse_document(&response_text);

        info!("{}", &response_text);
        Ok(html_document)
    }

    /// Replace current cookie jar with a new one.
    fn reset_jar(&mut self) {
        self.cookie_store = Rc::new(RefCell::new(CookieJar::new()));
    }

    /// Mobile cookies that makes us look like the mobile app
    fn standard_mobile_cookies() -> Vec<Cookie<'static>> {
        vec![
            Cookie::build("Steam_Language", "english")
                .domain(crate::STEAM_COMMUNITY_HOST)
                .finish(),
            Cookie::build("mobileClient", "android")
                .domain(crate::STEAM_COMMUNITY_HOST)
                .finish(),
            Cookie::build("mobileClientVersion", "0 (2.1.3)")
                .domain(crate::STEAM_COMMUNITY_HOST)
                .finish(),
        ]
    }

    /// Initialize cookie jar, and populates it with mobile cookies.
    fn init_cookie_jar() -> CookieJar {
        let mut mobile_cookies = CookieJar::new();
        Self::standard_mobile_cookies()
            .into_iter()
            .for_each(|cookie| mobile_cookies.add(cookie));
        mobile_cookies
    }

    /// Initiate mobile client with default headers
    fn init_mobile_client() -> Client {
        let user_agent = "Mozilla/5.0 (Linux; U; Android 4.1.1; en-us; Google Nexus 4 - 4.1.1 - \
                          API 16 - 768x1280 Build/JRO03S) AppleWebKit/534.30 (KHTML, like Gecko) \
                          Version/4.0 Mobile Safari/534.30";
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            reqwest::header::ACCEPT,
            "text/javascript, text/html, application/xml, text/xml, */*"
                .parse()
                .unwrap(),
        );
        default_headers.insert(
            reqwest::header::REFERER,
            crate::MOBILE_REFERER.parse().unwrap(),
        );
        default_headers.insert(
            "X-Requested-With",
            "com.valvesoftware.android.steam.community".parse().unwrap(),
        );

        let no_redirect_policy = reqwest::redirect::Policy::none();
        reqwest::Client::builder()
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
        Self {
            inner_http_client: Self::init_mobile_client(),
            cookie_store: Rc::new(RefCell::new(Self::init_cookie_jar())),
        }
    }
}

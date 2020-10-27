use std::{cell::RefCell, rc::Rc};

use backoff::future::FutureOperation;
use futures::TryFutureExt;
use reqwest::redirect::Policy;
use reqwest::{header::HeaderMap, Client, Method, Response, Url};
use scraper::Html;
use serde::Serialize;
use tokio::time::Duration;
use tracing::{debug, info, warn};

use cookie::{Cookie, CookieJar};

use crate::errors::{LinkerError, LoginError};
use crate::retry::login_retry_strategy;
use crate::types::LoginCaptcha;
use crate::web_handler::authenticator::{
    account_has_phone, add_authenticator_to_account, add_phone_to_account, check_email_confirmation, check_sms,
    finalize_authenticator, validate_phone_number, AddAuthenticatorStep, STEAM_ADD_PHONE_CATCHUP_SECS,
};
use crate::{
    errors::AuthError,
    utils::{dump_cookies_by_domain, dump_cookies_by_name, retrieve_header_location},
    web_handler::{
        cache_resolve, confirmation::Confirmations, confirmations_retrieve_all, confirmations_send,
        login::login_website, parental_unlock,
    },
    CachedInfo, ConfirmationMethod, MobileAuthFile, User, STEAM_COMMUNITY_HOST,
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

    /// Returns current user API Key. Need to login first.
    pub fn api_key(&self) -> Option<String> {
        let mut api_key = Default::default();

        {
            api_key = self.cached_data.borrow().api_key().cloned();
        }
        api_key
    }

    fn client(&self) -> &MobileClient {
        &self.client
    }

    /// Log on into Steam website and populates the inner client with cookies for the Steam Store,
    /// Steam community and Steam help domains.
    ///
    /// Automatically unlocks parental control if user uses it, but it need to be included inside
    /// the [User] builder.
    ///
    /// The mobile client also has a very simple exponential retry strategy for errors that are *probably*
    /// caused by fast requests, so we retry it. For errors such as bad credentials, or inserting captcha
    /// the proper errors are raised by `AuthError`.
    ///
    /// Also caches the API Key, if the user wants to use it for any operation later.
    ///
    /// The cookies are inside the [MobileClient] inner cookie storage.
    pub async fn login(&self, captcha: Option<LoginCaptcha<'_>>) -> Result<(), AuthError> {
        // FIXME: Add more permanent errors, such as bad credentials
        (|| async {
            login_website(&self.client, &self.user, self.cached_data.borrow_mut(), captcha.clone())
                .await
                .map_err(|e| match e {
                    LoginError::CaptchaRequired { captcha_guid } => {
                        backoff::Error::Permanent(LoginError::CaptchaRequired { captcha_guid })
                    }
                    _ => {
                        warn!("Transient error happened: Trying again..");
                        backoff::Error::Transient(e)
                    }
                })
        })
        .retry(login_retry_strategy())
        .await?;

        info!("Login to Steam successfully.");
        // FIXME: This should work the same as login, because it can sometimes fail for no reason
        if self.user.parental_code.is_some() {
            parental_unlock(&self.client, &self.user).await?;
            info!("Parental unlock successfully.");
        }

        {
            cache_resolve(&self.client, self.cached_data.borrow_mut()).await?;
        }
        info!("Cached API Key successfully.");

        Ok(())
    }

    pub async fn add_authenticator(
        &self,
        current_step: AddAuthenticatorStep,
        phone_number: &str,
    ) -> Result<AddAuthenticatorStep, AuthError> {
        let user_has_phone_registered = account_has_phone(&self.client).await?;
        debug!("Has phone registered? {:?}", user_has_phone_registered);

        if !user_has_phone_registered && current_step == AddAuthenticatorStep::InitialStep {
            let phone_registration_result = self.add_phone_number(phone_number).await?;
            debug!("User add phone result: {:?}", phone_registration_result);

            return Ok(AddAuthenticatorStep::EmailConfirmation);
        }

        // Signal steam that user confirmed email
        // If user already has a phone, calling email confirmation will result in a error finalizing the auth process.
        if !user_has_phone_registered {
            check_email_confirmation(&self.client).await?;
            debug!("Email confirmation signal sent.");
        }

        add_authenticator_to_account(&self.client, self.cached_data.borrow())
            .await
            .map(|mafile| AddAuthenticatorStep::MobileAuth(mafile))
            .map_err(|e| e.into())
    }

    /// Wraps up the whole process, finishing the registration of the phone number into the account.
    /// You only call this method after `add_authenticator` works correctly, and you saved your .maFile somewhere safe.
    pub async fn finalize_authenticator(&self, mafile: &MobileAuthFile, sms_code: &str) -> Result<(), AuthError> {
        // The delay is that Steam need some seconds to catch up with the new phone number associated.
        let account_has_phone_now: bool = check_sms(&self.client, sms_code)
            .map_ok(|x| tokio::time::delay_for(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)))
            .and_then(|_| account_has_phone(&self.client))
            .await?;

        if !account_has_phone_now {
            return Err(LinkerError::GeneralFailure("This should not happen.".to_string()).into());
        }

        info!("Successfully confirmed SMS code.");

        finalize_authenticator(&self.client, self.cached_data.borrow(), mafile, sms_code)
            .await
            .map_err(|e| e.into())
    }

    pub async fn remove_authenticator(&self) {}

    /// Add a phone number into the account, and then checks it to make sure it has been added.
    /// Returns true if number was successfully added.
    async fn add_phone_number(&self, phone_number: &str) -> Result<bool, AuthError> {
        if !validate_phone_number(phone_number) {
            return Err(LinkerError::GeneralFailure(
                "Invalid phone number. Should be in format of: +(CountryCode)(AreaCode)(PhoneNumber). E.g +5511976914922"
                    .to_string(),
            ).into());
        }

        // Add the phone number to user account
        // The delay is that Steam need some seconds to catch up.
        let response = add_phone_to_account(&self.client, phone_number).await?;
        tokio::time::delay_for(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)).await;

        Ok(response)
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

        confirmations_send(&self.client, &self.user, steamid, operation, confirmations.0)
            .await
            .map_err(|e| e.into())
    }

    /// You can request custom operations for any Steam operation that requires logging in.
    ///
    /// The authenticator will take care sending session cookies and keeping the session
    /// operational.
    pub async fn request_custom_endpoint<T>(
        &self,
        url: String,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
    ) -> Result<Response, reqwest::Error>
    where
        T: Serialize,
    {
        self.client
            .request_with_session_guard(url, method, custom_headers, data)
            .await
    }

    pub fn dump_cookie(&self, steam_domain_host: &str, steam_cookie_name: &str) -> Option<String> {
        // TODO: Change domain and names to enums
        dump_cookies_by_name(&self.client.cookie_store.borrow(), steam_domain_host, steam_cookie_name)
    }
}

#[derive(Debug)]
pub struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub inner_http_client: Client,
    /// Cookie jar that manually handle cookies, because reqwest doesn't let us handle its
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

        self.request(url, method, custom_headers, data.as_ref()).await
    }

    /// Simple wrapper to allow generic requests to be made.
    pub(crate) async fn request<T>(
        &self,
        url: String,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<&T>,
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
            domain_cookies.unwrap_or_else(|| "".to_string()).parse().unwrap(),
        );

        let req_builder = self.inner_http_client.request(method, parsed_url).headers(header_map);

        let request = match data {
            None => req_builder.build().unwrap(),
            Some(data) => req_builder.form(data).build().unwrap(),
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
        let response = self.request(account_url, Method::HEAD, None, None::<&u8>).await?;

        if let Some(location) = retrieve_header_location(&response) {
            return Ok(Url::parse(location).map(Self::url_expired_check).unwrap());
        }
        Ok(false)
    }

    /// If url is redirecting to '/login' or lostauth, returns true
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

        debug!("{}", &response_text);
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
                .domain(STEAM_COMMUNITY_HOST)
                .finish(),
            Cookie::build("mobileClient", "android")
                .domain(STEAM_COMMUNITY_HOST)
                .finish(),
            Cookie::build("mobileClientVersion", "0 (2.1.3)")
                .domain(STEAM_COMMUNITY_HOST)
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
        default_headers.insert(reqwest::header::REFERER, crate::MOBILE_REFERER.parse().unwrap());
        default_headers.insert(
            "X-Requested-With",
            "com.valvesoftware.android.steam.community".parse().unwrap(),
        );

        reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(Policy::none())
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

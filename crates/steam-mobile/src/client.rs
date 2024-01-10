use std::sync::Arc;
use std::time::Duration;

use backoff::future::retry;
use cookie::{Cookie, CookieJar};
use futures::TryFutureExt;
use futures_timer::Delay;
use parking_lot::RwLock;
use reqwest::header::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::{Client, Method, Response, Url};
use scraper::Html;
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::errors::{AuthError, LinkerError, LoginError};
use crate::retry::login_retry_strategy;
use crate::types::LoginCaptcha;
use crate::utils::{dump_cookies_by_domain, dump_cookies_by_name, retrieve_header_location};
use crate::web_handler::confirmation::Confirmations;
use crate::web_handler::login::login_website;
use crate::web_handler::steam_guard_linker::{
    account_has_phone, add_authenticator_to_account, add_phone_to_account, check_email_confirmation, check_sms,
    finalize, remove_authenticator, validate_phone_number, AddAuthenticatorStep, RemoveAuthenticatorScheme,
    STEAM_ADD_PHONE_CATCHUP_SECS,
};
use crate::web_handler::{cache_resolve, confirmations_retrieve_all, confirmations_send, parental_unlock};
use crate::{CachedInfo, ConfirmationMethod, MobileAuthFile, User, STEAM_COMMUNITY_HOST};

/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades, and some other operations that may or not be related
/// to mobile operations.
///
/// # Example: Fetch mobile notifications
///
/// ```rust
/// use steam_mobile::client::SteamAuthenticator;
/// use steam_mobile::User;
/// ```
#[derive(Debug)]
pub struct SteamAuthenticator {
    /// Inner client with cookie storage
    pub(crate) client: MobileClient,
    user: User,
    pub(crate) cached_data: Arc<RwLock<CachedInfo>>,
}

impl SteamAuthenticator {
    /// Constructs a Steam Authenticator that you use to login into the Steam Community / Steam
    /// Store.
    #[must_use]
    pub fn new(user: User) -> Self {
        Self {
            client: MobileClient::default(),
            user,
            cached_data: Arc::new(RwLock::new(CachedInfo::default())),
        }
    }

    /// Returns current user API Key. Need to login first.
    pub fn api_key(&self) -> Option<String> {
        self.cached_data.read().api_key().map(ToString::to_string)
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
        retry(login_retry_strategy(), || async {
            login_website(&self.client, &self.user, self.cached_data.clone(), captcha.clone())
                .await
                .map_err(|error| match error {
                    LoginError::IncorrectCredentials => backoff::Error::Permanent(LoginError::IncorrectCredentials),
                    LoginError::CaptchaRequired { captcha_guid } => {
                        backoff::Error::Permanent(LoginError::CaptchaRequired { captcha_guid })
                    }
                    _ => {
                        warn!("Transient error happened: Trying again..");
                        backoff::Error::transient(error)
                    }
                })
        })
        .await?;

        info!("Login to Steam successfully.");
        // FIXME: This should work the same as login, because it can sometimes fail for no reason
        if self.user.parental_code.is_some() {
            parental_unlock(&self.client, &self.user).await?;
            info!("Parental unlock successfully.");
        }

        cache_resolve(self).await?;
        info!("Cached API Key successfully.");

        Ok(())
    }

    /// Add an authenticator to the account.
    /// Note that this makes various assumptions about the account.
    ///
    /// This function takes the `AddAuthenticatorStep` to help you automate the process of adding an authenticator to
    /// the account.
    ///
    /// You will first call this method with `AddAuthenticatorStep::InitialStep`. This requires the account to be
    /// already connected with a verified email address. After this step is finished, you will receive an email
    /// about the phone confirmation.
    ///
    /// Once you confirm it, you will call this method with `AddAuthenticatorStep::EmailConfirmation`.
    ///
    /// This will return a `AddAuthenticatorStep::MobileAuthenticatorFile` now, with your maFile inside the variant.
    /// For more complete example, you can check the CLI Tool, that performs the inclusion of an authenticator
    /// interactively.
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

        add_authenticator_to_account(&self.client, self.cached_data.read())
            .await
            .map(AddAuthenticatorStep::MobileAuth)
            .map_err(Into::into)
    }

    /// Finalize the authenticator process, enabling `SteamGuard` for the account.
    /// This method wraps up the whole process, finishing the registration of the phone number into the account.
    ///
    /// You **should** only call this method after saving your maFile, because otherwise you WILL lose access to your
    /// account.
    pub async fn finalize_authenticator(&self, mafile: &MobileAuthFile, sms_code: &str) -> Result<(), AuthError> {
        // The delay is that Steam need some seconds to catch up with the new phone number associated.
        let account_has_phone_now: bool = check_sms(&self.client, sms_code)
            .map_ok(|_| Delay::new(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)))
            .and_then(|_| account_has_phone(&self.client))
            .await?;

        if !account_has_phone_now {
            return Err(LinkerError::GeneralFailure("This should not happen.".to_string()).into());
        }

        info!("Successfully confirmed SMS code.");

        finalize(&self.client, self.cached_data.read(), mafile, sms_code)
            .await
            .map_err(Into::into)
    }

    pub async fn remove_authenticator(
        &self,
        revocation_code: &str,
        remove_authenticator_scheme: RemoveAuthenticatorScheme,
    ) -> Result<(), AuthError> {
        remove_authenticator(
            &self.client,
            self.cached_data.read(),
            revocation_code,
            remove_authenticator_scheme,
        )
        .await
    }

    /// Add a phone number into the account, and then checks it to make sure it has been added.
    /// Returns true if number was successfully added.
    async fn add_phone_number(&self, phone_number: &str) -> Result<bool, AuthError> {
        if !validate_phone_number(phone_number) {
            return Err(LinkerError::GeneralFailure(
                "Invalid phone number. Should be in format of: +(CountryCode)(AreaCode)(PhoneNumber). E.g \
                 +5511976914922"
                    .to_string(),
            )
            .into());
        }

        // Add the phone number to user account
        // The delay is that Steam need some seconds to catch up.
        let response = add_phone_to_account(&self.client, phone_number).await?;
        Delay::new(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)).await;

        Ok(response)
    }

    /// Fetch all confirmations available with the authenticator.
    pub async fn fetch_confirmations(&self) -> Result<Option<Confirmations>, AuthError> {
        // TODO: With details? Maybe we need to check if there is a need to gather more details.
        let steamid = self
            .cached_data
            .read()
            .steam_id()
            .expect("Failed to retrieve cached SteamID. Are you logged in?");

        confirmations_retrieve_all(&self.client, &self.user, steamid, false)
            .map_ok(|confs| confs.map(Confirmations::from))
            .err_into()
            .await
    }

    /// Accept or deny confirmations.
    ///
    /// # Panics
    /// Will panic if not logged in with [`SteamAuthenticator`] first.
    pub async fn process_confirmations<I>(
        &self,
        operation: ConfirmationMethod,
        confirmations: I,
    ) -> Result<(), AuthError>
    where
        I: IntoIterator<Item = Confirmation> + Send + Sync,
    {
        let steamid = self
            .cached_data
            .read()
            .steam_id()
            .expect("Failed to retrieve cached SteamID. Are you logged in?");

        confirmations_send(&self.client, &self.user, steamid, operation, confirmations)
            .await
            .map_err(Into::into)
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
        T: Serialize + Send + Sync,
    {
        self.client
            .request_with_session_guard(url, method, custom_headers, data)
            .await
    }

    pub fn dump_cookie(&self, steam_domain_host: &str, steam_cookie_name: &str) -> Option<String> {
        // TODO: Change domain and names to enums
        dump_cookies_by_name(&self.client.cookie_store.read(), steam_domain_host, steam_cookie_name)
    }
}

#[derive(Debug)]
pub struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub inner_http_client: Client,
    /// Cookie jar that manually handle cookies, because reqwest doesn't let us handle its
    /// cookies.
    pub cookie_store: Arc<RwLock<CookieJar>>,
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
        T: Serialize + Send + Sync,
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
        T: Serialize + Send + Sync,
    {
        let parsed_url = Url::parse(&url).unwrap();
        let mut header_map = custom_headers.unwrap_or_default();

        // Send cookies stored on jar, based on the domain that we are requesting
        let domain = &format!(".{}", parsed_url.host_str().unwrap());
        let domain_cookies = dump_cookies_by_domain(&self.cookie_store.read(), domain);
        header_map.insert(
            reqwest::header::COOKIE,
            domain_cookies.unwrap_or_default().parse().unwrap(),
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
        self.cookie_store = Arc::new(RwLock::new(CookieJar::new()));
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
        let user_agent = "Dalvik/2.1.0 (Linux; U; Android 9; Valve Steam App Version/3)";
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
            .cookie_store(true)
            .redirect(Policy::limited(5))
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
            cookie_store: Arc::new(RwLock::new(Self::init_cookie_jar())),
        }
    }
}

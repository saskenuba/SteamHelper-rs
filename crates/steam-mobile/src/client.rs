use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use backoff::future::retry;
use base64::Engine;
use cookie::Cookie;
use cookie::CookieJar;
use futures::TryFutureExt;
use futures_timer::Delay;
use parking_lot::RwLock;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use reqwest::Client;
use reqwest::IntoUrl;
use reqwest::Method;
use reqwest::Response;
use reqwest::Url;
use scraper::Html;
use serde::de::DeserializeOwned;
use serde::Serialize;
use steam_protobuf::ProtobufDeserialize;
use steam_protobuf::ProtobufSerialize;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::adapter::SteamCookie;
use crate::errors::AuthError;
use crate::errors::InternalError;
use crate::errors::LinkerError;
use crate::errors::MobileAuthFileError;
use crate::retry::login_retry_strategy;
use crate::user::AbsentMaFile;
use crate::user::MaFileState;
use crate::user::PendingMaFile;
use crate::user::PresentMaFile;
use crate::user::SteamUser;
use crate::utils::dump_cookies_by_domain;
use crate::utils::dump_cookies_by_domain_and_name;
use crate::utils::retrieve_header_location;
use crate::web_handler::api_key_register;
use crate::web_handler::api_key_revoke;
use crate::web_handler::cache_api_key;
use crate::web_handler::confirmation::Confirmation;
use crate::web_handler::confirmation::Confirmations;
use crate::web_handler::get_confirmations;
use crate::web_handler::login::login_and_store_cookies;
use crate::web_handler::send_confirmations;
use crate::web_handler::steam_guard_linker::account_has_phone;
use crate::web_handler::steam_guard_linker::add_authenticator_to_account;
use crate::web_handler::steam_guard_linker::add_phone_to_account;
use crate::web_handler::steam_guard_linker::check_email_confirmation;
use crate::web_handler::steam_guard_linker::check_sms;
use crate::web_handler::steam_guard_linker::finalize;
use crate::web_handler::steam_guard_linker::remove_authenticator;
use crate::web_handler::steam_guard_linker::twofactor_status;
use crate::web_handler::steam_guard_linker::validate_phone_number;
use crate::web_handler::steam_guard_linker::ActivationCodeDelivery;
use crate::web_handler::steam_guard_linker::AddAuthenticatorStep;
use crate::web_handler::steam_guard_linker::PendingAuthenticator;
use crate::web_handler::steam_guard_linker::QueryStatusResponse;
use crate::web_handler::steam_guard_linker::RemoveAuthenticatorScheme;
use crate::web_handler::steam_guard_linker::STEAM_ADD_PHONE_CATCHUP_SECS;
use crate::ConfirmationAction;
use crate::MobileAuthFile;
use crate::SteamCache;
use crate::STEAM_COMMUNITY_HOST;

/// Main authenticator. We use it to spawn and act as our "mobile" client.
/// Responsible for accepting/denying trades, and some other operations that may or not be related
/// to mobile operations.
///
/// # Example: Fetch mobile notifications
///
/// ```rust
/// use steam_mobile::SteamAuthenticator;
/// use steam_mobile::user::SteamUser;
/// ```
#[derive(Debug)]
pub struct SteamAuthenticator<AuthState, MaFileState> {
    inner: InnerAuthenticator<MaFileState>,
    auth_level: PhantomData<AuthState>,
}

#[derive(Debug)]
struct InnerAuthenticator<MaFileState> {
    pub(crate) client: MobileClient,
    pub(crate) user: SteamUser<MaFileState>,
    /// Always `Some` once [`Authenticated`]. Never mutated through a shared reference, so it needs no lock.
    pub(crate) cache: Option<SteamCache>,
}

/// A successfully logged-in state. Many assumptions are made on this state.
#[derive(Clone, Copy, Debug)]
pub struct Authenticated;

/// A pending authorization state.
#[derive(Clone, Copy, Debug)]
pub struct Unauthenticated;

impl<AuthState, M> SteamAuthenticator<AuthState, M> {
    const fn client(&self) -> &MobileClient {
        &self.inner.client
    }
    const fn user(&self) -> &SteamUser<M> {
        &self.inner.user
    }

    /// Moves to another MaFile state, keeping the session.
    fn map_user<N>(self, f: impl FnOnce(SteamUser<M>) -> SteamUser<N>) -> SteamAuthenticator<AuthState, N> {
        SteamAuthenticator {
            inner: InnerAuthenticator {
                client: self.inner.client,
                user: f(self.inner.user),
                cache: self.inner.cache,
            },
            auth_level: PhantomData,
        }
    }
}

impl<M: MaFileState> SteamAuthenticator<Unauthenticated, M> {
    /// Returns current user API Key.
    ///
    /// Will return `None` if you are not logged in.
    #[must_use]
    pub fn new(user: SteamUser<M>) -> Self {
        Self {
            inner: InnerAuthenticator {
                client: MobileClient::default(),
                user,
                cache: None,
            },
            auth_level: PhantomData::<Unauthenticated>,
        }
    }
    /// Log on into Steam website and populates the inner client with cookies for the Steam Store,
    /// Steam community and Steam help domains.
    ///
    /// Automatically unlocks parental control if user uses it, but it need to be included inside
    /// the [SteamUser] builder.
    ///
    /// The mobile client also has a very simple exponential retry strategy for errors that are *probably*
    /// caused by fast requests, so we retry it. For errors such as bad credentials, or inserting captcha
    /// the proper errors are raised by `AuthError`.
    ///
    /// Also caches the API Key, if the user wants to use it for any operation later.
    ///
    /// The cookies are inside the [MobileClient] inner cookie storage.
    ///
    /// If the MaFile lacks the device ID that confirmations need, it is fetched from Steam, and login fails if Steam
    /// has none.
    pub async fn login(self) -> Result<SteamAuthenticator<Authenticated, M>, AuthError> {
        let mut user = self.inner.user;
        let client = self.inner.client;

        // FIXME: Add more permanent errors, such as bad credentials
        let mut cache = retry(login_retry_strategy(), || async {
            login_and_store_cookies(&client, &user)
                .await
                .map_err(|error| match error {
                    e => {
                        warn!("Permanent error happened.");
                        warn!("{e}");
                        backoff::Error::permanent(e)
                    }
                })
        })
        .await?;
        info!("Login to Steam successfully.");

        // FIXME: This should work the same as login, because it can sometimes fail for no reason
        // if user.parental_code.is_some() {
        //     parental_unlock(client, user).await?;
        //     info!("Parental unlock successfully.");
        // }

        // Some maFiles lack the device ID that confirmations need, but Steam remembers the one the authenticator was
        // added with.
        if user.active_mafile().is_some_and(|mafile| mafile.device_id().is_none()) {
            let status = twofactor_status(&client, &cache).await?;
            if status.device_identifier.is_empty() {
                return Err(MobileAuthFileError::GeneralFailure(
                    "The maFile has no device ID, and Steam has none for this account's authenticator.".to_string(),
                )
                .into());
            }
            if let Some(mafile) = user.active_mafile_mut() {
                mafile.set_device_id(status.device_identifier);
                info!("Recovered the maFile's missing device ID from Steam.");
            }
        }

        let api_key = cache_api_key(&client).await;
        if let Some(api_key) = api_key {
            cache.set_api_key(Some(api_key));
            info!("Cached API Key successfully.");
        }

        Ok(SteamAuthenticator {
            inner: InnerAuthenticator {
                client,
                user,
                cache: Some(cache),
            },
            auth_level: PhantomData,
        })
    }
}

impl<M> SteamAuthenticator<Authenticated, M>
where
    M: Send + Sync,
{
    fn cache(&self) -> &SteamCache {
        self.inner
            .cache
            .as_ref()
            .expect("Cache is always set in the Authenticated state.")
    }

    /// Returns account's API Key, if authenticator managed to cache it.
    pub fn api_key(&self) -> Option<String> {
        self.cache().api_key().map(ToString::to_string)
    }

    /// Returns this account SteamGuard information.
    pub async fn steam_guard_status(&self) -> Result<QueryStatusResponse, AuthError> {
        twofactor_status(self.client(), self.cache()).await.map_err(Into::into)
    }

    /// Add an authenticator to the account.
    /// Note that this makes various assumptions about the account.
    ///
    /// The first argument is an enum of  `AddAuthenticatorStep` to help you automate the process of adding an
    /// authenticator to the account.
    ///
    /// First call this method with `AddAuthenticatorStep::InitialStep`. This requires the account to be
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
        let user_has_phone_registered = account_has_phone(self.client()).await?;
        debug!("Has phone registered? {:?}", user_has_phone_registered);

        if !user_has_phone_registered && current_step == AddAuthenticatorStep::InitialStep {
            self.add_phone_number(phone_number).await?;
            debug!("Phone number added to account.");

            return Ok(AddAuthenticatorStep::EmailConfirmation);
        }

        // Signal steam that user confirmed email
        // If user already has a phone, calling email confirmation will result in a error finalizing the auth process.
        if !user_has_phone_registered {
            if !check_email_confirmation(self.client()).await? {
                return Err(LinkerError::EmailNotConfirmed.into());
            }
            debug!("Email confirmation signal sent.");
        }

        add_authenticator_to_account(self.client(), self.cache())
            .await
            .map(|pending| AddAuthenticatorStep::MobileAuth(pending.mafile))
            .map_err(Into::into)
    }

    /// Finalize the authenticator process, enabling `SteamGuard` for the account.
    /// This method wraps up the whole process, finishing the registration of the phone number into the account.
    ///
    /// * EXTREMELY IMPORTANT *
    ///
    /// Call this method **ONLY** after saving your maFile, because otherwise you WILL lose access to your
    /// account.
    pub async fn finalize_authenticator(&self, mafile: &MobileAuthFile, sms_code: &str) -> Result<(), AuthError> {
        if !check_sms(self.client(), sms_code).await? {
            return Err(LinkerError::BadSMSCode.into());
        }

        // Steam needs some seconds to catch up with the new phone number associated.
        Delay::new(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)).await;

        let account_has_phone_now = account_has_phone(self.client()).await?;
        if !account_has_phone_now {
            return Err(LinkerError::GeneralFailure("This should not happen.".to_string()).into());
        }

        info!("Successfully confirmed SMS code.");

        finalize(self.client(), self.cache(), mafile, sms_code)
            .await
            .map_err(Into::into)
    }

    /// Remove an authenticator from a Steam Account.
    ///
    /// Sets account to use `SteamGuard` email confirmation codes or even remove it completely.
    pub async fn remove_authenticator(
        &self,
        revocation_code: &str,
        remove_authenticator_scheme: RemoveAuthenticatorScheme,
    ) -> Result<(), AuthError> {
        remove_authenticator(
            self.client(),
            self.cache(),
            revocation_code,
            remove_authenticator_scheme,
        )
        .await
        .map_err(Into::into)
    }

    /// Add a phone number into the account, erroring if Steam rejects it.
    async fn add_phone_number(&self, phone_number: &str) -> Result<(), AuthError> {
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
        if !add_phone_to_account(self.client(), phone_number).await? {
            return Err(LinkerError::PhoneNumberRejected.into());
        }
        Delay::new(Duration::from_secs(STEAM_ADD_PHONE_CATCHUP_SECS)).await;

        Ok(())
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
    ) -> Result<Response, InternalError>
    where
        T: Serialize + Send + Sync,
    {
        self.client()
            .request_with_session_guard(url, method, custom_headers, data, None::<&str>)
            .await
    }

    #[allow(missing_docs)]
    pub fn dump_cookie(&self, steam_domain_host: &str, steam_cookie_name: &str) -> Option<String> {
        dump_cookies_by_domain_and_name(&self.client().cookie_store.read(), steam_domain_host, steam_cookie_name)
    }
}

impl SteamAuthenticator<Authenticated, AbsentMaFile> {
    /// Adds an authenticator to the account, without adding a phone number to it.
    ///
    /// Steam sends an activation code by SMS if the account already has a phone, or by email otherwise; the returned
    /// [`ActivationCodeDelivery`] says which. The authenticator moves to the [`PendingMaFile`] state, where
    /// [`SteamAuthenticator::finalize_pending`] takes that code.
    ///
    /// * EXTREMELY IMPORTANT *
    ///
    /// Save the pending [`SteamAuthenticator::mafile`] before finalizing, because otherwise you WILL lose access to
    /// your account.
    pub async fn begin_add_authenticator(
        self,
    ) -> Result<(SteamAuthenticator<Authenticated, PendingMaFile>, ActivationCodeDelivery), AuthError> {
        let PendingAuthenticator { mafile, code_delivery } =
            add_authenticator_to_account(self.client(), self.cache()).await?;
        let authenticator = self.map_user(|user| user.with_pending_mafile(mafile));
        Ok((authenticator, code_delivery))
    }
}

impl SteamAuthenticator<Authenticated, PendingMaFile> {
    /// The MaFile of the authenticator being added. Save it before finalizing.
    pub fn mafile(&self) -> &MobileAuthFile {
        self.user().mafile()
    }

    /// Finalizes the authenticator with the activation code Steam sent by SMS or email, enabling `SteamGuard` for the
    /// account.
    ///
    /// On success, the authenticator moves to the [`PresentMaFile`] state, ready for confirmations. On failure, such as
    /// a wrong code, it is handed back with the error, so you can try again.
    pub async fn finalize_pending(
        self,
        activation_code: &str,
    ) -> Result<SteamAuthenticator<Authenticated, PresentMaFile>, (Self, AuthError)> {
        match finalize(self.client(), self.cache(), self.user().mafile(), activation_code).await {
            Ok(()) => Ok(self.map_user(SteamUser::into_present)),
            Err(e) => Err((self, e.into())),
        }
    }
}

impl SteamAuthenticator<Authenticated, PresentMaFile> {
    /// Registers a new Steam Web API key for this account, and caches it.
    ///
    /// Steam requires a mobile confirmation for this. Only the confirmation tied to this registration request is
    /// accepted; any other pending API key confirmation is left untouched, since it may have been requested by
    /// someone else.
    ///
    /// Returns the cached key without registering if one is already known.
    pub async fn register_api_key(&mut self) -> Result<String, AuthError> {
        if let Some(api_key) = self.api_key() {
            return Ok(api_key);
        }

        let steamid = self.cache().steam_id();
        let api_key = api_key_register(self.client(), self.user(), steamid).await?;
        self.inner
            .cache
            .as_mut()
            .expect("Cache is always set in the Authenticated state.")
            .set_api_key(Some(api_key.clone()));

        Ok(api_key)
    }

    /// Revokes the account's current Steam Web API key and clears it from the cache.
    ///
    /// After this, [`Self::api_key`] returns `None` and [`Self::register_api_key`] will register a fresh key,
    /// accepting the mobile confirmation Steam requires.
    ///
    /// Revoking succeeds even if no key was registered, since Steam treats that as a no-op.
    pub async fn revoke_api_key(&mut self) -> Result<(), AuthError> {
        api_key_revoke(self.client()).await?;
        self.inner
            .cache
            .as_mut()
            .expect("Cache is always set in the Authenticated state.")
            .set_api_key(None);

        Ok(())
    }

    /// The maFile this authenticator was logged in with.
    ///
    /// If the file lacked a device ID, this one has the device ID recovered from Steam at login, so it can be saved
    /// back.
    pub fn mafile(&self) -> &MobileAuthFile {
        self.user().mafile()
    }

    /// Fetch all confirmations available with the authenticator.
    pub async fn fetch_confirmations(&self) -> Result<Confirmations, AuthError> {
        let steamid = self.cache().steam_id();
        let secret = (&self.inner.user).identity_secret();
        let device_id = self.user().device_id();

        get_confirmations(self.client(), secret, device_id, steamid)
            .err_into()
            .await
    }

    /// Fetches confirmations and process them.
    ///
    /// `f` is a function which you can use it to filter confirmations at the moment of the query.
    pub async fn handle_confirmations<'a, 'b, F>(&self, operation: ConfirmationAction, f: F) -> Result<(), AuthError>
    where
        F: Fn(Confirmations) -> Box<dyn Iterator<Item = Confirmation> + Send> + Send,
    {
        let confirmations = self.fetch_confirmations().await?;
        if !confirmations.is_empty() {
            self.process_confirmations(operation, f(confirmations)).await
        } else {
            Ok(())
        }
    }

    /// Accept or deny confirmations.
    ///
    /// # Panics
    /// Will panic if not logged in with [`SteamAuthenticator`] first.
    pub async fn process_confirmations<I>(
        &self,
        operation: ConfirmationAction,
        confirmations: I,
    ) -> Result<(), AuthError>
    where
        I: IntoIterator<Item = Confirmation> + Send,
    {
        let steamid = self.cache().steam_id();

        send_confirmations(
            self.client(),
            self.user().identity_secret(),
            self.user().device_id(),
            steamid,
            operation,
            confirmations,
        )
        .await
        .map_err(Into::into)
    }
}

pub struct MobileClient {
    /// Standard HTTP Client to make requests.
    pub inner_http_client: Client,
    /// Cookie jar that manually handle cookies, because reqwest doesn't let us handle its cookies.
    pub cookie_store: Arc<RwLock<CookieJar>>,
}

impl Debug for MobileClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Cookies hold session tokens, so they are left out.
        f.debug_struct("MobileClient").finish_non_exhaustive()
    }
}

/// Strips the query string, which may carry tokens or confirmation hashes, so the URL is safe to log.
fn loggable_url(url: &Url) -> String {
    let mut url = url.clone();
    url.set_query(None);
    url.to_string()
}

impl MobileClient {
    pub(crate) fn get_cookie_value(&self, domain: &str, name: &str) -> Option<String> {
        dump_cookies_by_domain_and_name(&self.cookie_store.read(), domain, name)
    }
    pub(crate) fn set_cookie_value(&self, cookie: Cookie<'static>) {
        self.cookie_store.write().add_original(cookie);
    }

    pub(crate) async fn request_proto<INPUT, OUTPUT>(
        &self,
        url: impl IntoUrl + Send,
        method: Method,
        proto_message: INPUT,
        _token: Option<&str>,
    ) -> Result<OUTPUT, InternalError>
    where
        INPUT: ProtobufSerialize,
        OUTPUT: ProtobufDeserialize<Output = OUTPUT> + Debug,
    {
        let url = url.into_url().unwrap();
        debug!("Request url: {}", url);
        let request_builder = self.inner_http_client.request(method.clone(), url);

        let req = if method == Method::GET {
            let encoded = base64::engine::general_purpose::URL_SAFE.encode(proto_message.to_bytes().unwrap());
            let parameters = &[("input_protobuf_encoded", encoded)];
            request_builder.query(parameters)
        } else if method == Method::POST {
            let encoded = base64::engine::general_purpose::STANDARD.encode(proto_message.to_bytes().unwrap());
            let form = reqwest::multipart::Form::new().text("input_protobuf_encoded", encoded);
            request_builder.multipart(form)
        } else {
            return Err(InternalError::GeneralFailure("Unsupported Method".to_string()));
        };

        let response = req.send().await?;
        debug!("Response status: {}", response.status());

        let res_bytes = response.bytes().await?;
        OUTPUT::from_bytes(res_bytes).map_or_else(
            |_| {
                error!("Failed deserializing {}", std::any::type_name::<OUTPUT>());
                Err(InternalError::GeneralFailure("asdfd".to_string()))
            },
            |res| {
                debug!("Decoded {}", std::any::type_name::<OUTPUT>());
                Ok(res)
            },
        )
    }

    /// Wrapper to make requests while preemptively checking if the session is still valid.
    pub(crate) async fn request_with_session_guard<T, QP, U>(
        &self,
        url: U,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
        query_params: Option<QP>,
    ) -> Result<Response, InternalError>
    where
        T: Serialize + Send,
        QP: Serialize + Send,
        U: IntoUrl + Send,
    {
        // We check preemptively if the session is still working.
        if self.session_is_expired().await? {
            warn!("Session was lost. Trying to reconnect.");
            unimplemented!()
        };

        self.request(url, method, custom_headers, data, query_params)
            .err_into()
            .await
    }
    pub(crate) async fn request_with_session_guard_and_decode<T, QP, OUTPUT>(
        &self,
        url: String,
        method: Method,
        custom_headers: Option<HeaderMap>,
        data: Option<T>,
        query_params: Option<QP>,
    ) -> Result<OUTPUT, InternalError>
    where
        T: Serialize + Send + Sync,
        QP: Serialize + Send + Sync,
        OUTPUT: DeserializeOwned,
    {
        let req = self
            .request_with_session_guard(url, method, custom_headers, data.as_ref(), query_params)
            .await?;

        let response_body = req
            .text()
            .inspect_ok(|s| {
                debug!("{} response: {} bytes", std::any::type_name::<OUTPUT>(), s.len());
            })
            .await?;

        serde_json::from_str::<OUTPUT>(&response_body).map_err(InternalError::DeserializationError)
    }

    /// Simple wrapper to allow generic requests to be made.
    pub(crate) async fn request<T, QS, U>(
        &self,
        url: U,
        method: Method,
        headers: Option<HeaderMap>,
        form_data: Option<T>,
        query_params: QS,
    ) -> Result<Response, InternalError>
    where
        QS: Serialize + Send,
        T: Serialize + Send,
        U: IntoUrl + Send,
    {
        let parsed_url = url
            .into_url()
            .map_err(|_| InternalError::GeneralFailure("Couldn't parse passed URL. Insert a valid one.".to_string()))?;
        let mut header_map = headers.unwrap_or_default();

        let domain_cookies = dump_cookies_by_domain(&self.cookie_store.read(), parsed_url.host_str().unwrap());
        header_map.insert(
            reqwest::header::COOKIE,
            domain_cookies.unwrap_or_default().parse().unwrap(),
        );

        let req_builder = self
            .inner_http_client
            .request(method, parsed_url)
            .headers(header_map)
            .query(&query_params);

        let request = match form_data {
            None => req_builder.build().unwrap(),
            Some(data) => match serde_urlencoded::to_string(data) {
                Ok(body) => {
                    debug!("Request body: {} bytes", body.len());
                    req_builder
                        .header(
                            CONTENT_TYPE,
                            HeaderValue::from_static("application/x-www-form-urlencoded; charset=UTF-8"),
                        )
                        .body(body)
                        .build()
                        .expect("Safe to unwrap.")
                }
                Err(err) => {
                    return Err(InternalError::GeneralFailure(format!(
                        "Failed to serialize body: {err}"
                    )))
                }
            },
        };
        debug!("{} {}", request.method(), loggable_url(request.url()));

        let res = self.inner_http_client.execute(request).err_into().await;
        if let Ok(ref response) = res {
            debug!("Response status: {:?}", response.status());

            let mut cookie_jar = self.cookie_store.write();
            for cookie in response.cookies() {
                let mut our_cookie = SteamCookie::from(cookie);
                let host = response.url().host().expect("Safe.").to_string();
                our_cookie.set_domain(host);

                trace!(
                    "New cookie from: {:?}, name: {}",
                    our_cookie.domain(),
                    our_cookie.name()
                );
                cookie_jar.add_original(our_cookie.deref().clone());
            }
        }
        res
    }

    pub(crate) async fn request_and_decode<T, OUTPUT, QS, U>(
        &self,
        url: U,
        method: Method,
        headers: Option<HeaderMap>,
        form_data: Option<T>,
        query_params: QS,
    ) -> Result<OUTPUT, InternalError>
    where
        OUTPUT: DeserializeOwned,
        QS: Serialize + Send + Sync,
        T: Serialize + Send + Sync,
        U: IntoUrl + Send,
    {
        let resp = self.request(url, method, headers, form_data, query_params).await?;
        let response_body = resp
            .text()
            .inspect_ok(|s| {
                debug!("{} response: {} bytes", std::any::type_name::<OUTPUT>(), s.len());
            })
            .await?;

        serde_json::from_str::<OUTPUT>(&response_body).map_err(InternalError::DeserializationError)
    }

    /// Checks if session is expired by parsing the the redirect URL for "steamobile:://lostauth"
    /// or a path that starts with "/login".
    ///
    /// This is the most reliable way to find out, since we check the session by requesting our
    /// account page at Steam Store, which is not going to be deprecated anytime soon.
    async fn session_is_expired(&self) -> Result<bool, InternalError> {
        let account_url = format!("{}/account", crate::STEAM_STORE_BASE);

        // FIXME: Not sure if we should request from client directly
        let response = self
            .request(account_url, Method::HEAD, None, None::<u8>, None::<u8>)
            .await?;

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
    pub(crate) async fn get_html<T, QS>(
        &self,
        url: T,
        headers: Option<HeaderMap>,
        query: Option<QS>,
    ) -> Result<Html, InternalError>
    where
        T: IntoUrl + Send,
        QS: Serialize + Send,
    {
        self.request_with_session_guard(url, Method::GET, headers, None::<&str>, query)
            .and_then(|r| r.text().err_into())
            .await
            .map(|s| Html::parse_document(&s))
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

        Client::builder()
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

/// Compile-time check that the public futures can be spawned on a multi-threaded runtime.
#[cfg(test)]
#[allow(dead_code, unused_must_use, clippy::let_underscore_future)]
fn assert_public_futures_are_send(
    authenticator: &mut SteamAuthenticator<Authenticated, PresentMaFile>,
    unauthenticated: SteamAuthenticator<Unauthenticated, PresentMaFile>,
    without_mafile: SteamAuthenticator<Authenticated, AbsentMaFile>,
    pending: SteamAuthenticator<Authenticated, PendingMaFile>,
    mafile: &MobileAuthFile,
) {
    fn is_send<T: Send>(_: T) {}

    is_send(unauthenticated.login());
    is_send(authenticator.steam_guard_status());
    is_send(authenticator.add_authenticator(AddAuthenticatorStep::InitialStep, ""));
    is_send(authenticator.finalize_authenticator(mafile, ""));
    is_send(without_mafile.begin_add_authenticator());
    is_send(pending.finalize_pending(""));
    is_send(authenticator.remove_authenticator("", RemoveAuthenticatorScheme::ReturnToEmailCodes));
    is_send(authenticator.fetch_confirmations());
    is_send(authenticator.process_confirmations(ConfirmationAction::Accept, Vec::new()));
    is_send(authenticator.request_custom_endpoint(String::new(), Method::GET, None, None::<u8>));
    is_send(authenticator.register_api_key());
}

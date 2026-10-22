use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use backoff::future::retry;
use base64::Engine;
use cookie::Cookie;
use futures::TryFutureExt;
use futures_timer::Delay;
use parking_lot::RwLock;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use reqwest::Client;
use reqwest::ClientBuilder;
use reqwest::IntoUrl;
use reqwest::Method;
use reqwest::Response;
use reqwest::StatusCode;
use reqwest::Url;
use scraper::Html;
use serde::de::DeserializeOwned;
use serde::Serialize;
#[cfg(feature = "cm")]
use steam_client::client::User;
#[cfg(feature = "cm")]
use steam_client::handle::SteamHandle;
use steam_language::enums::EResult;
use steam_protobuf::ProtobufDeserialize;
use steam_protobuf::ProtobufSerialize;
use steamid_parser::SteamID;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::cookie_jar::cookie_header;
use crate::cookie_jar::ClientJar;
use crate::cookie_jar::CookieJar;
use crate::errors::AuthError;
use crate::errors::InternalError;
use crate::errors::LinkerError;
use crate::errors::MobileAuthFileError;
use crate::retry::login_retry_strategy;
#[cfg(feature = "cm")]
use crate::session::login_secure_cookies;
#[cfg(feature = "cm")]
use crate::session::session_id_cookies;
#[cfg(feature = "cm")]
use crate::session::ClientLogin;
use crate::session::LoginMode;
use crate::user::AbsentMaFile;
use crate::user::MaFileState;
use crate::user::PendingMaFile;
use crate::user::PresentMaFile;
use crate::user::SteamUser;
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
use crate::AccessToken;
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

    /// Logs in to the Steam websites through `handle`, a Steam client logged on to this account with steam-client-lib,
    /// instead of with a password: the client generates an access token (`Authentication.GenerateAccessTokenForApp`
    /// over its CM connection), and the session takes it, with the client's SteamID. Feature `cm`.
    ///
    /// Sets the session cookies, `steamLoginSecure` (the SteamID and the token) and a new random `sessionid`, on the
    /// Steam Community, Store, Help and Checkout hosts, then checks Steam takes them. As [`SteamAuthenticator::login`]
    /// does, it recovers a device ID the maFile lacks and caches the API key.
    ///
    /// The authenticator keeps `handle`, and so keeps the client running: a spawned client stops only once every
    /// handle to it is dropped, this one too, or it logs off. It renews the token through it, which lasts about a day:
    /// the first request with the session once the token expires within an hour ([`SteamAuthenticator::renew_before`]
    /// sets another margin) gets a new one first, and keeps the `sessionid`. That request waits for the client's
    /// answer, up to the client's call deadline (30 seconds); requests coming meanwhile go at once with the current
    /// token while it is valid, and only those whose token has expired wait for that answer. Only early: a request
    /// that fails is never retried. If a renewal fails, such as while the client is connecting again or after it
    /// stopped, requests go on with the current token while it is valid, and the client is asked again 30 seconds
    /// after the failure at the soonest; meanwhile a request whose token has expired fails at once with
    /// [`InternalError::ClientToken`], without being sent. An expired token is never sent. The token's lifetime is
    /// read from its own claims and counted from when the client was asked for it, so this machine's clock being off
    /// doesn't matter.
    ///
    /// ```no_run
    /// # use steam_client::{client::User, handle::SteamHandle};
    /// # use steam_mobile::{MobileAuthFile, SteamAuthenticator, user::SteamUser};
    /// # async fn f(mafile: MobileAuthFile, password: String, handle: SteamHandle<User>) {
    /// let user = SteamUser::new("username".to_string(), password).with_mafile(mafile);
    /// let authenticator = SteamAuthenticator::new(user)
    ///     .login_with_client(handle.clone())
    ///     .await
    ///     .unwrap();
    /// authenticator.fetch_confirmations().await.unwrap();
    /// # }
    /// ```
    #[cfg(feature = "cm")]
    pub async fn login_with_client(
        self,
        handle: SteamHandle<User>,
    ) -> Result<SteamAuthenticator<Authenticated, M>, AuthError> {
        let steam_id = handle.steam_id();
        let (login, access_token) = ClientLogin::start(Box::new(handle), Instant::now).await?;
        let client = self.inner.client;
        client.set_session_cookies(steam_id, &access_token);
        let cache = SteamCache::with_client(steam_id, access_token, login);

        if client.session_is_expired().await? {
            return Err(InternalError::SessionExpired.into());
        }
        info!("Logged in to Steam through the Steam client.");

        finish_login(client, self.inner.user, cache).await
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
        let user = self.inner.user;
        let client = self.inner.client;

        // FIXME: Add more permanent errors, such as bad credentials
        let cache = retry(login_retry_strategy(), || async {
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

        finish_login(client, user, cache).await
    }
}

/// What every login does once Steam took the session: completes the maFile, and caches the API key.
async fn finish_login<M: MaFileState>(
    client: MobileClient,
    mut user: SteamUser<M>,
    mut cache: SteamCache,
) -> Result<SteamAuthenticator<Authenticated, M>, AuthError> {
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

    /// The account's SteamID, from the login.
    pub fn steam_id(&self) -> SteamID {
        self.cache().steamid
    }

    /// Returns account's API Key, if authenticator managed to cache it.
    pub fn api_key(&self) -> Option<String> {
        self.cache().api_key().map(ToString::to_string)
    }

    /// Sets how long before its access token expires a login through a Steam client
    /// ([`SteamAuthenticator::login_with_client`]) renews it: an hour by default. Feature `cm`.
    ///
    /// The margin is how long the session outlasts a client that can't renew it, such as while the client can't reach
    /// Steam: requests go on with the current token till it expires, and fail unsent after. A longer one renews more
    /// often, as Steam issues tokens for about a day; one of the token's lifetime or more makes a request renew it
    /// whenever 30 seconds passed since the last renewal, the least time between two.
    ///
    /// A password login's token is never renewed, so on one this changes nothing.
    #[cfg(feature = "cm")]
    #[must_use]
    pub fn renew_before(mut self, margin: Duration) -> Self {
        if let Some(LoginMode::Client(login)) = self.inner.cache.as_mut().map(|cache| &mut cache.mode) {
            login.set_renew_before(margin);
        }
        self
    }

    /// The session's access token, for Steam's Web API calls that take `access_token=`.
    ///
    /// Call it for each request instead of keeping the value: a login through a Steam client (feature `cm`) first
    /// renews the token if it is due, as every request with the session does, which is why this can wait for the
    /// client and fail ([`InternalError::ClientToken`], only once the current token has expired); a kept one stops
    /// working when it expires. A password login's token comes as it is.
    pub async fn access_token(&self) -> Result<AccessToken, InternalError> {
        self.renew_session_if_expiring().await?;
        Ok(AccessToken::new(self.cache().access_token()))
    }

    /// Renews the session's access token through the Steam client of a login through it (feature `cm`), if it is due:
    /// what every request with the session does first, before it reads the token or sends the cookies. Password
    /// logins have nothing to renew.
    async fn renew_session_if_expiring(&self) -> Result<(), InternalError> {
        self.renew_session_at(Instant::now).await
    }

    /// [`Self::renew_session_if_expiring`], with `now` reading the monotonic clock: the tests set it.
    #[cfg_attr(not(feature = "cm"), allow(unused_variables))]
    async fn renew_session_at(&self, now: impl Fn() -> Instant + Send) -> Result<(), InternalError> {
        match &self.cache().mode {
            LoginMode::Password { .. } => Ok(()),
            #[cfg(feature = "cm")]
            LoginMode::Client(login) => {
                login
                    .renew_if_due(now, |access_token| self.swap_access_token(access_token))
                    .await
            }
        }
    }

    /// Replaces the session's access token with a new one Steam issued to the same account, in the cache and in the
    /// `steamLoginSecure` cookies, and keeps the `sessionid`. Each is swapped under a short lock, never held across a
    /// request.
    #[cfg(feature = "cm")]
    fn swap_access_token(&self, access_token: String) {
        let steam_id = self.cache().steamid;
        {
            let mut jar = self.client().cookie_store.write();
            for cookie in login_secure_cookies(steam_id, &access_token) {
                jar.store_cookie(&cookie);
            }
        }
        self.cache().set_access_token(access_token);
    }

    /// Returns this account SteamGuard information.
    pub async fn steam_guard_status(&self) -> Result<QueryStatusResponse, AuthError> {
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
        self.client()
            .request_with_session_guard(url, method, custom_headers, data, None::<&str>)
            .await
    }

    /// The value of the cookie `steam_cookie_name` on the host `steam_domain_host`, or `None` if the session has no
    /// such cookie.
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
        self.renew_session_if_expiring().await?;
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
        if let Err(e) = self.renew_session_if_expiring().await {
            return Err((self, e.into()));
        }
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

        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
        self.renew_session_if_expiring().await?;
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
    /// Standard HTTP Client to make requests. Its cookie store is `cookie_store`.
    pub inner_http_client: Client,
    /// The session's cookies, the only jar: [`MobileClient::request`] sends them, and the HTTP client sends them on
    /// redirects and stores every response's Set-Cookie here (see [`ClientJar`]).
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

/// The header Steam's Web API reports a call's [`steam_language::enums::EResult`] in.
const X_ERESULT: &str = "x-eresult";
/// How much of an error page goes into [`InternalError::HttpStatus`].
const ERROR_BODY_SNIPPET_CHARS: usize = 200;

/// Passes a 2xx response through, and turns any other into an error before its body is parsed.
///
/// Only for responses whose body is parsed as a success: callers that inspect redirects or error bodies themselves
/// use [`MobileClient::request`] directly.
async fn ensure_success(response: Response) -> Result<Response, InternalError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let url = response.url().clone();
    // An error page, so it holds nothing of ours; only its start is kept.
    let body = response.text().await.unwrap_or_default();
    Err(status_error(status, &url, &body))
}

/// The error for a response that failed with `status`.
fn status_error(status: StatusCode, url: &Url, body: &str) -> InternalError {
    let url = loggable_url(url);
    if status == StatusCode::TOO_MANY_REQUESTS {
        return InternalError::TooManyRequests { url };
    }
    InternalError::HttpStatus {
        status,
        url,
        body: body_snippet(body),
    }
}

/// The start of an error page, whitespace collapsed.
fn body_snippet(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.char_indices().nth(ERROR_BODY_SNIPPET_CHARS) {
        Some((end, _)) => format!("{}…", &collapsed[..end]),
        None => collapsed,
    }
}

/// Fails if the `x-eresult` header reports anything but OK. Responses without it, or with one that isn't a number,
/// pass, as they did before it was checked.
fn eresult_check(header: Option<&HeaderValue>, url: &Url) -> Result<(), InternalError> {
    let Some(eresult) = header.and_then(|value| value.to_str().ok()?.trim().parse::<i32>().ok()) else {
        return Ok(());
    };
    if eresult == EResult::OK as i32 {
        return Ok(());
    }
    Err(InternalError::EResult {
        eresult,
        url: loggable_url(url),
    })
}

impl MobileClient {
    pub(crate) fn get_cookie_value(&self, domain: &str, name: &str) -> Option<String> {
        dump_cookies_by_domain_and_name(&self.cookie_store.read(), domain, name)
    }
    pub(crate) fn set_cookie_value(&self, cookie: Cookie<'static>) {
        self.cookie_store.write().store_cookie(&cookie);
    }

    /// Sets the cookies of a new session on every Steam host: `steamLoginSecure`, and a new `sessionid`.
    #[cfg(feature = "cm")]
    fn set_session_cookies(&self, steam_id: SteamID, access_token: &str) {
        store_session_cookies(&mut self.cookie_store.write(), steam_id, access_token);
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

        let response = ensure_success(response).await?;
        eresult_check(response.headers().get(X_ERESULT), response.url())?;

        let res_bytes = response.bytes().await?;
        OUTPUT::from_bytes(res_bytes).map_or_else(
            |error| {
                error!("Failed deserializing {}", std::any::type_name::<OUTPUT>());
                Err(InternalError::GeneralFailure(format!(
                    "Failed to decode the {} response: {error}",
                    std::any::type_name::<OUTPUT>()
                )))
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
            warn!("The web session expired.");
            return Err(InternalError::SessionExpired);
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

        let response_body = ensure_success(req)
            .await?
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

        // Always set, empty when the jar has none for the host. The HTTP client then leaves its cookie store (the same
        // jar) out of this request, and asks it on a redirect.
        let cookies = cookie_header(&self.cookie_store.read(), parsed_url.host_str().unwrap());
        header_map.insert(
            reqwest::header::COOKIE,
            cookies.unwrap_or_else(|| HeaderValue::from_static("")),
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

        // the Set-Cookie of each response, redirects included, is stored by the HTTP client's cookie store
        let res = self.inner_http_client.execute(request).err_into().await;
        if let Ok(ref response) = res {
            debug!("Response status: {:?}", response.status());
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
        let response_body = ensure_success(resp)
            .await?
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
            // relative to the page, when it has no host
            let redirect = response.url().join(location).map_err(|error| {
                InternalError::GeneralFailure(format!("Steam redirected to an invalid URL: {error}"))
            })?;
            return Ok(Self::url_expired_check(&redirect));
        }
        Ok(false)
    }

    /// If url is redirecting to '/login' or lostauth, returns true
    fn url_expired_check(redirect_url: &Url) -> bool {
        redirect_url.host_str() == Some("lostauth") || redirect_url.path().starts_with("/login")
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

    /// Mobile cookies that makes us look like the mobile app, as (name, value) on the Steam Community.
    const STANDARD_MOBILE_COOKIES: [(&'static str, &'static str); 3] = [
        ("Steam_Language", "english"),
        ("mobileClient", "android"),
        ("mobileClientVersion", "0 (2.1.3)"),
    ];

    /// Initialize cookie jar, and populates it with mobile cookies, which the cookies Steam sets don't replace.
    fn init_cookie_jar() -> CookieJar {
        let mut mobile_cookies = CookieJar::default();
        for (name, value) in Self::STANDARD_MOBILE_COOKIES {
            mobile_cookies.add_default(STEAM_COMMUNITY_HOST, name, value);
        }
        mobile_cookies
    }

    /// A client with a new jar (the mobile cookies only), and an HTTP client built by `configure` from the mobile app's
    /// with that jar as its cookie store.
    fn with_http_client(configure: impl FnOnce(ClientBuilder) -> ClientBuilder) -> Self {
        let cookie_store = Arc::new(RwLock::new(Self::init_cookie_jar()));
        let builder = Self::mobile_client_builder().cookie_provider(Arc::new(ClientJar(Arc::clone(&cookie_store))));
        Self {
            inner_http_client: configure(builder).build().unwrap(),
            cookie_store,
        }
    }

    /// The HTTP client of the mobile app, with its default headers, without a cookie store.
    fn mobile_client_builder() -> ClientBuilder {
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
            .redirect(Policy::limited(5))
            .default_headers(default_headers)
            .referer(false)
    }
}

/// Stores the cookies of a new session on every Steam host: `steamLoginSecure`, and a new `sessionid`.
#[cfg(feature = "cm")]
fn store_session_cookies(jar: &mut CookieJar, steam_id: SteamID, access_token: &str) {
    for cookie in login_secure_cookies(steam_id, access_token).chain(session_id_cookies()) {
        jar.store_cookie(&cookie);
    }
}

impl Default for MobileClient {
    fn default() -> Self {
        Self::with_http_client(|builder| builder)
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
    #[cfg(feature = "cm")] client_login: SteamAuthenticator<Unauthenticated, PresentMaFile>,
    #[cfg(feature = "cm")] handle: SteamHandle<User>,
    mafile: &MobileAuthFile,
) {
    fn is_send<T: Send>(_: T) {}

    is_send(unauthenticated.login());
    #[cfg(feature = "cm")]
    is_send(client_login.login_with_client(handle));
    is_send(authenticator.access_token());
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "cm")]
    use crate::session::FakeClock;
    #[cfg(feature = "cm")]
    use crate::session::FakeIssuer;
    #[cfg(feature = "cm")]
    use crate::session::IssuedToken;
    #[cfg(feature = "cm")]
    use crate::session::DEFAULT_RENEW_BEFORE as RENEW_BEFORE;
    #[cfg(feature = "cm")]
    use crate::session::MIN_RENEWAL_INTERVAL;
    #[cfg(feature = "cm")]
    use crate::session::RETRY_AFTER_FAILURE;
    #[cfg(feature = "cm")]
    use crate::session::SESSION_HOSTS;
    #[cfg(feature = "cm")]
    use crate::utils::dump_cookies_by_domain;
    #[cfg(feature = "cm")]
    use steam_client::errors::ClientError;

    #[cfg(feature = "cm")]
    const STEAM_ID: u64 = 76561197960287930;

    #[cfg(feature = "cm")]
    fn cookie(client: &MobileClient, host: &str, name: &str) -> Option<String> {
        client.cookie_store.read().get(host, name).map(ToString::to_string)
    }

    /// The cache of a login through a Steam client, with `issuer` standing in for it: `access_token` arrived at
    /// `asked_at`, and lasts `lasts`.
    #[cfg(feature = "cm")]
    fn client_session_cache(
        access_token: &str,
        lasts: Duration,
        issuer: Arc<FakeIssuer>,
        asked_at: Instant,
    ) -> SteamCache {
        let login = ClientLogin::new(Box::new(issuer), lasts, asked_at).unwrap();
        SteamCache::with_client(SteamID::from_steam64(STEAM_ID), access_token.to_string(), login)
    }

    /// An authenticator logged in through a Steam client, without the requests that check it: its token arrived at
    /// `asked_at`, and lasts `lasts`, and `issuer` stands in for the client.
    #[cfg(feature = "cm")]
    fn with_client_login(
        access_token: &str,
        lasts: Duration,
        issuer: Arc<FakeIssuer>,
        asked_at: Instant,
    ) -> SteamAuthenticator<Authenticated, PresentMaFile> {
        let client = MobileClient::default();
        client.set_session_cookies(SteamID::from_steam64(STEAM_ID), access_token);
        let mafile = MobileAuthFile::new(String::new(), String::new(), "android:1".to_string());
        SteamAuthenticator {
            inner: InnerAuthenticator {
                client,
                user: SteamUser::new("username".to_string(), "password".to_string()).with_mafile(mafile),
                cache: Some(client_session_cache(access_token, lasts, issuer, asked_at)),
            },
            auth_level: PhantomData,
        }
    }

    /// An authenticator logged in through a Steam client whose token lasts a day, and that isn't asked for another.
    #[cfg(feature = "cm")]
    fn with_session(access_token: &str) -> SteamAuthenticator<Authenticated, PresentMaFile> {
        with_client_login(access_token, DAY, FakeIssuer::answering([]), Instant::now())
    }

    #[cfg(feature = "cm")]
    const DAY: Duration = Duration::from_secs(86400);

    #[cfg(feature = "cm")]
    fn secs(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[cfg(feature = "cm")]
    fn issued(secret: &str, lasts: Duration) -> Result<IssuedToken, ClientError> {
        Ok(IssuedToken {
            secret: secret.to_string(),
            lasts,
        })
    }

    /// The session's token, as the Web API calls and the linker get it, and its `steamLoginSecure` cookies.
    #[cfg(feature = "cm")]
    fn assert_session_token(authenticator: &SteamAuthenticator<Authenticated, PresentMaFile>, access_token: &str) {
        let cache = authenticator.cache();
        assert_eq!(cache.access_token(), access_token);
        assert_eq!(cache.oauth_token(), access_token);
        for host in SESSION_HOSTS {
            assert_eq!(
                cookie(authenticator.client(), host, "steamLoginSecure").unwrap(),
                format!("{STEAM_ID}%7C%7C{access_token}")
            );
        }
    }

    /// An authenticator logged in with a password, without the requests.
    fn with_password_login(access_token: &str) -> SteamAuthenticator<Authenticated, PresentMaFile> {
        let mafile = MobileAuthFile::new(String::new(), String::new(), "android:1".to_string());
        let cache =
            SteamCache::with_login_data("76561197960287930", access_token.to_string(), "refresh".to_string()).unwrap();
        SteamAuthenticator {
            inner: InnerAuthenticator {
                client: MobileClient::default(),
                user: SteamUser::new("username".to_string(), "password".to_string()).with_mafile(mafile),
                cache: Some(cache),
            },
            auth_level: PhantomData,
        }
    }

    #[tokio::test]
    async fn password_logins_never_renew_their_token() {
        let authenticator = with_password_login("first");
        assert!(matches!(authenticator.cache().mode, LoginMode::Password { .. }));

        // no request, even long after: a password login has no client to ask, so this returns before any
        let in_a_year = Instant::now() + Duration::from_secs(365 * 86400);
        authenticator.renew_session_at(|| in_a_year).await.unwrap();
        assert_eq!(authenticator.access_token().await.unwrap().expose_secret(), "first");
        assert_eq!(
            authenticator.cache().query_tokens(),
            vec![("access_token", "first".to_string())]
        );
        assert_eq!(authenticator.cache().oauth_token(), "refresh");
    }

    #[cfg(feature = "cm")]
    #[test]
    fn session_cookies_are_sent_to_every_steam_host() {
        let authenticator = with_session("first");
        let jar = authenticator.client().cookie_store.read();

        for host in SESSION_HOSTS {
            let header = dump_cookies_by_domain(&jar, host).unwrap();
            assert!(
                header.contains(&format!("steamLoginSecure={STEAM_ID}%7C%7Cfirst")),
                "{}: {}",
                host,
                header
            );
            assert!(header.contains("sessionid="), "{}: {}", host, header);
        }
    }

    #[cfg(feature = "cm")]
    #[test]
    fn the_sessionid_steam_trading_reads_is_set() {
        let authenticator = with_session("first");
        let session_id = authenticator.dump_cookie(STEAM_COMMUNITY_HOST, "sessionid").unwrap();
        assert_eq!(session_id.len(), 24);
    }

    #[cfg(feature = "cm")]
    #[test]
    fn renewing_replaces_the_token_and_keeps_the_sessionid() {
        let authenticator = with_session("first");
        let client = authenticator.client();
        let session_id = cookie(client, STEAM_COMMUNITY_HOST, "sessionid").unwrap();

        authenticator.swap_access_token("second".to_string());

        for host in SESSION_HOSTS {
            assert_eq!(
                cookie(client, host, "steamLoginSecure").unwrap(),
                format!("{STEAM_ID}%7C%7Csecond")
            );
            assert_eq!(cookie(client, host, "sessionid").unwrap(), session_id);
        }
        let login_secure_cookies = client
            .cookie_store
            .read()
            .iter()
            .filter(|(_, name, _)| *name == "steamLoginSecure")
            .count();
        assert_eq!(login_secure_cookies, SESSION_HOSTS.len());

        let cache = authenticator.cache();
        assert_eq!(cache.query_tokens(), vec![("access_token", "second".to_string())]);
        // the linker sends the session's access token too
        assert_eq!(cache.oauth_token(), "second");
    }

    #[cfg(feature = "cm")]
    #[test]
    fn password_logins_keep_sending_the_refresh_token_to_the_linker() {
        let cache = SteamCache::with_login_data(&STEAM_ID.to_string(), "access".into(), "refresh".into()).unwrap();
        cache.set_access_token("renewed".to_string());
        assert_eq!(cache.oauth_token(), "refresh");
        assert_eq!(cache.query_tokens(), vec![("access_token", "renewed".to_string())]);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn access_token_follows_renewals() {
        let authenticator = with_session("first");
        let before = authenticator.access_token().await.unwrap();

        authenticator.swap_access_token("second".to_string());

        let after = authenticator.access_token().await.unwrap();
        assert_eq!(after.expose_secret(), "second");
        // a value got before the renewal is a snapshot
        assert_eq!(before.expose_secret(), "first");
        assert!(!format!("{after:?}").contains("second"));
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_token_not_due_is_not_renewed() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        let just_before_due = start + DAY - RENEW_BEFORE - secs(1);
        authenticator.renew_session_at(|| just_before_due).await.unwrap();

        assert_eq!(issuer.asked(), 0);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_due_token_is_renewed_once_and_swapped() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", DAY), issued("third", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);
        let session_id = cookie(authenticator.client(), STEAM_COMMUNITY_HOST, "sessionid").unwrap();

        let due = start + DAY - RENEW_BEFORE;
        authenticator.renew_session_at(|| due).await.unwrap();

        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
        assert_eq!(authenticator.access_token().await.unwrap().expose_secret(), "second");
        for host in SESSION_HOSTS {
            assert_eq!(cookie(authenticator.client(), host, "sessionid").unwrap(), session_id);
        }

        // the new token's day counts from when it was asked for
        let next_due = due + DAY - RENEW_BEFORE;
        authenticator.renew_session_at(|| next_due - secs(1)).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        authenticator.renew_session_at(|| next_due).await.unwrap();
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "third");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_failed_renewal_lets_the_request_go_with_the_valid_token() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([Err(ClientError::Dropped)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        let a_minute_left = start + DAY - secs(60);
        authenticator.renew_session_at(|| a_minute_left).await.unwrap();

        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_failed_renewal_of_an_expired_token_fails_the_request() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([Err(ClientError::Closed)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        // such as after the client was disconnected for a day
        let expired = start + DAY;
        let error = authenticator.renew_session_at(|| expired).await.unwrap_err();

        // the source downcasts as at the login (`session::tests`)
        assert!(matches!(client_error(&error), ClientError::Closed));
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn after_a_failure_the_client_is_asked_again_only_after_the_backoff() {
        let start = Instant::now();
        let lasts = RENEW_BEFORE + secs(60);
        let issuer = FakeIssuer::answering([Err(ClientError::Dropped), Err(ClientError::Dropped)]);
        let authenticator = with_client_login("first", lasts, issuer.clone(), start);

        let failed_at = start + lasts - secs(10);
        authenticator.renew_session_at(|| failed_at).await.unwrap();
        assert_eq!(issuer.asked(), 1);

        // within the backoff: the valid token goes without asking, then the expired one fails at once
        authenticator.renew_session_at(|| failed_at + secs(9)).await.unwrap();
        let error = authenticator
            .renew_session_at(|| failed_at + secs(10))
            .await
            .unwrap_err();
        assert!(matches!(error, InternalError::ClientToken(_)), "{:?}", error);
        authenticator
            .renew_session_at(|| failed_at + RETRY_AFTER_FAILURE - secs(1))
            .await
            .unwrap_err();
        assert_eq!(issuer.asked(), 1);

        authenticator
            .renew_session_at(|| failed_at + RETRY_AFTER_FAILURE)
            .await
            .unwrap_err();
        assert_eq!(issuer.asked(), 2);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn concurrent_requests_renew_once() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", DAY), issued("third", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        // the first waits for the client, which answers later, while the second comes and goes with the valid token
        let due = start + DAY - RENEW_BEFORE;
        let (first, second) = futures::join!(
            authenticator.renew_session_at(|| due),
            authenticator.renew_session_at(|| due)
        );
        first.unwrap();
        second.unwrap();

        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_token_issued_for_less_than_the_margin_does_not_renew_on_every_request() {
        let start = Instant::now();
        let short = RENEW_BEFORE - secs(60);
        let issuer = FakeIssuer::answering([issued("second", short), issued("third", short)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        let due = start + DAY - RENEW_BEFORE;
        authenticator.renew_session_at(|| due).await.unwrap();
        assert_eq!(issuer.asked(), 1);

        for later in [secs(1), MIN_RENEWAL_INTERVAL - secs(1)] {
            authenticator.renew_session_at(|| due + later).await.unwrap();
        }
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");

        authenticator
            .renew_session_at(|| due + MIN_RENEWAL_INTERVAL)
            .await
            .unwrap();
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "third");
    }

    /// The client's error behind an [`InternalError::ClientToken`], as its docs say to get it.
    #[cfg(feature = "cm")]
    fn client_error(error: &InternalError) -> &ClientError {
        assert!(matches!(error, InternalError::ClientToken(_)), "{:?}", error);
        let source = std::error::Error::source(error).unwrap();
        source.downcast_ref::<Arc<ClientError>>().unwrap()
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn the_backoff_counts_from_when_the_renewal_failed() {
        let start = Instant::now();
        let due = start + DAY - RENEW_BEFORE;
        let clock = FakeClock::at(due);
        // the client doesn't answer: the call fails at its deadline
        let issuer =
            FakeIssuer::answering_after([Err(ClientError::Dropped), Err(ClientError::Dropped)], &clock, secs(30));
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        let failed_at = due + secs(30);
        assert_eq!(clock.now(), failed_at);

        clock.set(failed_at + RETRY_AFTER_FAILURE - secs(1));
        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        clock.set(failed_at + RETRY_AFTER_FAILURE);
        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn requests_with_an_expired_token_share_a_renewal_that_timed_out() {
        let start = Instant::now();
        let clock = FakeClock::at(start + DAY);
        let issuer = FakeIssuer::answering_after([Err(ClientError::Dropped)], &clock, secs(30));
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        // the first asks the client, the others wait for its answer, which comes 30 seconds later
        let (first, second, third) = futures::join!(
            authenticator.renew_session_at(|| clock.now()),
            authenticator.renew_session_at(|| clock.now()),
            authenticator.renew_session_at(|| clock.now())
        );
        for result in [first, second, third] {
            assert!(matches!(client_error(&result.unwrap_err()), ClientError::Dropped));
        }
        assert_eq!(issuer.asked(), 1);

        // and the next ones fail at once, unsent, till the backoff ends
        clock.advance(RETRY_AFTER_FAILURE - secs(1));
        authenticator.renew_session_at(|| clock.now()).await.unwrap_err();
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn requests_with_an_expired_token_wait_for_the_renewal_in_flight() {
        let start = Instant::now();
        let clock = FakeClock::at(start + DAY);
        let issuer = FakeIssuer::answering_after([issued("second", DAY), issued("third", DAY)], &clock, secs(10));
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        let (first, second, third) = futures::join!(
            authenticator.renew_session_at(|| clock.now()),
            authenticator.renew_session_at(|| clock.now()),
            authenticator.renew_session_at(|| clock.now())
        );
        first.unwrap();
        second.unwrap();
        third.unwrap();

        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_valid_token_does_not_wait_for_the_renewal_in_flight() {
        use futures::FutureExt;

        let start = Instant::now();
        let due = start + DAY - RENEW_BEFORE;
        let issuer = FakeIssuer::answering([issued("second", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        let mut renewing = std::pin::pin!(authenticator.renew_session_at(|| due));
        assert!(futures::poll!(&mut renewing).is_pending());
        assert_eq!(issuer.asked(), 1);

        // due too, but valid: goes at once with the current token, and doesn't ask again
        let meanwhile = authenticator.renew_session_at(|| due + secs(1)).now_or_never();
        assert!(matches!(meanwhile, Some(Ok(()))), "{:?}", meanwhile);
        assert_session_token(&authenticator, "first");

        // an expired one waits
        let expired = authenticator.renew_session_at(|| start + DAY).now_or_never();
        assert!(expired.is_none(), "{:?}", expired);

        renewing.await.unwrap();
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn an_expired_token_is_renewed_within_the_least_interval() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", DAY)]);
        let authenticator = with_client_login("first", secs(10), issuer.clone(), start);

        authenticator.renew_session_at(|| start + secs(9)).await.unwrap();
        assert_eq!(issuer.asked(), 0);
        // expired, though less than the least interval after it was asked for: renewed, not sent
        authenticator.renew_session_at(|| start + secs(15)).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn an_expired_token_whose_renewal_fails_within_the_least_interval_is_not_sent() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([Err(ClientError::Closed)]);
        let authenticator = with_client_login("first", secs(10), issuer.clone(), start);

        let error = authenticator.renew_session_at(|| start + secs(15)).await.unwrap_err();
        assert!(matches!(client_error(&error), ClientError::Closed));
        assert_eq!(issuer.asked(), 1);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_renewed_tokens_lifetime_counts_from_the_ask_not_the_answer() {
        let start = Instant::now();
        let due = start + DAY - RENEW_BEFORE;
        let clock = FakeClock::at(due);
        let issuer = FakeIssuer::answering_after([issued("second", DAY), issued("third", DAY)], &clock, secs(10));
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        assert_eq!(clock.now(), due + secs(10));

        // due a day less the margin after the ask, 10 seconds before that after the answer
        clock.set(due + DAY - RENEW_BEFORE - secs(1));
        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 1);
        clock.set(due + DAY - RENEW_BEFORE);
        authenticator.renew_session_at(|| clock.now()).await.unwrap();
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "third");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_renewal_after_a_failed_one_clears_the_failure() {
        let start = Instant::now();
        let due = start + DAY - RENEW_BEFORE;
        let issuer = FakeIssuer::answering([Err(ClientError::Dropped), issued("second", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        authenticator.renew_session_at(|| due).await.unwrap();
        let renewed_at = due + RETRY_AFTER_FAILURE;
        authenticator.renew_session_at(|| renewed_at).await.unwrap();
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "second");

        // the new token isn't due: no request asks again
        authenticator.renew_session_at(|| renewed_at + secs(1)).await.unwrap();
        authenticator
            .renew_session_at(|| renewed_at + DAY - RENEW_BEFORE - secs(1))
            .await
            .unwrap();
        assert_eq!(issuer.asked(), 2);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_renewal_whose_lifetime_is_past_the_clock_fails() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", Duration::MAX), issued("third", Duration::MAX)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start);

        // valid: the request goes with the current token
        authenticator.renew_session_at(|| start + DAY - secs(60)).await.unwrap();
        assert_session_token(&authenticator, "first");
        // expired: it fails, unsent
        let error = authenticator
            .renew_session_at(|| start + DAY + secs(60))
            .await
            .unwrap_err();
        assert!(matches!(client_error(&error), ClientError::InvalidToken(_)));
        assert_eq!(issuer.asked(), 2);
        assert_session_token(&authenticator, "first");
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn access_token_renews_a_due_token_first() {
        // expired as soon as it arrived, by the real clock that `access_token` reads
        let issuer = FakeIssuer::answering([issued("second", DAY)]);
        let authenticator = with_client_login("first", Duration::ZERO, issuer.clone(), Instant::now());

        assert_eq!(authenticator.access_token().await.unwrap().expose_secret(), "second");
        assert_eq!(issuer.asked(), 1);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn fetch_confirmations_renews_a_due_token_first() {
        // the renewal fails and the token has expired, so nothing is sent
        let issuer = FakeIssuer::answering([]);
        let authenticator = with_client_login("first", Duration::ZERO, issuer.clone(), Instant::now());

        let error = authenticator.fetch_confirmations().await.unwrap_err();
        match error {
            AuthError::InternalError(error) => assert!(matches!(client_error(&error), ClientError::Closed)),
            other => panic!("{:?}", other),
        }
        assert_eq!(issuer.asked(), 1);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn renew_before_sets_the_margin() {
        let start = Instant::now();
        let issuer = FakeIssuer::answering([issued("second", DAY)]);
        let authenticator = with_client_login("first", DAY, issuer.clone(), start).renew_before(DAY);

        // a margin of the token's lifetime: due once the least interval passed
        authenticator
            .renew_session_at(|| start + MIN_RENEWAL_INTERVAL - secs(1))
            .await
            .unwrap();
        assert_eq!(issuer.asked(), 0);
        authenticator
            .renew_session_at(|| start + MIN_RENEWAL_INTERVAL)
            .await
            .unwrap();
        assert_eq!(issuer.asked(), 1);
        assert_session_token(&authenticator, "second");
    }

    #[cfg(feature = "cm")]
    #[test]
    fn the_default_margin_is_an_hour() {
        assert_eq!(RENEW_BEFORE, Duration::from_secs(3600));
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn renew_before_changes_nothing_on_a_password_login() {
        let authenticator = with_password_login("first").renew_before(Duration::MAX);
        assert!(matches!(authenticator.cache().mode, LoginMode::Password { .. }));
        assert_eq!(authenticator.access_token().await.unwrap().expose_secret(), "first");
    }

    #[cfg(feature = "cm")]
    #[test]
    fn a_missing_cookie_is_none() {
        let authenticator = with_session("first");
        assert_eq!(authenticator.dump_cookie(STEAM_COMMUNITY_HOST, "steamparental"), None);
        // the jar has other cookies on that host
        assert!(authenticator
            .dump_cookie(STEAM_COMMUNITY_HOST, "mobileClient")
            .is_some());
    }

    const MOBILE_COOKIES: &str = "Steam_Language=english; mobileClient=android; mobileClientVersion=0 (2.1.3)";
    const STORE: &str = "store.steampowered.com";
    const HELP: &str = "help.steampowered.com";

    #[cfg(feature = "cm")]
    #[test]
    fn every_session_cookie_survives_in_a_thousand_fresh_jars() {
        // the old jar lost one in about 9% of jars, so 1,000 of them all passing by chance is about 0.909^1000
        for _ in 0..1000 {
            let mut jar = MobileClient::init_cookie_jar();
            store_session_cookies(&mut jar, SteamID::from_steam64(STEAM_ID), "first");

            let session_id = jar.get(STEAM_COMMUNITY_HOST, "sessionid").unwrap();
            let session_cookies = format!("steamLoginSecure={STEAM_ID}%7C%7Cfirst; sessionid={session_id}");
            assert_eq!(
                dump_cookies_by_domain(&jar, STEAM_COMMUNITY_HOST).unwrap(),
                format!("{MOBILE_COOKIES}; {session_cookies}")
            );
            for host in &SESSION_HOSTS[1..] {
                assert_eq!(dump_cookies_by_domain(&jar, host).unwrap(), session_cookies);
            }
            assert_eq!(jar.iter().count(), 3 + 2 * SESSION_HOSTS.len());
        }
    }

    /// What a request to [`fake_steam`] carried.
    #[derive(Debug)]
    struct Seen {
        host: String,
        path: String,
        cookie: Option<String>,
    }

    /// A loopback server standing in for Steam's hosts. A request for `/set/<name>=<value>/…` gets those cookies in
    /// Set-Cookie headers. One for `/redirect/<host>/<name>=<value>/…` gets them too, in a 302 to `/landed` on `<host>`
    /// (same port), as Steam's `market/eligibilitycheck` redirect sets `sessionid` and `webTradeEligibility` (seen live
    /// on 2026-10-22). What each request carried is sent back to the test.
    fn fake_steam() -> (u16, std::sync::mpsc::Receiver<Seen>) {
        use std::io::BufRead;
        use std::io::BufReader;
        use std::io::Write;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split(' ').nth(1).unwrap().to_string();
                let (mut host, mut cookie) = (String::new(), None);
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    let Some((name, value)) = line.trim_end().split_once(':') else {
                        break;
                    };
                    match name.to_ascii_lowercase().as_str() {
                        "host" => host = value.trim().split(':').next().unwrap().to_string(),
                        "cookie" => cookie = Some(value.trim().to_string()),
                        _ => {}
                    }
                }
                // a Domain that an RFC 6265 jar would share with every steampowered.com host
                let domain = if host.ends_with("steampowered.com") {
                    "; Domain=steampowered.com"
                } else {
                    ""
                };
                let (status, location, pairs) = if let Some(pairs) = path.strip_prefix("/set/") {
                    ("200 OK", String::new(), pairs)
                } else if let Some(rest) = path.strip_prefix("/redirect/") {
                    let (to, pairs) = rest.split_once('/').unwrap();
                    ("302 Found", format!("Location: http://{to}:{port}/landed\r\n"), pairs)
                } else {
                    ("200 OK", String::new(), "")
                };
                let set_cookies: String = pairs
                    .split('/')
                    .filter(|pair| !pair.is_empty())
                    .map(|pair| format!("Set-Cookie: {pair}{domain}; Path=/\r\n"))
                    .collect();
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\n{location}{set_cookies}Content-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                sender.send(Seen { host, path, cookie }).unwrap();
            }
        });
        (port, receiver)
    }

    /// The real client, with the Steam hosts it requests resolved to [`fake_steam`] on `port`.
    fn fake_steam_client(port: u16) -> MobileClient {
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        MobileClient::with_http_client(|builder| {
            builder
                .no_proxy()
                .resolve(STEAM_COMMUNITY_HOST, address)
                .resolve(STORE, address)
                .resolve(HELP, address)
        })
    }

    /// GETs `path` on `host` through `request`, and returns what the server saw.
    async fn get(
        client: &MobileClient,
        server: &std::sync::mpsc::Receiver<Seen>,
        port: u16,
        host: &str,
        path: &str,
    ) -> Seen {
        client
            .request(
                format!("http://{host}:{port}{path}"),
                Method::GET,
                None,
                None::<u8>,
                None::<u8>,
            )
            .await
            .unwrap();
        let seen = server.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(seen.host, host);
        seen
    }

    /// GETs `path` on `host`, which redirects, and returns what the server saw of the request and of the redirected
    /// one.
    async fn redirected(
        client: &MobileClient,
        server: &std::sync::mpsc::Receiver<Seen>,
        port: u16,
        host: &str,
        path: &str,
    ) -> (Seen, Seen) {
        let response = client
            .request(
                format!("http://{host}:{port}{path}"),
                Method::GET,
                None,
                None::<u8>,
                None::<u8>,
            )
            .await
            .unwrap();
        assert_eq!(response.url().path(), "/landed");
        let first = server.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = server.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((first.host.as_str(), first.path.as_str()), (host, path));
        assert_eq!(second.path, "/landed");
        (first, second)
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_client_logins_cookies_go_through_a_redirect() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        // as `login_with_client`: the session cookies are set here, Steam never sent them
        client.set_session_cookies(SteamID::from_steam64(STEAM_ID), "first");
        let session_id = cookie(&client, STEAM_COMMUNITY_HOST, "sessionid").unwrap();
        let session = format!("{MOBILE_COOKIES}; steamLoginSecure={STEAM_ID}%7C%7Cfirst; sessionid={session_id}");

        let (first, second) = redirected(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/redirect/steamcommunity.com/webTradeEligibility=e1",
        )
        .await;
        assert_eq!(first.cookie.unwrap(), session);
        // live, after this login, it carried only `browserid`
        assert_eq!(second.cookie.unwrap(), format!("{session}; webTradeEligibility=e1"));
    }

    #[tokio::test]
    async fn a_password_logins_cookies_go_through_a_redirect() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        // as the password login: Steam sets the session cookies (settoken, then a page)
        get(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/set/steamLoginSecure=token/sessionid=s1",
        )
        .await;
        let session = format!("{MOBILE_COOKIES}; steamLoginSecure=token; sessionid=s1");

        let (first, second) = redirected(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/redirect/steamcommunity.com/webTradeEligibility=e1",
        )
        .await;
        assert_eq!(first.cookie.unwrap(), session);
        // the mobile cookies too, which reqwest's own jar didn't have
        assert_eq!(second.cookie.unwrap(), format!("{session}; webTradeEligibility=e1"));
    }

    #[tokio::test]
    async fn a_redirects_set_cookie_is_kept_as_any_other() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        get(&client, &server, port, STEAM_COMMUNITY_HOST, "/set/sessionid=s1").await;

        // a new value replaces the old one in its place, a new cookie comes last
        let (_, second) = redirected(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/redirect/steamcommunity.com/sessionid=s2/webTradeEligibility=e1",
        )
        .await;
        let after = format!("{MOBILE_COOKIES}; sessionid=s2; webTradeEligibility=e1");
        assert_eq!(second.cookie.unwrap(), after);
        assert_eq!(
            client.cookie_store.read().get(STEAM_COMMUNITY_HOST, "sessionid"),
            Some("s2")
        );
        assert_eq!(
            get(&client, &server, port, STEAM_COMMUNITY_HOST, "/")
                .await
                .cookie
                .unwrap(),
            after
        );
    }

    #[tokio::test]
    async fn a_redirects_cookie_is_sent_until_its_max_age_runs_out() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        get(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/set/steamLoginSecure=token/sessionid=s1",
        )
        .await;
        let session = format!("{MOBILE_COOKIES}; steamLoginSecure=token; sessionid=s1");

        // as `market/eligibilitycheck` set it live on a redirect that carried only `browserid` (2026-10-22)
        let (_, second) = redirected(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            "/redirect/steamcommunity.com/webTradeEligibility=e1;Max-Age=30",
        )
        .await;
        let eligible = format!("{session}; webTradeEligibility=e1");
        assert_eq!(second.cookie.unwrap(), eligible);
        assert_eq!(
            get(&client, &server, port, STEAM_COMMUNITY_HOST, "/")
                .await
                .cookie
                .unwrap(),
            eligible
        );

        client.cookie_store.write().advance_clock(Duration::from_secs(30));
        // as before the redirect
        assert_eq!(
            get(&client, &server, port, STEAM_COMMUNITY_HOST, "/")
                .await
                .cookie
                .unwrap(),
            session
        );
    }

    #[tokio::test]
    async fn a_redirect_to_another_host_sends_that_hosts_cookies() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        get(&client, &server, port, STEAM_COMMUNITY_HOST, "/set/sessionid=c1").await;
        get(&client, &server, port, STORE, "/set/sessionid=s1").await;

        let (first, second) = redirected(
            &client,
            &server,
            port,
            STEAM_COMMUNITY_HOST,
            &format!("/redirect/{STORE}/browserid=b1"),
        )
        .await;
        assert_eq!(first.cookie.unwrap(), format!("{MOBILE_COOKIES}; sessionid=c1"));
        assert_eq!(second.host, STORE);
        assert_eq!(second.cookie.unwrap(), "sessionid=s1");
        // stored under the host that answered
        assert_eq!(
            client.cookie_store.read().get(STEAM_COMMUNITY_HOST, "browserid"),
            Some("b1")
        );
        assert_eq!(client.cookie_store.read().get(STORE, "browserid"), None);
    }

    #[tokio::test]
    async fn a_host_without_cookies_gets_an_empty_cookie_header() {
        let (port, server) = fake_steam();
        let client = fake_steam_client(port);
        // so the HTTP client's cookie store isn't asked
        assert_eq!(get(&client, &server, port, HELP, "/").await.cookie.unwrap(), "");
        assert_eq!(
            get(&client, &server, port, STEAM_COMMUNITY_HOST, "/")
                .await
                .cookie
                .unwrap(),
            MOBILE_COOKIES
        );
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn the_cookie_header_each_host_gets() {
        let (port, server) = fake_steam();
        let mafile = MobileAuthFile::new(String::new(), String::new(), "android:1".to_string());
        let authenticator = SteamAuthenticator::<Authenticated, PresentMaFile> {
            inner: InnerAuthenticator {
                client: fake_steam_client(port),
                user: SteamUser::new("username".to_string(), "password".to_string()).with_mafile(mafile),
                cache: Some(client_session_cache(
                    "first",
                    DAY,
                    FakeIssuer::answering([]),
                    Instant::now(),
                )),
            },
            auth_level: PhantomData,
        };
        let client = authenticator.client();
        let sent = |host: &'static str, path: &'static str| {
            let server = &server;
            async move {
                get(client, server, port, host, path)
                    .await
                    .cookie
                    .expect("every request has a Cookie header")
            }
        };
        // a host with no cookies still gets the header, empty
        assert_eq!(sent(HELP, "/").await, "");
        assert_eq!(sent(STEAM_COMMUNITY_HOST, "/").await, MOBILE_COOKIES);

        // Set-Cookie: stored under the host that answered; the mobile cookies win
        sent(STEAM_COMMUNITY_HOST, "/set/sessionid=c1/mobileClient=web/browserid=b1").await;
        sent(STORE, "/set/sessionid=s1/steamCountry=BR").await;
        assert_eq!(
            sent(STEAM_COMMUNITY_HOST, "/").await,
            format!("{MOBILE_COOKIES}; sessionid=c1; browserid=b1")
        );
        assert_eq!(sent(STORE, "/").await, "sessionid=s1; steamCountry=BR");
        assert_eq!(sent(HELP, "/").await, "");

        // a session: steamLoginSecure and a new sessionid on every host
        client.set_session_cookies(SteamID::from_steam64(STEAM_ID), "first");
        let session_id = cookie(client, STEAM_COMMUNITY_HOST, "sessionid").unwrap();
        let login_secure = |token: &str| format!("steamLoginSecure={STEAM_ID}%7C%7C{token}");
        assert_eq!(
            sent(STEAM_COMMUNITY_HOST, "/").await,
            format!(
                "{MOBILE_COOKIES}; sessionid={session_id}; browserid=b1; {}",
                login_secure("first")
            )
        );
        assert_eq!(
            sent(STORE, "/").await,
            format!("sessionid={session_id}; steamCountry=BR; {}", login_secure("first"))
        );
        assert_eq!(
            sent(HELP, "/").await,
            format!("{}; sessionid={session_id}", login_secure("first"))
        );

        // renewing replaces steamLoginSecure in place
        authenticator.swap_access_token("second".to_string());
        assert_eq!(
            sent(HELP, "/").await,
            format!("{}; sessionid={session_id}", login_secure("second"))
        );

        // a later Set-Cookie replaces the value on that host only
        sent(STEAM_COMMUNITY_HOST, "/set/sessionid=c2").await;
        // parental unlock: the community gets steamparental, the store doesn't (its host has a leading dot)
        crate::web_handler::store_parental_cookie(&mut client.cookie_store.write(), "pin");
        assert_eq!(
            sent(STEAM_COMMUNITY_HOST, "/").await,
            format!(
                "{MOBILE_COOKIES}; sessionid=c2; browserid=b1; {}; steamparental=pin",
                login_secure("second")
            )
        );
        assert_eq!(
            sent(STORE, "/").await,
            format!("sessionid={session_id}; steamCountry=BR; {}", login_secure("second"))
        );
    }

    #[tokio::test]
    async fn http_errors_never_show_the_query() {
        // nothing listens there, so the request fails without leaving the machine
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let error = Client::new()
            .get(format!(
                "http://{address}/mobileconf/getlist?p=android%3A1&a=1&k=SECRET-HASH&input_protobuf_encoded=SECRET-PROTO"
            ))
            .send()
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("SECRET"),
            "the test needs an error that shows its URL"
        );

        let error = InternalError::from(error);
        assert!(matches!(error, InternalError::HttpError(_)), "{:?}", error);
        for text in [
            error.to_string(),
            format!("{error:?}"),
            AuthError::from(error).to_string(),
        ] {
            assert!(!text.contains("SECRET"), "{}", text);
            assert!(text.contains("/mobileconf/getlist"), "{}", text);
        }
    }

    fn url() -> Url {
        Url::parse("https://steamcommunity.com/mobileconf/getlist?p=android%3A1&k=hash").unwrap()
    }

    #[test]
    fn http_429_is_too_many_requests() {
        let error = status_error(StatusCode::TOO_MANY_REQUESTS, &url(), "<html>Access Denied</html>");
        match error {
            InternalError::TooManyRequests { url } => assert_eq!(url, "https://steamcommunity.com/mobileconf/getlist"),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn other_http_errors_keep_the_status_and_the_start_of_the_page() {
        let page = format!("<html>\n  <head>   <title>Error</title>\n{}</html>", "x".repeat(1000));
        let error = status_error(StatusCode::FORBIDDEN, &url(), &page);
        let message = error.to_string();
        match error {
            InternalError::HttpStatus { status, url, body } => {
                assert_eq!(status, StatusCode::FORBIDDEN);
                // no query string: it carries the confirmation hash
                assert_eq!(url, "https://steamcommunity.com/mobileconf/getlist");
                assert!(body.starts_with("<html> <head> <title>Error</title> xxx"), "{}", body);
                assert_eq!(body.chars().count(), ERROR_BODY_SNIPPET_CHARS + 1);
                assert!(body.ends_with('…'));
            }
            other => panic!("{:?}", other),
        }
        assert!(message.contains("403"), "{}", message);
        assert!(!message.contains("hash"), "{}", message);
    }

    #[test]
    fn short_error_pages_are_kept_whole() {
        assert_eq!(body_snippet(""), "");
        assert_eq!(body_snippet(" Too\n many "), "Too many");
        // cut on a char boundary
        assert_eq!(
            body_snippet(&"é".repeat(300)).chars().count(),
            ERROR_BODY_SNIPPET_CHARS + 1
        );
    }

    #[test]
    fn x_eresult_other_than_ok_is_an_error() {
        let header = |value: &str| HeaderValue::from_str(value).unwrap();

        assert!(eresult_check(None, &url()).is_ok());
        assert!(eresult_check(Some(&header("1")), &url()).is_ok());
        // not a number: passes, as before the check
        assert!(eresult_check(Some(&header("OK")), &url()).is_ok());

        match eresult_check(Some(&header("5")), &url()) {
            Err(error @ InternalError::EResult { eresult: 5, .. }) => {
                let message = error.to_string();
                assert!(message.contains("InvalidPassword"), "{}", message);
                assert!(!message.contains("hash"), "{}", message);
            }
            other => panic!("{:?}", other),
        }
        // a number steam-language doesn't know
        match eresult_check(Some(&header("100000")), &url()) {
            Err(error @ InternalError::EResult { eresult: 100_000, .. }) => {
                assert!(error.to_string().contains("unknown"));
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn redirects_to_login_mean_the_session_expired() {
        let expired = |url: &str| MobileClient::url_expired_check(&Url::parse(url).unwrap());
        assert!(expired("https://store.steampowered.com/login/?redir=account"));
        assert!(expired("steammobile://lostauth"));
        assert!(!expired("https://store.steampowered.com/account/"));
    }
}

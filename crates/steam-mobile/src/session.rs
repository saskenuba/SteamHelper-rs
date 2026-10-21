//! The session's access token and, with feature `cm`, a login through a Steam client logged on with steam-client-lib,
//! for `SteamAuthenticator::login_with_client`: its cookies, and how its token is renewed.
//!
//! Steam's web pages take a session as two cookies: `steamLoginSecure`, the SteamID and an access token joined by
//! `||` and URL-encoded, and `sessionid`, a random value forms send back, on each Steam host.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
#[cfg(feature = "cm")]
use std::fmt::Write;
#[cfg(feature = "cm")]
use std::sync::Arc;
#[cfg(feature = "cm")]
use std::time::Duration;
#[cfg(feature = "cm")]
use std::time::Instant;
#[cfg(feature = "cm")]
use std::time::SystemTime;

#[cfg(feature = "cm")]
use cookie::Cookie;
#[cfg(feature = "cm")]
use futures::future::BoxFuture;
#[cfg(feature = "cm")]
use futures::lock::Mutex;
#[cfg(feature = "cm")]
use steam_client::client::User;
#[cfg(feature = "cm")]
use steam_client::errors::ClientError;
#[cfg(feature = "cm")]
use steam_client::handle::SteamHandle;
#[cfg(feature = "cm")]
use steamid_parser::SteamID;
#[cfg(feature = "cm")]
use tracing::info;
#[cfg(feature = "cm")]
use tracing::warn;

#[cfg(feature = "cm")]
use crate::errors::InternalError;
#[cfg(feature = "cm")]
use crate::web_handler::login::SESSION_ID_COOKIE;
#[cfg(feature = "cm")]
use crate::web_handler::login::STEAM_LOGIN_SECURE_COOKIE;

/// The hosts the session cookies are set on, as the cookie jar looks them up: the exact host of each request.
#[cfg(feature = "cm")]
pub(crate) const SESSION_HOSTS: [&str; 4] = [
    "steamcommunity.com",
    "store.steampowered.com",
    "help.steampowered.com",
    "checkout.steampowered.com",
];

/// How long before its access token expires a session logged in through a Steam client renews it, unless
/// `SteamAuthenticator::renew_before` sets another margin. The margin is also how long the session outlasts a client
/// that can't renew it, such as while the client can't reach Steam: requests go on with the current token meanwhile.
#[cfg(feature = "cm")]
pub(crate) const DEFAULT_RENEW_BEFORE: Duration = Duration::from_secs(60 * 60);

/// The access token of an authenticated session, as [`crate::SteamAuthenticator::access_token`] returns it: what
/// Steam's Web API takes as `access_token=`.
///
/// A snapshot: a login through a Steam client (feature `cm`) replaces the session's token about daily, so get it again
/// for each request instead of keeping it.
#[derive(Clone)]
pub struct AccessToken(String);

impl AccessToken {
    pub(crate) fn new(access_token: String) -> Self {
        Self(access_token)
    }

    /// The token itself. Keep it out of logs and error messages.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl Debug for AccessToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AccessToken").field(&"<redacted>").finish()
    }
}

/// How the session logged in, which decides what the authenticator linker sends as its `access_token`, and whether
/// the session's access token is renewed.
pub(crate) enum LoginMode {
    /// A password login. The linker sends its refresh token, as it always did, and the access token is never renewed,
    /// as it never was.
    Password { refresh_token: String },
    /// A login through a Steam client, which issued the session no refresh token: the linker sends the session's
    /// access token, and the client issues a new one before it expires.
    #[cfg(feature = "cm")]
    Client(ClientLogin),
}

/// How long after a failed renewal, counted from when it failed, the session waits before it asks the Steam client
/// again. Meanwhile, requests go on with the current token while it is valid, and fail unsent with the renewal's error
/// once it has expired, instead of each waiting up to the client's call deadline for another answer.
#[cfg(feature = "cm")]
pub(crate) const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(30);

/// The least time between asking for a token and renewing it while it is valid, for a token Steam issued for less than
/// the renewal margin: one due as soon as it arrives would otherwise make every request ask the client again. It never
/// defers the renewal of an expired token.
#[cfg(feature = "cm")]
pub(crate) const MIN_RENEWAL_INTERVAL: Duration = Duration::from_secs(30);

/// Something that issues the session access tokens: the Steam client it logged in through, or a test's stand-in.
#[cfg(feature = "cm")]
pub(crate) trait TokenIssuer: Send + Sync {
    /// Asks for a new access token for the session's account.
    fn issue(&self) -> BoxFuture<'_, Result<IssuedToken, ClientError>>;
}

#[cfg(feature = "cm")]
impl TokenIssuer for SteamHandle<User> {
    fn issue(&self) -> BoxFuture<'_, Result<IssuedToken, ClientError>> {
        Box::pin(async move { self.web_access_token().await.map(IssuedToken::from_client) })
    }
}

/// An access token a Steam client just issued, and how long it lasts from now.
#[cfg(feature = "cm")]
pub(crate) struct IssuedToken {
    pub(crate) secret: String,
    pub(crate) lasts: Duration,
}

#[cfg(feature = "cm")]
impl IssuedToken {
    fn from_client(token: steam_client::auth::AccessToken) -> Self {
        let expires_at = token.expires_at();
        // its lifetime by its own claims, both Steam's clock, so that this machine's clock being off doesn't matter
        let lasts = token
            .issued_at()
            .and_then(|issued_at| expires_at.duration_since(issued_at).ok())
            // without a usable `iat`: what is left until its `exp` by the local clock, which a slow clock overrates
            .unwrap_or_else(|| expires_at.duration_since(SystemTime::now()).unwrap_or_default());
        Self {
            secret: token.expose_secret().to_string(),
            lasts,
        }
    }
}

/// The Steam client a session logged in through, and when its token expires and is renewed.
///
/// At most one request renews the token at a time, holding `renewal` while it waits for the client: the first one
/// that finds the renewal due, which waits for it, as steam-mobile spawns no task of its own. A request coming
/// meanwhile goes at once with the current token while it is valid; one whose token has expired waits for that
/// renewal, and takes its outcome, so the client is asked once.
#[cfg(feature = "cm")]
pub(crate) struct ClientLogin {
    /// The client, kept: a handle keeps a spawned client running.
    issuer: Box<dyn TokenIssuer>,
    /// How long before the token expires it is renewed.
    renew_before: Duration,
    /// Held by the request renewing the token, across its wait for the client, and by nothing else.
    renewal: Mutex<()>,
    /// Only held to read or write it, never across an await.
    token: parking_lot::Mutex<TokenState>,
}

/// When the session's token expires, and whether its last renewal failed, by the monotonic clock.
#[cfg(feature = "cm")]
struct TokenState {
    expires: Instant,
    /// [`MIN_RENEWAL_INTERVAL`] after the token was asked for: no renewal is due before, while it is valid.
    earliest: Instant,
    renewal: Renewal,
}

#[cfg(feature = "cm")]
enum Renewal {
    /// Due the renewal margin before the token expires, or at `earliest` if that is later, or once it has expired.
    Scheduled,
    /// The last renewal failed: the client is asked again from `until` on, and till then a request whose token has
    /// expired fails with `error`.
    Failed { until: Instant, error: Arc<ClientError> },
}

/// What a request does about the session's token.
#[cfg(feature = "cm")]
enum Check {
    /// Goes with the current token, which is valid.
    Go,
    /// Fails unsent: the token has expired, and the last renewal failed within [`RETRY_AFTER_FAILURE`].
    Fail(Arc<ClientError>),
    /// Renews the token, or waits for the renewal in flight; `valid` if the current token still is.
    Renew { valid: bool },
}

#[cfg(feature = "cm")]
impl TokenState {
    /// The state of a token asked for at `asked_at` that lasts `lasts`, or an error for a lifetime past what the
    /// clock can count.
    fn issued(asked_at: Instant, lasts: Duration) -> Result<Self, ClientError> {
        let after = |by| {
            asked_at
                .checked_add(by)
                .ok_or_else(|| ClientError::InvalidToken("its expiry is out of range".to_string()))
        };
        Ok(Self {
            expires: after(lasts)?,
            earliest: after(MIN_RENEWAL_INTERVAL)?,
            renewal: Renewal::Scheduled,
        })
    }

    /// When the token is due for renewal while it is valid, `renew_before` before it expires.
    fn due(&self, renew_before: Duration) -> Instant {
        self.expires
            .checked_sub(renew_before)
            .map_or(self.earliest, |due| due.max(self.earliest))
    }

    fn check(&self, now: Instant, renew_before: Duration) -> Check {
        let valid = now < self.expires;
        match &self.renewal {
            Renewal::Failed { until, error } if now < *until => {
                if valid {
                    Check::Go
                } else {
                    Check::Fail(Arc::clone(error))
                }
            }
            Renewal::Scheduled if valid && now < self.due(renew_before) => Check::Go,
            Renewal::Scheduled | Renewal::Failed { .. } => Check::Renew { valid },
        }
    }
}

#[cfg(feature = "cm")]
impl ClientLogin {
    /// The session of a login through `issuer`, asked at `now()` for the session's first token: its lifetime counts
    /// from then, a little before it arrived. Returns the token too.
    pub(crate) async fn start(
        issuer: Box<dyn TokenIssuer>,
        now: impl Fn() -> Instant + Send,
    ) -> Result<(Self, String), InternalError> {
        let asked_at = now();
        let token = issuer
            .issue()
            .await
            .map_err(|error| client_token_error(Arc::new(error)))?;
        let login = Self::new(issuer, token.lasts, asked_at).map_err(|error| client_token_error(Arc::new(error)))?;
        Ok((login, token.secret))
    }

    /// The session of a login through `issuer`, whose token was asked for at `asked_at` and lasts `lasts`.
    pub(crate) fn new(issuer: Box<dyn TokenIssuer>, lasts: Duration, asked_at: Instant) -> Result<Self, ClientError> {
        Ok(Self {
            issuer,
            renew_before: DEFAULT_RENEW_BEFORE,
            renewal: Mutex::new(()),
            token: parking_lot::Mutex::new(TokenState::issued(asked_at, lasts)?),
        })
    }

    /// Sets how long before the token expires it is renewed, from the next request on.
    pub(crate) fn set_renew_before(&mut self, renew_before: Duration) {
        self.renew_before = renew_before;
    }

    fn check(&self, now: Instant) -> Check {
        self.token.lock().check(now, self.renew_before)
    }

    /// Renews the session's token if it is due, with `swap` taking the new one, before a request: what every request
    /// with the session does first. `now` reads the monotonic clock.
    ///
    /// The request that starts a renewal waits for the client's answer; one whose token is still valid doesn't wait
    /// for a renewal another request started. Only fails when the request can't go: its token has expired, and the
    /// renewal failed, now or within [`RETRY_AFTER_FAILURE`] before. An expired token is never sent. A request that
    /// fails is never retried here.
    pub(crate) async fn renew_if_due(
        &self,
        now: impl Fn() -> Instant + Send,
        swap: impl FnOnce(String) + Send,
    ) -> Result<(), InternalError> {
        let valid = match self.check(now()) {
            Check::Go => return Ok(()),
            Check::Fail(error) => return Err(client_token_error(error)),
            Check::Renew { valid } => valid,
        };
        let _renewing = match self.renewal.try_lock() {
            Some(renewing) => renewing,
            // another request is renewing it: this one goes meanwhile
            None if valid => return Ok(()),
            // this one can't go: it waits for that renewal, and takes its outcome
            None => self.renewal.lock().await,
        };
        // a renewal may have ended since the first look
        match self.check(now()) {
            Check::Go => return Ok(()),
            Check::Fail(error) => return Err(client_token_error(error)),
            Check::Renew { .. } => {}
        }

        let asked_at = now();
        let renewed = self.issuer.issue().await.and_then(|token| {
            let state = TokenState::issued(asked_at, token.lasts)?;
            Ok((token.secret, state))
        });
        match renewed {
            Ok((secret, state)) => {
                // the token first, so that a request that sees the new state reads the new token
                swap(secret);
                *self.token.lock() = state;
                info!("Renewed the web session.");
                Ok(())
            }
            Err(error) => {
                let failed_at = now();
                let error = Arc::new(error);
                // an `Instant` 30 seconds later can always be counted in practice; if not, no backoff
                let until = failed_at.checked_add(RETRY_AFTER_FAILURE).unwrap_or(failed_at);
                let expires = {
                    let mut state = self.token.lock();
                    state.renewal = Renewal::Failed {
                        until,
                        error: Arc::clone(&error),
                    };
                    state.expires
                };
                let left = expires.saturating_duration_since(failed_at);
                if left.is_zero() {
                    return Err(client_token_error(error));
                }
                warn!(
                    "Failed to renew the web session, whose token is valid for {}s more, so the request goes with it: \
                     {error}",
                    left.as_secs()
                );
                Ok(())
            }
        }
    }
}

/// The error of a Steam client that failed to issue the session a token: at the login, or at a renewal once the
/// current token has expired. Both box the `Arc`, which the requests failing with the same renewal's error share.
#[cfg(feature = "cm")]
pub(crate) fn client_token_error(error: Arc<ClientError>) -> InternalError {
    InternalError::ClientToken(Box::new(error))
}

/// A stand-in for the Steam client in tests: answers with the tokens it was given, in order, then with
/// [`ClientError::Closed`], and counts how often it was asked. Each answer comes after the test's executor polled the
/// future once more, and, if it was made with [`FakeIssuer::answering_after`], once `takes` passed on the test's clock.
#[cfg(all(test, feature = "cm"))]
pub(crate) struct FakeIssuer {
    answers: parking_lot::Mutex<std::collections::VecDeque<Result<IssuedToken, ClientError>>>,
    asked: std::sync::atomic::AtomicUsize,
    takes: Option<(FakeClock, Duration)>,
}

#[cfg(all(test, feature = "cm"))]
impl FakeIssuer {
    pub(crate) fn answering(answers: impl IntoIterator<Item = Result<IssuedToken, ClientError>>) -> Arc<Self> {
        Self::with(answers, None)
    }

    /// Answers once `takes` passed on `clock`, as the client does after its round trip to Steam, or after its call
    /// deadline when Steam doesn't answer.
    pub(crate) fn answering_after(
        answers: impl IntoIterator<Item = Result<IssuedToken, ClientError>>,
        clock: &FakeClock,
        takes: Duration,
    ) -> Arc<Self> {
        Self::with(answers, Some((clock.clone(), takes)))
    }

    fn with(
        answers: impl IntoIterator<Item = Result<IssuedToken, ClientError>>,
        takes: Option<(FakeClock, Duration)>,
    ) -> Arc<Self> {
        Arc::new(Self {
            answers: parking_lot::Mutex::new(answers.into_iter().collect()),
            asked: std::sync::atomic::AtomicUsize::new(0),
            takes,
        })
    }

    pub(crate) fn asked(&self) -> usize {
        self.asked.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(all(test, feature = "cm"))]
impl TokenIssuer for Arc<FakeIssuer> {
    fn issue(&self) -> BoxFuture<'_, Result<IssuedToken, ClientError>> {
        Box::pin(async move {
            self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // answers later, as the client does: a request coming meanwhile finds the renewal in flight
            tokio::task::yield_now().await;
            if let Some((clock, takes)) = &self.takes {
                clock.advance(*takes);
            }
            let answer = self.answers.lock().pop_front();
            answer.unwrap_or(Err(ClientError::Closed))
        })
    }
}

/// A monotonic clock for tests, which only moves when told.
#[cfg(all(test, feature = "cm"))]
#[derive(Clone)]
pub(crate) struct FakeClock(Arc<parking_lot::Mutex<Instant>>);

#[cfg(all(test, feature = "cm"))]
impl FakeClock {
    pub(crate) fn at(now: Instant) -> Self {
        Self(Arc::new(parking_lot::Mutex::new(now)))
    }

    pub(crate) fn now(&self) -> Instant {
        *self.0.lock()
    }

    pub(crate) fn set(&self, now: Instant) {
        *self.0.lock() = now;
    }

    pub(crate) fn advance(&self, by: Duration) {
        let mut now = self.0.lock();
        *now += by;
    }
}

/// The `steamLoginSecure` cookies of a session, one per host.
#[cfg(feature = "cm")]
pub(crate) fn login_secure_cookies(steam_id: SteamID, access_token: &str) -> impl Iterator<Item = Cookie<'static>> {
    let value = encode_uri_component(&format!("{}||{}", steam_id.to_steam64(), access_token));
    SESSION_HOSTS
        .iter()
        .copied()
        .map(move |host| session_cookie(STEAM_LOGIN_SECURE_COOKIE, value.clone(), host))
}

/// The `sessionid` cookies of a new session, one per host, all with the same random value.
#[cfg(feature = "cm")]
pub(crate) fn session_id_cookies() -> impl Iterator<Item = Cookie<'static>> {
    // as node-steam-session does: 12 random bytes, in hex
    let session_id = hex::encode(rand::random::<[u8; 12]>());
    SESSION_HOSTS
        .iter()
        .copied()
        .map(move |host| session_cookie(SESSION_ID_COOKIE, session_id.clone(), host))
}

#[cfg(feature = "cm")]
fn session_cookie(name: &'static str, value: String, host: &'static str) -> Cookie<'static> {
    Cookie::build(name, value).domain(host).path("/").secure(true).finish()
}

/// Encodes like JavaScript's `encodeURIComponent`, which is what Steam's own clients use for `steamLoginSecure`.
#[cfg(feature = "cm")]
fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                encoded.push(byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "cm")]
    const STEAM_ID: u64 = 76561197960287930;
    /// Not a real token: a JWT's shape, with the characters one can have.
    const ACCESS_TOKEN: &str = "eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiI3NjU2MTE5Nzk2MDI4NzkzMCJ9.c2lnbmF0dXJl-_";

    #[cfg(feature = "cm")]
    #[test]
    fn steam_login_secure_joins_and_encodes_the_steam_id_and_token() {
        let cookies: Vec<_> = login_secure_cookies(SteamID::from_steam64(STEAM_ID), ACCESS_TOKEN).collect();

        assert_eq!(cookies.len(), SESSION_HOSTS.len());
        for (cookie, &host) in cookies.iter().zip(SESSION_HOSTS.iter()) {
            assert_eq!(cookie.name(), "steamLoginSecure");
            assert_eq!(cookie.value(), format!("{STEAM_ID}%7C%7C{ACCESS_TOKEN}"));
            assert_eq!(cookie.domain(), Some(host));
            assert_eq!(cookie.path(), Some("/"));
        }
    }

    #[cfg(feature = "cm")]
    #[test]
    fn session_ids_are_random_and_the_same_on_every_host() {
        let first: Vec<_> = session_id_cookies().collect();
        let second: Vec<_> = session_id_cookies().collect();

        assert_eq!(first.len(), SESSION_HOSTS.len());
        for cookie in &first {
            assert_eq!(cookie.name(), "sessionid");
            assert_eq!(cookie.value(), first[0].value());
            assert_eq!(cookie.value().len(), 24);
            assert!(cookie.value().bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        assert_ne!(first[0].value(), second[0].value());
    }

    #[cfg(feature = "cm")]
    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(encode_uri_component("a||b"), "a%7C%7Cb");
        assert_eq!(encode_uri_component("A-z_0.9!~*'()"), "A-z_0.9!~*'()");
        assert_eq!(encode_uri_component(" /+=é"), "%20%2F%2B%3D%C3%A9");
    }

    #[cfg(feature = "cm")]
    const DAY: Duration = Duration::from_secs(86400);

    #[cfg(feature = "cm")]
    fn issued(now: Instant, lasts: Duration) -> TokenState {
        TokenState::issued(now, lasts).unwrap()
    }

    #[cfg(feature = "cm")]
    #[test]
    fn tokens_are_renewed_the_margin_before_they_expire() {
        let now = Instant::now();
        let state = issued(now, DAY);

        assert_eq!(state.expires, now + DAY);
        assert_eq!(state.due(DEFAULT_RENEW_BEFORE), now + DAY - Duration::from_secs(3600));
        assert_eq!(
            state.due(Duration::from_secs(300)),
            now + DAY - Duration::from_secs(300)
        );
    }

    #[cfg(feature = "cm")]
    #[test]
    fn tokens_issued_for_less_than_the_margin_are_renewed_after_the_least_interval() {
        let now = Instant::now();
        for lasts in [
            Duration::ZERO,
            Duration::from_secs(60),
            DEFAULT_RENEW_BEFORE,
            DEFAULT_RENEW_BEFORE + Duration::from_secs(1),
        ] {
            let state = issued(now, lasts);
            assert_eq!(state.expires, now + lasts);
            assert_eq!(state.due(DEFAULT_RENEW_BEFORE), now + MIN_RENEWAL_INTERVAL, "{lasts:?}");
        }
        // a margin of the token's lifetime or more: renewed after the least interval, however far it reaches
        for renew_before in [DAY, DAY + Duration::from_secs(1), Duration::MAX] {
            assert_eq!(issued(now, DAY).due(renew_before), now + MIN_RENEWAL_INTERVAL);
        }
    }

    #[cfg(feature = "cm")]
    #[test]
    fn an_expired_token_is_renewed_within_the_least_interval() {
        let now = Instant::now();
        let state = issued(now, Duration::from_secs(10));

        assert!(matches!(
            state.check(now + Duration::from_secs(9), DEFAULT_RENEW_BEFORE),
            Check::Go
        ));
        assert!(matches!(
            state.check(now + Duration::from_secs(10), DEFAULT_RENEW_BEFORE),
            Check::Renew { valid: false }
        ));
    }

    #[cfg(feature = "cm")]
    #[test]
    fn a_lifetime_past_the_clock_is_an_error() {
        let error = TokenState::issued(Instant::now(), Duration::MAX).err().unwrap();
        assert!(matches!(error, ClientError::InvalidToken(_)), "{:?}", error);

        // the claims of the review's panic: a token issued at 1 that expires at `i64::MAX`
        let token = client_token(&format!(r#"{{"sub":"{STEAM_ID}","iat":1,"exp":{}}}"#, i64::MAX));
        let token = IssuedToken::from_client(token);
        let issuer = Box::new(FakeIssuer::answering([]));
        let error = ClientLogin::new(issuer, token.lasts, Instant::now()).err().unwrap();
        assert!(matches!(error, ClientError::InvalidToken(_)), "{:?}", error);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn the_first_tokens_lifetime_counts_from_the_logins_ask() {
        let start = Instant::now();
        let clock = FakeClock::at(start);
        let answers = [
            Ok(IssuedToken {
                secret: "first".to_string(),
                lasts: DAY,
            }),
            Err(ClientError::Dropped),
        ];
        let issuer = FakeIssuer::answering_after(answers, &clock, Duration::from_secs(10));

        let (login, secret) = ClientLogin::start(Box::new(issuer.clone()), || clock.now())
            .await
            .unwrap();
        assert_eq!(secret, "first");
        assert_eq!(clock.now(), start + Duration::from_secs(10));

        // due a day less the margin after the ask, 10 seconds before that after the answer
        let due = start + DAY - DEFAULT_RENEW_BEFORE;
        login
            .renew_if_due(|| due - Duration::from_secs(1), |_| {})
            .await
            .unwrap();
        assert_eq!(issuer.asked(), 1);
        login.renew_if_due(|| due, |_| {}).await.unwrap();
        assert_eq!(issuer.asked(), 2);
    }

    #[cfg(feature = "cm")]
    #[tokio::test]
    async fn a_failed_login_downcasts_as_a_failed_renewal() {
        let issuer = FakeIssuer::answering([Err(ClientError::Closed)]);
        let error = ClientLogin::start(Box::new(issuer), Instant::now).await.err().unwrap();

        assert!(matches!(error, InternalError::ClientToken(_)), "{:?}", error);
        let source = std::error::Error::source(&error).unwrap();
        // as `client::tests::client_error` reads a failed renewal's
        let client_error = source.downcast_ref::<Arc<ClientError>>().unwrap();
        assert!(matches!(client_error.as_ref(), ClientError::Closed));
    }

    /// A token Steam could have issued, with these claims.
    #[cfg(feature = "cm")]
    fn client_token(claims: &str) -> steam_client::auth::AccessToken {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let jwt = format!(
            "{}.{}.c2lnbmF0dXJl",
            URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT","alg":"EdDSA"}"#),
            URL_SAFE_NO_PAD.encode(claims)
        );
        steam_client::auth::AccessToken::new(jwt).unwrap()
    }

    #[cfg(feature = "cm")]
    #[test]
    fn a_tokens_lifetime_is_read_from_its_claims_whatever_the_local_clock() {
        // issued and expiring in 2023, long before this clock's now: its lifetime still counts
        let token = client_token(&format!(r#"{{"sub":"{STEAM_ID}","iat":1700000000,"exp":1700086400}}"#));
        let issued = IssuedToken::from_client(token.clone());
        assert_eq!(issued.lasts, Duration::from_secs(86400));
        assert_eq!(issued.secret, token.expose_secret());
    }

    #[cfg(feature = "cm")]
    #[test]
    fn without_an_issue_time_a_tokens_expiry_is_read_by_the_local_clock() {
        let in_an_hour = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let lasts = |claims: String| IssuedToken::from_client(client_token(&claims)).lasts;

        let without_iat = lasts(format!(r#"{{"sub":"{STEAM_ID}","exp":{in_an_hour}}}"#));
        assert!(without_iat <= Duration::from_secs(3600) && without_iat > Duration::from_secs(3590));
        // an `iat` after the `exp` is no lifetime either
        let late_iat = lasts(format!(
            r#"{{"sub":"{STEAM_ID}","iat":{},"exp":{in_an_hour}}}"#,
            in_an_hour + 1
        ));
        assert!(late_iat <= Duration::from_secs(3600) && late_iat > Duration::from_secs(3590));
        // an expired one lasts nothing
        assert_eq!(lasts(format!(r#"{{"sub":"{STEAM_ID}","exp":1}}"#)), Duration::ZERO);
    }

    #[test]
    fn access_token_debug_hides_the_token() {
        let token = AccessToken::new(ACCESS_TOKEN.to_string());
        let debug = format!("{token:?}");
        assert!(!debug.contains(ACCESS_TOKEN));
        assert_eq!(debug, r#"AccessToken("<redacted>")"#);
        assert_eq!(token.expose_secret(), ACCESS_TOKEN);
    }
}

//! Authenticating with `IAuthenticationService` over the CM connection, as the Steam client does: an account name and
//! password, then Steam Guard, give a refresh token, which logs on.
//!
//! ```no_run
//! # async fn run(shared_secret: steam_totp::Secret) -> Result<(), steam_client::errors::ClientError> {
//! use steam_client::auth::SteamGuard;
//! use steam_client::client::SteamClient;
//!
//! let client = SteamClient::connect()
//!     .await?
//!     .login("account", "password", SteamGuard::SharedSecret(shared_secret))
//!     .await?;
//! // save it: it logs on again without the password, and Steam may replace it
//! let refresh_token = client.refresh_token().expose_secret();
//! # Ok(())
//! # }
//! ```
//!
//! For an email code, or to choose how to answer Steam Guard, begin an [`AuthSession`] instead.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use bytes::Bytes;
use rsa::BigUint;
use rsa::Pkcs1v15Encrypt;
use rsa::RsaPublicKey;
use serde::Deserialize;
use steam_protobuf::protobufs::enums::ESessionPersistence;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_DeviceDetails;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::EAuthSessionGuardType;
use steam_protobuf::protobufs::steammessages_auth_steamclient::EAuthTokenPlatformType;
use steam_totp::Secret;
use steamid_parser::SteamID;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;

use crate::client::OS_TYPE;
use crate::connection::Connection;
use crate::errors::ClientError;
use crate::service::methods::BeginAuthSessionViaCredentials;
use crate::service::methods::GetPasswordRSAPublicKey;
use crate::service::ResponseOf;
use crate::service::ServiceRequest;

/// How Steam shows the device in the account's authorized devices.
const DEVICE_FRIENDLY_NAME: &str = "SteamHelper-rs";
/// `EGamingDeviceType` of a PC, as SteamKit sends.
const GAMING_DEVICE_STANDARD_PC: u32 = 1;
/// Bounds on the poll interval Steam asks for.
const MIN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// A refresh token: logs on, and gets access tokens. Steam issues it for about 200 days, and may replace it when
/// renewing, which revokes this one, so save the new one.
///
/// Debug hides it.
#[derive(Clone)]
pub struct RefreshToken(Jwt);

/// An access token, for the Steam websites and Web API. Lasts about a day.
///
/// Debug hides it.
#[derive(Clone)]
pub struct AccessToken(Jwt);

macro_rules! tokens {
    ($($token:ident),*) => {
        $(
            impl $token {
                /// Parses a token Steam issued, as saved.
                pub fn new(token: String) -> Result<Self, ClientError> {
                    Jwt::parse(token).map(Self)
                }

                /// The token itself, to save or send. Keep it secret.
                pub fn expose_secret(&self) -> &str {
                    &self.0.token
                }

                /// The account the token is for.
                pub fn steam_id(&self) -> SteamID {
                    self.0.steam_id
                }

                /// When the token expires.
                pub fn expires_at(&self) -> SystemTime {
                    self.0.expires_at
                }

                /// When Steam issued the token (its `iat` claim), if it says. Steam's clock, as [`Self::expires_at`]
                /// is: the time between the two is how long the token lasts, whatever this machine's clock says.
                pub fn issued_at(&self) -> Option<SystemTime> {
                    self.0.issued_at
                }
            }

            impl Debug for $token {
                fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                    f.debug_struct(stringify!($token))
                        .field("steam_id", &self.0.steam_id)
                        .field("expires_at", &self.0.expires_at)
                        .finish_non_exhaustive()
                }
            }
        )*
    };
}

tokens!(RefreshToken, AccessToken);

/// A JWT Steam issued, with the claims we use.
#[derive(Clone)]
struct Jwt {
    token: String,
    steam_id: SteamID,
    expires_at: SystemTime,
    issued_at: Option<SystemTime>,
}

impl Jwt {
    fn parse(token: String) -> Result<Self, ClientError> {
        #[derive(Deserialize)]
        struct Claims {
            sub: String,
            exp: u64,
            iat: Option<u64>,
        }

        // the errors never show the token
        let invalid = |reason: &str| ClientError::InvalidToken(reason.to_string());
        let mut parts = token.split('.');
        let payload = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(_), Some(payload), Some(_), None) => payload,
            _ => return Err(invalid("not a JWT")),
        };
        let payload = URL_SAFE_NO_PAD
            .decode(payload.trim_end_matches('='))
            .map_err(|_| invalid("its claims are not base64"))?;
        let claims: Claims = serde_json::from_slice(&payload).map_err(|_| invalid("no `sub` and `exp` claims"))?;
        let steam_id = claims
            .sub
            .parse()
            .map(SteamID::from_steam64)
            .map_err(|_| invalid("its subject is not a SteamID"))?;

        let expires_at = UNIX_EPOCH
            .checked_add(Duration::from_secs(claims.exp))
            .ok_or_else(|| invalid("its expiry is out of range"))?;

        Ok(Self {
            token,
            steam_id,
            expires_at,
            issued_at: claims
                .iat
                .and_then(|iat| UNIX_EPOCH.checked_add(Duration::from_secs(iat))),
        })
    }
}

/// The tokens an [`AuthSession`] ends with.
#[derive(Debug)]
pub struct AuthTokens {
    account_name: String,
    refresh_token: RefreshToken,
}

impl AuthTokens {
    /// The account name, as Steam spells it.
    pub fn account_name(&self) -> &str {
        &self.account_name
    }

    /// The refresh token, to log on with.
    pub fn refresh_token(&self) -> &RefreshToken {
        &self.refresh_token
    }

    /// The account name and the refresh token.
    pub fn into_parts(self) -> (String, RefreshToken) {
        (self.account_name, self.refresh_token)
    }
}

/// A way Steam accepts to confirm an [`AuthSession`], in Steam's order of preference.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Confirmation {
    /// Nothing to confirm: polling gives the tokens.
    None,
    /// A code Steam emailed to an address at `email_domain`: [`AuthSession::submit_email_code`].
    EmailCode {
        /// The domain of the address.
        email_domain: String,
    },
    /// A code of the mobile authenticator: [`AuthSession::submit_device_code`].
    DeviceCode,
    /// Approving the logon in the Steam mobile app, while polling.
    DeviceConfirmation,
    /// Following the link Steam emailed, while polling.
    EmailConfirmation,
    /// Steam remembers this machine: polling gives the tokens.
    MachineToken,
    /// A kind we don't know.
    Unknown(i32),
}

impl Confirmation {
    fn new(kind: EAuthSessionGuardType, message: &str) -> Self {
        use EAuthSessionGuardType::*;
        match kind {
            k_EAuthSessionGuardType_None => Self::None,
            k_EAuthSessionGuardType_EmailCode => Self::EmailCode {
                email_domain: message.to_string(),
            },
            k_EAuthSessionGuardType_DeviceCode => Self::DeviceCode,
            k_EAuthSessionGuardType_DeviceConfirmation => Self::DeviceConfirmation,
            k_EAuthSessionGuardType_EmailConfirmation => Self::EmailConfirmation,
            k_EAuthSessionGuardType_MachineToken => Self::MachineToken,
            other => Self::Unknown(other as i32),
        }
    }

    /// Whether polling alone can complete it, with nothing to submit.
    fn needs_no_code(&self) -> bool {
        !matches!(self, Self::EmailCode { .. } | Self::DeviceCode | Self::Unknown(_))
    }
}

/// How [`SteamClient::login`](crate::client::SteamClient) answers Steam Guard.
// passed once, by value
#[allow(clippy::large_enum_variant)]
pub enum SteamGuard {
    /// Submits a code of the mobile authenticator, generated from its shared secret (the maFile's `shared_secret`).
    SharedSecret(Secret),
    /// Submits nothing, and polls: for accounts Steam doesn't challenge, or to approve the logon in the mobile app or
    /// by email link.
    NoCode,
}

impl Debug for SteamGuard {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedSecret(_) => f.write_str("SharedSecret(<redacted>)"),
            Self::NoCode => f.write_str("NoCode"),
        }
    }
}

/// An authentication with an account name and password, waiting for Steam Guard.
///
/// Submit a code if [`AuthSession::confirmations`] asks for one, then [`AuthSession::poll`] for the tokens. It borrows
/// the client, whose connection it calls `Authentication` over.
pub struct AuthSession<'a, T = TcpStream> {
    connection: &'a mut Connection<T>,
    account_name: String,
    client_id: u64,
    request_id: Bytes,
    steam_id: SteamID,
    interval: Duration,
    confirmations: Vec<Confirmation>,
}

impl<T> Debug for AuthSession<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSession")
            .field("account_name", &self.account_name)
            .field("steam_id", &self.steam_id)
            .field("interval", &self.interval)
            .field("confirmations", &self.confirmations)
            .finish_non_exhaustive()
    }
}

impl<'a, T> AuthSession<'a, T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Encrypts the password with the account's key, and begins the session. Steam sends any email code now.
    pub(crate) async fn begin(
        connection: &'a mut Connection<T>,
        account_name: &str,
        password: &str,
    ) -> Result<AuthSession<'a, T>, ClientError> {
        let mut key_request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
        key_request.set_account_name(account_name.to_string());
        let key = connection.call::<GetPasswordRSAPublicKey>(key_request).await?;
        let encrypted_password = encrypt_password(password, key.publickey_mod(), key.publickey_exp())?;

        let mut details = CAuthentication_DeviceDetails::new();
        details.set_device_friendly_name(DEVICE_FRIENDLY_NAME.to_string());
        details.set_platform_type(EAuthTokenPlatformType::k_EAuthTokenPlatformType_SteamClient);
        details.set_os_type(OS_TYPE as i32);
        details.set_gaming_device_type(GAMING_DEVICE_STANDARD_PC);

        let mut request = CAuthentication_BeginAuthSessionViaCredentials_Request::new();
        request.set_account_name(account_name.to_string());
        request.set_encrypted_password(encrypted_password);
        request.set_encryption_timestamp(key.timestamp());
        request.set_persistence(ESessionPersistence::k_ESessionPersistence_Persistent);
        request.set_website_id("Client".to_string());
        request.set_device_details(details);
        let response = connection.call::<BeginAuthSessionViaCredentials>(request).await?;

        let confirmations = response
            .allowed_confirmations
            .iter()
            .map(|allowed| Confirmation::new(allowed.confirmation_type(), allowed.associated_message()))
            .collect();
        let interval = Duration::try_from_secs_f32(response.interval())
            .unwrap_or(MIN_POLL_INTERVAL)
            .clamp(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL);

        Ok(Self {
            connection,
            account_name: account_name.to_string(),
            client_id: response.client_id(),
            request_id: response.request_id.clone().unwrap_or_default(),
            steam_id: SteamID::from_steam64(response.steamid()),
            interval,
            confirmations,
        })
    }

    /// The ways Steam accepts to confirm this session, the one it prefers first.
    pub fn confirmations(&self) -> &[Confirmation] {
        &self.confirmations
    }

    /// The account's SteamID.
    pub fn steam_id(&self) -> SteamID {
        self.steam_id
    }

    /// Submits a code of the mobile authenticator. A wrong one fails with `TwoFactorCodeMismatch`, and the session
    /// can take another.
    pub async fn submit_device_code(&mut self, code: &str) -> Result<(), ClientError> {
        self.submit_code(code, EAuthSessionGuardType::k_EAuthSessionGuardType_DeviceCode)
            .await
    }

    /// Generates the current code of the mobile authenticator from its shared secret, and submits it.
    ///
    /// Asks Steam's time server for the clock offset first, as the codes depend on the time.
    pub async fn submit_shared_secret(&mut self, shared_secret: Secret) -> Result<(), ClientError> {
        let code = steam_totp::generate_auth_code_async(shared_secret).await?;
        self.submit_device_code(&code).await
    }

    /// Submits the code Steam emailed. A wrong one fails with `InvalidLoginAuthCode`, and the session can take
    /// another.
    pub async fn submit_email_code(&mut self, code: &str) -> Result<(), ClientError> {
        self.submit_code(code, EAuthSessionGuardType::k_EAuthSessionGuardType_EmailCode)
            .await
    }

    async fn submit_code(&mut self, code: &str, kind: EAuthSessionGuardType) -> Result<(), ClientError> {
        let mut request = CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request::new();
        request.set_client_id(self.client_id);
        request.set_steamid(self.steam_id.to_steam64());
        request.set_code(code.to_string());
        request.set_code_type(kind);
        self.call(request).await?;
        debug!("Steam took the Steam Guard code");
        Ok(())
    }

    /// Polls until Steam confirms the session, and returns its tokens.
    ///
    /// Waits as long as the confirmation takes, such as approving in the mobile app, until Steam expires the
    /// session (wrap it in a timeout to wait less).
    pub async fn poll(mut self) -> Result<AuthTokens, ClientError> {
        loop {
            let mut request = CAuthentication_PollAuthSessionStatus_Request::new();
            request.set_client_id(self.client_id);
            request.set_request_id(self.request_id.clone());
            let response = self.call(request).await?;

            if response.new_client_id() != 0 {
                self.client_id = response.new_client_id();
            }
            if !response.refresh_token().is_empty() {
                let account_name = match response.account_name() {
                    "" => self.account_name,
                    account_name => account_name.to_string(),
                };
                info!("Authenticated as {}", account_name);
                return Ok(AuthTokens {
                    account_name,
                    refresh_token: RefreshToken::new(response.refresh_token().to_string())?,
                });
            }

            trace!("Not confirmed yet, polling again in {:?}", self.interval);
            tokio::time::sleep(self.interval).await;
        }
    }

    /// Answers Steam Guard as `guard` says, and polls for the tokens.
    pub(crate) async fn complete(mut self, guard: SteamGuard) -> Result<AuthTokens, ClientError> {
        let can_poll = self.confirmations.iter().any(Confirmation::needs_no_code);
        match guard {
            SteamGuard::SharedSecret(secret) if self.confirmations.contains(&Confirmation::DeviceCode) => {
                self.submit_shared_secret(secret).await?;
            }
            _ if can_poll => {}
            _ => return Err(ClientError::SteamGuard(self.confirmations)),
        }
        self.poll().await
    }

    async fn call<R: ServiceRequest>(&mut self, request: R) -> Result<ResponseOf<R>, ClientError> {
        self.connection.call::<R::Method>(request).await
    }
}

/// Encrypts the password with the account's RSA key (PKCS #1 v1.5), as `BeginAuthSessionViaCredentials` takes it:
/// in base64. The key's modulus and exponent are in hex.
fn encrypt_password(password: &str, modulus: &str, exponent: &str) -> Result<String, ClientError> {
    let invalid = || ClientError::PasswordEncryption("Steam sent an invalid RSA key");
    let modulus = BigUint::parse_bytes(modulus.as_bytes(), 16).ok_or_else(invalid)?;
    let exponent = BigUint::parse_bytes(exponent.as_bytes(), 16).ok_or_else(invalid)?;
    let key = RsaPublicKey::new(modulus, exponent).map_err(|_| invalid())?;

    let encrypted = key
        .encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, password.as_bytes())
        .map_err(|_| ClientError::PasswordEncryption("the password is too long for the key"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(encrypted))
}

#[cfg(test)]
pub(crate) mod tests {
    use rsa::traits::PublicKeyParts;
    use rsa::RsaPrivateKey;
    use steam_language_gen::generated::enums::EMsg;
    use steam_language_gen::generated::enums::EResult;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AllowedConfirmation;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Response;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Response;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response;

    use super::*;
    use crate::connection::tests::connection;
    use crate::connection::tests::soon;
    use crate::connection::tests::FakeCm;

    pub(crate) const STEAM_ID: u64 = 76561197960287930;
    const NON_AUTHED: EMsg = EMsg::ServiceMethodCallFromClientNonAuthed;

    /// A token with Steam's shape and these claims, signed with nothing. Tokens are opaque to us past the claims.
    pub(crate) fn jwt(claims: &str) -> String {
        format!(
            "{}.{}.c2lnbmF0dXJl",
            URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT","alg":"EdDSA"}"#),
            URL_SAFE_NO_PAD.encode(claims)
        )
    }

    pub(crate) fn refresh_token_jwt() -> String {
        jwt(&format!(
            r#"{{"iss":"steam","sub":"{}","aud":["client","web","renew","derive"],"exp":1800000000,"nbf":1700000000,"iat":1700000000,"jti":"0001_ABCD"}}"#,
            STEAM_ID
        ))
    }

    pub(crate) fn key() -> RsaPrivateKey {
        RsaPrivateKey::new(&mut rand::thread_rng(), 1024).unwrap()
    }

    fn decrypt(key: &RsaPrivateKey, encrypted: &str) -> String {
        let encrypted = base64::engine::general_purpose::STANDARD.decode(encrypted).unwrap();
        String::from_utf8(key.decrypt(Pkcs1v15Encrypt, &encrypted).unwrap()).unwrap()
    }

    fn hex(value: &BigUint) -> String {
        value.to_str_radix(16)
    }

    #[test]
    fn parses_the_claims_of_tokens() {
        let token = RefreshToken::new(refresh_token_jwt()).unwrap();
        assert_eq!(token.steam_id().to_steam64(), STEAM_ID);
        assert_eq!(token.expires_at(), UNIX_EPOCH + Duration::from_secs(1_800_000_000));
        assert_eq!(token.issued_at(), Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)));
        assert_eq!(token.expose_secret(), refresh_token_jwt());
    }

    #[test]
    fn a_token_without_an_issue_time_still_parses() {
        let token = AccessToken::new(jwt(r#"{"sub":"76561197960287930","exp":1700086400}"#)).unwrap();
        assert_eq!(token.expires_at(), UNIX_EPOCH + Duration::from_secs(1_700_086_400));
        assert_eq!(token.issued_at(), None);
    }

    #[test]
    fn rejects_what_isnt_a_steam_token() {
        for token in [
            String::new(),
            "a.b".to_string(),
            "a.b.c.d".to_string(),
            "a.!!!.c".to_string(),
            jwt(r#"{"sub":"76561197960287930"}"#),
            jwt(r#"{"sub":"someone","exp":1}"#),
        ] {
            assert!(matches!(AccessToken::new(token), Err(ClientError::InvalidToken(_))));
        }
    }

    #[test]
    fn an_expiry_out_of_range_is_an_error_not_a_panic() {
        let token = jwt(&format!(r#"{{"sub":"{}","iat":1,"exp":{}}}"#, STEAM_ID, u64::MAX));
        assert!(matches!(
            AccessToken::new(token.clone()),
            Err(ClientError::InvalidToken(_))
        ));
        assert!(matches!(RefreshToken::new(token), Err(ClientError::InvalidToken(_))));
    }

    #[test]
    fn debug_hides_tokens() {
        let token = RefreshToken::new(refresh_token_jwt()).unwrap();
        let debug = format!("{:?}", token);
        assert!(!debug.contains(&refresh_token_jwt()));
        assert!(!debug.contains(refresh_token_jwt().split('.').nth(1).unwrap()));
        assert!(debug.contains("steam_id"));
    }

    #[test]
    fn reads_the_confirmations_steam_allows() {
        use EAuthSessionGuardType::*;
        let confirmations: Vec<_> = [
            k_EAuthSessionGuardType_None,
            k_EAuthSessionGuardType_EmailConfirmation,
            k_EAuthSessionGuardType_MachineToken,
            k_EAuthSessionGuardType_Unknown,
        ]
        .iter()
        .map(|kind| Confirmation::new(*kind, ""))
        .collect();
        assert_eq!(
            confirmations,
            [
                Confirmation::None,
                Confirmation::EmailConfirmation,
                Confirmation::MachineToken,
                Confirmation::Unknown(k_EAuthSessionGuardType_Unknown as i32),
            ]
        );
        assert!(confirmations[..3].iter().all(Confirmation::needs_no_code));
        assert!(!confirmations[3].needs_no_code());
    }

    #[test]
    fn debug_hides_the_shared_secret() {
        let secret = Secret::from_b64("cnOgv/KdpLoP6Nbh0GMkXkPXALQ=").unwrap();
        assert_eq!(
            format!("{:?}", SteamGuard::SharedSecret(secret)),
            "SharedSecret(<redacted>)"
        );
        assert_eq!(format!("{:?}", SteamGuard::NoCode), "NoCode");
    }

    #[test]
    fn encrypts_the_password_with_the_accounts_key() {
        let key = key();
        let encrypted = encrypt_password("hunter2", &hex(key.n()), &hex(key.e())).unwrap();
        assert_eq!(decrypt(&key, &encrypted), "hunter2");
    }

    #[test]
    fn invalid_keys_and_passwords_are_errors() {
        let key = key();
        assert!(matches!(
            encrypt_password("hunter2", "not hex", &hex(key.e())),
            Err(ClientError::PasswordEncryption(_))
        ));
        assert!(matches!(
            encrypt_password("hunter2", &hex(key.n()), "zz"),
            Err(ClientError::PasswordEncryption(_))
        ));
        assert!(matches!(
            encrypt_password(&"x".repeat(200), &hex(key.n()), &hex(key.e())),
            Err(ClientError::PasswordEncryption(_))
        ));
    }

    pub(crate) fn allowed(kind: EAuthSessionGuardType, message: &str) -> CAuthentication_AllowedConfirmation {
        let mut allowed = CAuthentication_AllowedConfirmation::new();
        allowed.set_confirmation_type(kind);
        allowed.set_associated_message(message.to_string());
        allowed
    }

    /// Answers the RSA key and the session's beginning, checking the encrypted password.
    pub(crate) async fn begins<S: AsyncRead + AsyncWrite + Unpin>(
        cm: &mut FakeCm<S>,
        key: &RsaPrivateKey,
        allowed_confirmations: Vec<CAuthentication_AllowedConfirmation>,
    ) {
        let (call, request) = cm
            .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(NON_AUTHED)
            .await;
        assert_eq!(request.account_name(), "account");
        let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
        response.set_publickey_mod(hex(key.n()));
        response.set_publickey_exp(hex(key.e()));
        response.set_timestamp(4242);
        cm.reply(&call, EResult::OK, response).await;

        let (call, request) = cm
            .expect::<CAuthentication_BeginAuthSessionViaCredentials_Request>(NON_AUTHED)
            .await;
        assert_eq!(request.account_name(), "account");
        assert_eq!(decrypt(key, request.encrypted_password()), "hunter2");
        assert_eq!(request.encryption_timestamp(), 4242);
        assert_eq!(
            request.persistence(),
            ESessionPersistence::k_ESessionPersistence_Persistent
        );
        assert_eq!(request.website_id(), "Client");
        assert_eq!(
            request.device_details.platform_type(),
            EAuthTokenPlatformType::k_EAuthTokenPlatformType_SteamClient
        );
        assert_eq!(request.device_details.os_type(), OS_TYPE as i32);

        let mut response = CAuthentication_BeginAuthSessionViaCredentials_Response::new();
        response.set_client_id(11);
        response.set_request_id(Bytes::from_static(&[7; 16]));
        response.set_interval(0.0);
        response.set_steamid(STEAM_ID);
        response.allowed_confirmations = allowed_confirmations;
        cm.reply(&call, EResult::OK, response).await;
    }

    /// Answers polls: first not confirmed (with a new client id), then with the tokens.
    pub(crate) async fn confirms<S: AsyncRead + AsyncWrite + Unpin>(cm: &mut FakeCm<S>) {
        let (call, request) = cm
            .expect::<CAuthentication_PollAuthSessionStatus_Request>(NON_AUTHED)
            .await;
        assert_eq!(request.client_id(), 11);
        assert_eq!(request.request_id(), &[7; 16]);
        let mut response = CAuthentication_PollAuthSessionStatus_Response::new();
        response.set_new_client_id(12);
        cm.reply(&call, EResult::OK, response).await;

        let (call, request) = cm
            .expect::<CAuthentication_PollAuthSessionStatus_Request>(NON_AUTHED)
            .await;
        assert_eq!(request.client_id(), 12);
        let mut response = CAuthentication_PollAuthSessionStatus_Response::new();
        response.set_refresh_token(refresh_token_jwt());
        response.set_access_token(jwt(r#"{"sub":"76561197960287930","exp":1700086400}"#));
        response.set_account_name("Account".to_string());
        cm.reply(&call, EResult::OK, response).await;
    }

    #[tokio::test(start_paused = true)]
    async fn authenticates_with_a_device_code() {
        let (mut connection, mut cm) = connection();
        let key = key();

        let server = tokio::spawn(async move {
            begins(
                &mut cm,
                &key,
                vec![
                    allowed(EAuthSessionGuardType::k_EAuthSessionGuardType_DeviceConfirmation, ""),
                    allowed(EAuthSessionGuardType::k_EAuthSessionGuardType_DeviceCode, ""),
                ],
            )
            .await;

            // a wrong code first
            let (call, request) = cm
                .expect::<CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request>(NON_AUTHED)
                .await;
            assert_eq!(request.code(), "BAD00");
            cm.reply(
                &call,
                EResult::TwoFactorCodeMismatch,
                CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response::new(),
            )
            .await;

            let (call, request) = cm
                .expect::<CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request>(NON_AUTHED)
                .await;
            assert_eq!(request.client_id(), 11);
            assert_eq!(request.steamid(), STEAM_ID);
            assert_eq!(request.code(), "2BC4F");
            assert_eq!(
                request.code_type(),
                EAuthSessionGuardType::k_EAuthSessionGuardType_DeviceCode
            );
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response::new(),
            )
            .await;

            confirms(&mut cm).await;
        });

        let mut session = AuthSession::begin(&mut connection, "account", "hunter2").await.unwrap();
        assert_eq!(session.steam_id().to_steam64(), STEAM_ID);
        assert_eq!(
            session.confirmations(),
            [Confirmation::DeviceConfirmation, Confirmation::DeviceCode]
        );
        assert!(matches!(
            session.submit_device_code("BAD00").await,
            Err(ClientError::ServiceMethod {
                result: EResult::TwoFactorCodeMismatch,
                ..
            })
        ));
        session.submit_device_code("2BC4F").await.unwrap();

        let tokens = session.poll().await.unwrap();
        assert_eq!(tokens.account_name(), "Account");
        assert_eq!(tokens.refresh_token().steam_id().to_steam64(), STEAM_ID);
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn authenticates_with_an_email_code() {
        let (mut connection, mut cm) = connection();
        let key = key();

        let server = tokio::spawn(async move {
            begins(
                &mut cm,
                &key,
                vec![allowed(
                    EAuthSessionGuardType::k_EAuthSessionGuardType_EmailCode,
                    "example.com",
                )],
            )
            .await;
            let (call, request) = cm
                .expect::<CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request>(NON_AUTHED)
                .await;
            assert_eq!(request.code(), "F00BA");
            assert_eq!(
                request.code_type(),
                EAuthSessionGuardType::k_EAuthSessionGuardType_EmailCode
            );
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response::new(),
            )
            .await;
            confirms(&mut cm).await;
        });

        let mut session = AuthSession::begin(&mut connection, "account", "hunter2").await.unwrap();
        assert_eq!(
            session.confirmations(),
            [Confirmation::EmailCode {
                email_domain: "example.com".to_string()
            }]
        );
        session.submit_email_code("F00BA").await.unwrap();
        session.poll().await.unwrap();
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn polls_when_no_code_is_needed() {
        let (mut connection, mut cm) = connection();
        let key = key();

        let server = tokio::spawn(async move {
            begins(
                &mut cm,
                &key,
                vec![allowed(EAuthSessionGuardType::k_EAuthSessionGuardType_None, "")],
            )
            .await;
            confirms(&mut cm).await;
        });

        let session = AuthSession::begin(&mut connection, "account", "hunter2").await.unwrap();
        let debug = format!("{:?}", session);
        assert!(debug.contains("account") && !debug.contains("hunter2"), "{}", debug);
        let tokens = session.complete(SteamGuard::NoCode).await.unwrap();
        assert_eq!(tokens.account_name(), "Account");
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn refuses_a_guard_it_cant_answer() {
        let (mut connection, mut cm) = connection();
        let key = key();

        let server = tokio::spawn(async move {
            begins(
                &mut cm,
                &key,
                vec![allowed(
                    EAuthSessionGuardType::k_EAuthSessionGuardType_EmailCode,
                    "example.com",
                )],
            )
            .await;
        });

        let session = AuthSession::begin(&mut connection, "account", "hunter2").await.unwrap();
        let secret = Secret::from_b64("cnOgv/KdpLoP6Nbh0GMkXkPXALQ=").unwrap();
        match session.complete(SteamGuard::SharedSecret(secret)).await {
            Err(ClientError::SteamGuard(confirmations)) => assert_eq!(
                confirmations,
                [Confirmation::EmailCode {
                    email_domain: "example.com".to_string()
                }]
            ),
            other => panic!("expected a Steam Guard error, got {:?}", other),
        }
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn wrong_passwords_are_errors() {
        let (mut connection, mut cm) = connection();
        let key = key();

        let server = tokio::spawn(async move {
            let (call, _) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(NON_AUTHED)
                .await;
            let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
            response.set_publickey_mod(hex(key.n()));
            response.set_publickey_exp(hex(key.e()));
            cm.reply(&call, EResult::OK, response).await;

            let (call, _) = cm
                .expect::<CAuthentication_BeginAuthSessionViaCredentials_Request>(NON_AUTHED)
                .await;
            cm.reply(
                &call,
                EResult::InvalidPassword,
                CAuthentication_BeginAuthSessionViaCredentials_Response::new(),
            )
            .await;
        });

        let error = AuthSession::begin(&mut connection, "account", "wrong")
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                ClientError::ServiceMethod {
                    result: EResult::InvalidPassword,
                    ..
                }
            ),
            "{:?}",
            error
        );
        soon(server).await.unwrap();
    }
}

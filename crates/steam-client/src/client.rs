//! Connects to a Steam CM server and logs on.
//!
//! The client is a type state: [`SteamClient<Connected>`] is connected over an encrypted channel, and logging on
//! consumes it into a [`SteamClient<LoggedOn<Anonymous>>`] or, for an account, a [`SteamClient<LoggedOn<User>>`].
//! See [`crate::auth`] to log on to an account.
//!
//! ```no_run
//! # async fn run() -> Result<(), steam_client::errors::ClientError> {
//! use steam_client::client::SteamClient;
//!
//! let client = SteamClient::connect().await?.login_anonymous().await?;
//! println!("Logged on as {}", client.steam_id());
//! client.log_off().await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Write;
use std::net::Ipv4Addr;
use std::time::Duration;

use rand::seq::SliceRandom;
use sha1::Digest;
use sha1::Sha1;
use steam_language_gen::generated::enums::EAccountType;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EOSType;
use steam_language_gen::generated::enums::EPersonaState;
use steam_language_gen::generated::enums::EUniverse;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::ETokenRenewalType;
use steam_protobuf::protobufs::steammessages_clientserver::cmsg_client_games_played::GamePlayed;
use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGamesPlayed;
use steam_protobuf::protobufs::steammessages_clientserver_friends::CMsgClientChangeStatus;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
use steamid_parser::SteamID;
use tokio::time::timeout;

use crate::auth::AccessToken;
use crate::auth::AuthSession;
use crate::auth::AuthTokens;
use crate::auth::RefreshToken;
use crate::auth::SteamGuard;
use crate::cm_list::dump_tcp_servers;
use crate::connection::Connection;
use crate::errors::ClientError;
use crate::events::Event;
use crate::gc;
use crate::handle;
use crate::handle::Events;
use crate::handle::SteamHandle;
use crate::handle::SteamTask;
use crate::job::JobMessage;
use crate::job::JobRequest;
use crate::job::JobResponseOf;
use crate::protocol::LogonInfo;
use crate::service;
use crate::service::ResponseOf;
use crate::service::ServiceMethod;
use crate::service::ServiceRequest;

/// `MsgClientLogon.CurrentProtocol` in SteamKit's `steammsg.steamd`.
const PROTOCOL_VERSION: u32 = 65581;
/// `client_package_version` SteamKit sends on account logons.
const CLIENT_PACKAGE_VERSION: u32 = 1771;
/// `chat_mode` for the new Steam chat, which friend messages come through: SteamKit's `ChatMode.NewSteamChat`, and
/// what node-steam-user always sends.
const NEW_STEAM_CHAT: u32 = 2;
/// How many servers of the CM list [`SteamClient::connect`] tries.
const SERVERS_TO_TRY: usize = 5;
/// How long a connection to a server, and the channel encryption, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long Steam's directory has to give the CM list.
const DIRECTORY_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(target_os = "windows")]
pub(crate) const OS_TYPE: EOSType = EOSType::Windows10;
#[cfg(target_os = "macos")]
pub(crate) const OS_TYPE: EOSType = EOSType::MacOSUnknown;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) const OS_TYPE: EOSType = EOSType::LinuxUnknown;

/// A client of a Steam CM server, in state `S`: [`Connected`] or [`LoggedOn`].
///
/// The client has no task of its own: it reads and writes the connection while one of its methods is awaited, and
/// only then. Once logged on, [`SteamClient::spawn`] it to have a task drive it, as a bot should; or keep awaiting
/// [`SteamClient::next_event`], which reads what Steam sends and sends the heartbeats.
#[derive(Debug)]
pub struct SteamClient<S> {
    connection: Connection,
    /// The CM server, as `ip:port`.
    endpoint: String,
    state: S,
}

/// Connected to a CM server over an encrypted channel, and not logged on.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Connected;

/// Logged on to Steam, as an [`Anonymous`] user or a [`User`].
///
/// The client sends a heartbeat every [`SteamClient::heartbeat_interval`], but only while one of its methods is
/// awaited: a client left idle sends none, and Steam drops the connection (measured: alive after 90 s idle, gone at
/// 180 s). Keep awaiting [`SteamClient::next_event`] between calls, or [`SteamClient::spawn`] it.
#[derive(Debug)]
pub struct LoggedOn<A> {
    logon: LogonInfo,
    account: A,
}

/// Logged on anonymously, with no account.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Anonymous;

/// Logged on to an account, with its refresh token.
pub struct User {
    account_name: String,
    refresh_token: RefreshToken,
    /// The persona state last set, which a spawned client sets again after logging on again.
    persona_state: Option<EPersonaState>,
    /// The apps last set as played, which a spawned client sets again after logging on again.
    games_played: Vec<u32>,
}

impl Debug for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("User")
            .field("account_name", &self.account_name)
            .field("refresh_token", &self.refresh_token)
            .field("persona_state", &self.persona_state)
            .field("games_played", &self.games_played)
            .finish()
    }
}

/// What [`SteamClient::renew_tokens`] got.
#[derive(Debug)]
pub struct RenewedTokens {
    /// A new access token.
    pub access_token: AccessToken,
    /// The refresh token that replaced the client's, if Steam replaced it. The old one is revoked: save this one.
    pub refresh_token: Option<RefreshToken>,
}

impl<S> SteamClient<S> {
    /// The CM server, as `ip:port`.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl SteamClient<Connected> {
    /// Connects to one of the first CM servers of the list Steam's directory returns, picked at random so that many
    /// clients spread over them.
    pub async fn connect() -> Result<Self, ClientError> {
        let (connection, endpoint) = connect_to_any(None).await?;
        Self::hello(connection, &endpoint).await
    }

    /// Connects to the CM server at `endpoint` (`ip:port`), encrypts the channel and says hello.
    pub async fn connect_to(endpoint: &str) -> Result<Self, ClientError> {
        Self::hello(encrypted_connection(endpoint).await?, endpoint).await
    }

    /// Says hello on a connection with an encrypted channel.
    async fn hello(mut connection: Connection, endpoint: &str) -> Result<Self, ClientError> {
        connection.send(EMsg::ClientHello, hello()).await?;
        info!("Connected to {}", endpoint);

        Ok(Self {
            connection,
            endpoint: endpoint.to_string(),
            state: Connected,
        })
    }

    /// Calls service method `M`, before logon (`ServiceMethodCallFromClientNonAuthed`), and waits for its response.
    /// See [`ServiceMethod`] to call a method the crate doesn't know.
    pub async fn call<M: ServiceMethod>(&mut self, request: M::Request) -> Result<M::Response, ClientError> {
        service::check(&request)?;
        self.connection.call::<M>(request).await
    }

    /// Calls the method of `request`, one of [`service::methods`], as [`SteamClient::call`] does.
    pub async fn call_service_method<R: ServiceRequest>(&mut self, request: R) -> Result<ResponseOf<R>, ClientError> {
        self.call::<R::Method>(request).await
    }

    /// Begins authenticating with the account's name and password, to answer Steam Guard and get a refresh token.
    ///
    /// The password is encrypted with the account's key, and isn't sent otherwise.
    pub async fn begin_auth_session(
        &mut self,
        account_name: &str,
        password: &str,
    ) -> Result<AuthSession<'_>, ClientError> {
        AuthSession::begin(&mut self.connection, account_name, password).await
    }

    /// Authenticates with the account's name and password, answering Steam Guard as `guard` says, and returns the
    /// refresh token.
    pub async fn authenticate(
        &mut self,
        account_name: &str,
        password: &str,
        guard: SteamGuard,
    ) -> Result<AuthTokens, ClientError> {
        self.begin_auth_session(account_name, password)
            .await?
            .complete(guard)
            .await
    }

    /// Authenticates with the account's name and password, then logs on with the refresh token Steam issued.
    ///
    /// Save the token, [`SteamClient::refresh_token`], to log on again without the password.
    pub async fn login(
        mut self,
        account_name: &str,
        password: &str,
        guard: SteamGuard,
    ) -> Result<SteamClient<LoggedOn<User>>, ClientError> {
        let (account_name, refresh_token) = self.authenticate(account_name, password, guard).await?.into_parts();
        self.login_with_refresh_token(&account_name, refresh_token).await
    }

    /// Logs on to the account with a refresh token, from a previous authentication.
    pub async fn login_with_refresh_token(
        self,
        account_name: &str,
        refresh_token: RefreshToken,
    ) -> Result<SteamClient<LoggedOn<User>>, ClientError> {
        self.logged_on(User::new(account_name, refresh_token)).await
    }

    /// Logs on as an anonymous user, which can only use a few services, such as downloading public content.
    pub async fn login_anonymous(self) -> Result<SteamClient<LoggedOn<Anonymous>>, ClientError> {
        self.logged_on(Anonymous).await
    }

    /// Sends `ClientLogon` for `account`, waits for the response, and moves to the logged on state.
    async fn logged_on<A: Account>(mut self, account: A) -> Result<SteamClient<LoggedOn<A>>, ClientError> {
        let (steam_id, logon) = account.logon();
        let logon = self.connection.log_on(steam_id, logon).await?;
        info!("Logged on as {}", logon.session.steam_id);

        Ok(SteamClient {
            connection: self.connection,
            endpoint: self.endpoint,
            state: LoggedOn { logon, account },
        })
    }
}

impl<A> SteamClient<LoggedOn<A>> {
    /// Our SteamID, which Steam assigned on logon.
    pub fn steam_id(&self) -> SteamID {
        self.state.logon.session.steam_id
    }

    /// The session id Steam assigned on logon.
    pub fn session_id(&self) -> i32 {
        self.state.logon.session.session_id
    }

    /// How often the client sends a heartbeat, while one of its methods is awaited.
    pub fn heartbeat_interval(&self) -> Duration {
        self.state.logon.heartbeat_interval
    }

    /// The cell (region) Steam placed us in.
    pub fn cell_id(&self) -> u32 {
        self.state.logon.cell_id
    }

    /// Our public IP, as Steam sees it.
    pub fn public_ip(&self) -> Option<Ipv4Addr> {
        self.state.logon.public_ip
    }

    /// Calls service method `M`, in the session (`ServiceMethodCallFromClient`), and waits for its response, at most
    /// 30 s. See [`ServiceMethod`] to call a method the crate doesn't know.
    ///
    /// Messages Steam sends meanwhile are kept, as events for [`SteamClient::next_event`].
    pub async fn call<M: ServiceMethod>(&mut self, request: M::Request) -> Result<M::Response, ClientError> {
        service::check(&request)?;
        self.connection.call::<M>(request).await
    }

    /// Calls the method of `request`, one of [`service::methods`], as [`SteamClient::call`] does.
    pub async fn call_service_method<R: ServiceRequest>(&mut self, request: R) -> Result<ResponseOf<R>, ClientError> {
        self.call::<R::Method>(request).await
    }

    /// Sends job message `J`, in the session, and waits for the message answering it, at most 30 s. Fails with
    /// [`ClientError::JobMessage`] if the answer has a result other than `OK` (see [`JobMessage::result`]). See
    /// [`JobMessage`] to send a message the crate doesn't know.
    ///
    /// Messages Steam sends meanwhile are kept, as events for [`SteamClient::next_event`].
    pub async fn call_job<J: JobMessage>(&mut self, request: J::Request) -> Result<J::Response, ClientError> {
        service::check(&request)?;
        self.connection.call_job::<J>(request).await
    }

    /// Sends the job message of `request`, one of [`job::messages`](crate::job::messages), as [`SteamClient::call_job`]
    /// does.
    pub async fn call_job_message<R: JobRequest>(&mut self, request: R) -> Result<JobResponseOf<R>, ClientError> {
        self.call_job::<R::Job>(request).await
    }

    /// Waits for the next thing Steam sends that no call waits for, sending heartbeats meanwhile.
    ///
    /// Await it whenever the client isn't calling, or Steam drops the connection: in a loop, or in a `select!` with
    /// the bot's own work. It is cancel-safe: nothing is lost if it is cancelled. Steam's messages are kept until read:
    /// at most 256 from earlier reads, besides those of the latest read (a big `Multi` is kept whole); past that, the
    /// oldest go, and an [`Event::Lagged`] says how many. Once the connection is gone, it fails with
    /// [`ClientError::Dropped`]; there is no reconnecting without [`SteamClient::spawn`].
    ///
    /// ```no_run
    /// # async fn run(
    /// #     mut client: steam_client::client::SteamClient<steam_client::client::LoggedOn<steam_client::client::Anonymous>>,
    /// # ) -> Result<(), steam_client::errors::ClientError> {
    /// use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
    ///
    /// let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
    /// loop {
    ///     tokio::select! {
    ///         event = client.next_event() => println!("{:?}", event?),
    ///         _ = ticks.tick() => {
    ///             let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
    ///             request.set_account_name("someone".to_string());
    ///             let key = client.call_service_method(request).await?;
    ///             println!("{} bits", key.publickey_mod().len() * 4);
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn next_event(&mut self) -> Result<Event, ClientError> {
        self.connection.next_event().await
    }

    /// Logs off, and waits for Steam to confirm or close the connection.
    ///
    /// Dropping the client instead closes the connection without logging off.
    pub async fn log_off(mut self) -> Result<(), ClientError> {
        self.connection.log_off().await
    }
}

impl<A: Account> SteamClient<LoggedOn<A>> {
    /// Moves the client into a task of its own, which drives the connection from then on: it sends the heartbeats,
    /// reads what Steam sends, and connects and logs on again when the connection is lost. The way to run a bot.
    ///
    /// Returns a handle to call through, which clones; the [`Events`] Steam sends; and the [`SteamTask`], to await
    /// once the handles are dropped or [`SteamHandle::log_off`] was called, so that the program doesn't exit before
    /// the client logged off. See [`crate::handle`].
    ///
    /// Must be called within a tokio runtime.
    pub fn spawn(self) -> (SteamHandle<A>, Events, SteamTask) {
        handle::spawn(
            self.connection,
            self.endpoint,
            self.state.logon,
            self.state.account,
            handle::connector(|avoid| Box::pin(connect_to_any(avoid))),
        )
    }
}

impl SteamClient<LoggedOn<User>> {
    /// The account's name.
    pub fn account_name(&self) -> &str {
        &self.state.account.account_name
    }

    /// The refresh token the client logged on with, or the one that replaced it in [`SteamClient::renew_tokens`].
    pub fn refresh_token(&self) -> &RefreshToken {
        &self.state.account.refresh_token
    }

    /// Generates an access token for the Steam websites and Web API, with `Authentication.GenerateAccessTokenForApp`.
    /// It lasts about a day; generate another before it expires.
    pub async fn web_access_token(&mut self) -> Result<AccessToken, ClientError> {
        let response = self
            .call_service_method(self.access_token_request(ETokenRenewalType::k_ETokenRenewalType_None))
            .await?;
        AccessToken::new(response.access_token().to_string())
    }

    /// Generates an access token as [`SteamClient::web_access_token`] does, letting Steam replace the refresh token
    /// too, which it does when the token gets old. A replaced token is revoked, so save the new one: the client
    /// keeps it as [`SteamClient::refresh_token`].
    pub async fn renew_tokens(&mut self) -> Result<RenewedTokens, ClientError> {
        let response = self
            .call_service_method(self.access_token_request(ETokenRenewalType::k_ETokenRenewalType_Allow))
            .await?;
        let renewed = renewed_tokens(response)?;
        if let Some(refresh_token) = &renewed.refresh_token {
            self.state.account.refresh_token = refresh_token.clone();
            info!("Steam replaced the refresh token");
        }
        Ok(renewed)
    }

    /// Sets the account's persona state, such as [`EPersonaState::Online`], with `ClientChangeStatus`, as SteamKit's
    /// `SteamFriends.SetPersonaState`. Steam answers nothing.
    ///
    /// An account shows as offline until this sets it otherwise (as node-steam-user's docs say), and may get no
    /// messages from its friends meanwhile. A [spawned](SteamClient::spawn) client sets the latest state again each
    /// time it logs on again.
    pub async fn set_persona_state(&mut self, state: EPersonaState) -> Result<(), ClientError> {
        self.state.account.persona_state = Some(state);
        self.connection
            .send(EMsg::ClientChangeStatus, change_status(state))
            .await
    }

    /// Sets the apps the account plays, by app id (CS2 is 730), with `ClientGamesPlayedWithDataBlob`, as
    /// node-steam-user's `gamesPlayed`; none to stop playing. Steam answers nothing.
    ///
    /// A game's coordinator only talks to an account that plays it (see [`crate::gc`]), and the account must own the
    /// game. Friends see the account in game. A [spawned](SteamClient::spawn) client sets the latest apps again each
    /// time it logs on again.
    pub async fn set_games_played(&mut self, app_ids: &[u32]) -> Result<(), ClientError> {
        self.state.account.set_games_played(app_ids);
        self.connection
            .send(EMsg::ClientGamesPlayedWithDataBlob, games_played(app_ids))
            .await
    }

    /// Sends a protobuf message of type `msg_type` (the game's own numbering, without the protobuf flag) to the
    /// game coordinator of app `app_id`, which the account must play ([`SteamClient::set_games_played`]). What it
    /// answers comes as [`EMsg::ClientFromGC`] events: read them with
    /// [`RawMessage::gc_message`](crate::events::RawMessage::gc_message).
    ///
    /// A [spawned](SteamClient::spawn) client sends with a [`GcChannel`](gc::GcChannel) instead, which also receives.
    pub async fn send_to_gc<M: Message>(&mut self, app_id: u32, msg_type: u32, body: &M) -> Result<(), ClientError> {
        let message = gc::client_to_gc(app_id, msg_type, body)?;
        self.connection.send_routed(EMsg::ClientToGC, app_id, message).await
    }

    fn access_token_request(&self, renewal: ETokenRenewalType) -> CAuthentication_AccessToken_GenerateForApp_Request {
        access_token_request(self.refresh_token(), self.steam_id(), renewal)
    }
}

/// What a client logs on as: an [`Anonymous`] user or a [`User`]. A spawned client logs on again as it.
pub trait Account: account::Logon {}

impl Account for Anonymous {}
impl Account for User {}

pub(crate) mod account {
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
    use steamid_parser::SteamID;

    use super::*;

    /// Not public: what logs on as the account.
    pub trait Logon: Send + 'static {
        /// The `ClientLogon`, and the SteamID to send it with.
        fn logon(&self) -> (SteamID, CMsgClientLogon);

        /// The persona state to set again once logged on again, if one was set.
        fn persona_state(&self) -> Option<EPersonaState> {
            None
        }

        /// The apps to set as played again once logged on again, if any.
        fn games_played(&self) -> &[u32] {
            &[]
        }
    }

    impl Logon for Anonymous {
        fn logon(&self) -> (SteamID, CMsgClientLogon) {
            (anonymous_steam_id(), anonymous_logon())
        }
    }

    impl Logon for User {
        fn logon(&self) -> (SteamID, CMsgClientLogon) {
            (
                user_logon_steam_id(),
                user_logon(&self.account_name, &self.refresh_token),
            )
        }

        fn persona_state(&self) -> Option<EPersonaState> {
            self.persona_state
        }

        fn games_played(&self) -> &[u32] {
            &self.games_played
        }
    }
}

impl User {
    pub(crate) fn new(account_name: &str, refresh_token: RefreshToken) -> Self {
        Self {
            account_name: account_name.to_string(),
            refresh_token,
            persona_state: None,
            games_played: Vec::new(),
        }
    }

    pub(crate) fn account_name(&self) -> &str {
        &self.account_name
    }

    pub(crate) fn refresh_token(&self) -> &RefreshToken {
        &self.refresh_token
    }

    pub(crate) fn set_refresh_token(&mut self, refresh_token: RefreshToken) {
        self.refresh_token = refresh_token;
    }

    pub(crate) fn set_persona_state(&mut self, state: EPersonaState) {
        self.persona_state = Some(state);
    }

    pub(crate) fn set_games_played(&mut self, app_ids: &[u32]) {
        self.games_played = app_ids.to_vec();
    }
}

/// Connects to one of the first CM servers of the list Steam's directory returns, other than `avoid`, and encrypts
/// the channel.
async fn connect_to_any(avoid: Option<String>) -> Result<(Connection, String), ClientError> {
    let servers = timeout(DIRECTORY_TIMEOUT, dump_tcp_servers())
        .await
        .map_err(|error| ClientError::ServerList(error.into()))?
        .map_err(|error| ClientError::ServerList(error.into()))?;

    for endpoint in servers_to_try(servers, avoid.as_deref()) {
        match encrypted_connection(&endpoint).await {
            Ok(connection) => return Ok((connection, endpoint)),
            Err(error) => warn!("Could not connect to {}: {}", endpoint, error),
        }
    }
    Err(ClientError::NoServerAvailable)
}

/// The first servers of the directory's list (Steam lists the best ones for us first), but `avoid`, in a random order:
/// a client connecting again goes elsewhere than the server that just failed it, and many clients spread. `avoid` is
/// still tried if the list has nothing else.
fn servers_to_try(mut servers: Vec<String>, avoid: Option<&str>) -> Vec<String> {
    if servers.iter().any(|server| Some(server.as_str()) != avoid) {
        servers.retain(|server| Some(server.as_str()) != avoid);
    }
    servers.truncate(SERVERS_TO_TRY);
    servers.shuffle(&mut rand::thread_rng());
    servers
}

async fn encrypted_connection(endpoint: &str) -> Result<Connection, ClientError> {
    timeout(CONNECT_TIMEOUT, Connection::connect(endpoint))
        .await
        .map_err(|_| ClientError::Timeout(EMsg::ChannelEncryptResult))?
}

/// `ClientHello`, which a client sends once the channel is encrypted.
pub(crate) fn hello() -> CMsgClientHello {
    let mut hello = CMsgClientHello::new();
    hello.set_protocol_version(PROTOCOL_VERSION);
    hello
}

/// `GenerateAccessTokenForApp` with the refresh token of `steam_id`.
pub(crate) fn access_token_request(
    refresh_token: &RefreshToken,
    steam_id: SteamID,
    renewal: ETokenRenewalType,
) -> CAuthentication_AccessToken_GenerateForApp_Request {
    let mut request = CAuthentication_AccessToken_GenerateForApp_Request::new();
    request.set_refresh_token(refresh_token.expose_secret().to_string());
    request.set_steamid(steam_id.to_steam64());
    request.set_renewal_type(renewal);
    request
}

/// `ClientChangeStatus` setting the persona state only, as SteamKit's `SteamFriends.SetPersonaState`.
pub(crate) fn change_status(state: EPersonaState) -> CMsgClientChangeStatus {
    let mut status = CMsgClientChangeStatus::new();
    status.set_persona_state(state as i32 as u32);
    status
}

/// `ClientGamesPlayedWithDataBlob`'s body, playing `app_ids`: their game ids only, as node-steam-user's `gamesPlayed`
/// and SteamKit's Dota sample send them. A Steam app's game id is its app id.
pub(crate) fn games_played(app_ids: &[u32]) -> CMsgClientGamesPlayed {
    let mut games = CMsgClientGamesPlayed::new();
    for &app_id in app_ids {
        let mut game = GamePlayed::new();
        game.set_game_id(u64::from(app_id));
        games.games_played.push(game);
    }
    games
}

/// The tokens of a `GenerateAccessTokenForApp` response: a refresh token only when Steam replaced it.
pub(crate) fn renewed_tokens(
    response: CAuthentication_AccessToken_GenerateForApp_Response,
) -> Result<RenewedTokens, ClientError> {
    let refresh_token = match response.refresh_token() {
        "" => None,
        refresh_token => Some(RefreshToken::new(refresh_token.to_string())?),
    };
    Ok(RenewedTokens {
        access_token: AccessToken::new(response.access_token().to_string())?,
        refresh_token,
    })
}

/// The SteamID anonymous users log on with: an anonymous user in the public universe, instance 0, account id 0.
fn anonymous_steam_id() -> SteamID {
    SteamID::from_steam64((EUniverse::Public as u64) << 56 | (EAccountType::AnonUser as u64) << 52)
}

/// The SteamID account logons go with, before Steam assigns ours, as SteamKit's: an individual in the public
/// universe, desktop instance 1, account id 0.
fn user_logon_steam_id() -> SteamID {
    SteamID::from_steam64((EUniverse::Public as u64) << 56 | (EAccountType::Individual as u64) << 52 | 1 << 32)
}

/// `ClientLogon` for an account, with a refresh token in place of the password, as SteamKit's `SteamUser.LogOn`, and
/// in the new Steam chat, as node-steam-user. It goes with [`user_logon_steam_id`].
fn user_logon(account_name: &str, refresh_token: &RefreshToken) -> CMsgClientLogon {
    let mut logon = CMsgClientLogon::new();
    logon.set_protocol_version(PROTOCOL_VERSION);
    logon.set_client_os_type(OS_TYPE as i32 as u32);
    logon.set_client_language("english".to_string());
    logon.set_cell_id(0);
    logon.set_client_package_version(CLIENT_PACKAGE_VERSION);
    logon.set_supports_rate_limit_response(true);
    logon.set_machine_id(machine_id(account_name).into());
    logon.set_account_name(account_name.to_string());
    logon.set_access_token(refresh_token.expose_secret().to_string());
    logon.set_should_remember_password(true);
    logon.set_chat_mode(NEW_STEAM_CHAT);
    logon
}

/// A machine id for the account, the same on every logon: the binary KeyValues `MessageObject` of three hashes,
/// derived from the account name as node-steam-user's default (`AccountNameGenerated`).
fn machine_id(account_name: &str) -> Vec<u8> {
    let mut machine_id = Vec::with_capacity(155);
    machine_id.push(0);
    machine_id.extend_from_slice(b"MessageObject\0");
    for key in ["BB3", "FF2", "3B3"] {
        let hash = Sha1::digest(format!("SteamUser Hash {} {}", key, account_name).as_bytes());
        let hash = hash.iter().fold(String::with_capacity(40), |mut hex, byte| {
            let _ = write!(hex, "{:02x}", byte);
            hex
        });
        machine_id.push(1);
        machine_id.extend_from_slice(key.as_bytes());
        machine_id.push(0);
        machine_id.extend_from_slice(hash.as_bytes());
        machine_id.push(0);
    }
    // the ends of the object and of the document
    machine_id.extend_from_slice(&[8, 8]);
    machine_id
}

/// `ClientLogon` for an anonymous user, as SteamKit's `SteamUser.LogOnAnonymous`. It goes with
/// [`anonymous_steam_id`].
fn anonymous_logon() -> CMsgClientLogon {
    let mut logon = CMsgClientLogon::new();
    logon.set_protocol_version(PROTOCOL_VERSION);
    logon.set_client_os_type(OS_TYPE as i32 as u32);
    logon.set_client_language("english".to_string());
    logon.set_cell_id(0);
    logon
}

#[cfg(test)]
mod tests {
    use steam_language_gen::generated::enums::EAccountType;
    use steam_language_gen::generated::enums::EResult;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::EAuthSessionGuardType;
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;

    use super::*;
    use crate::auth::tests::allowed;
    use crate::auth::tests::begins;
    use crate::auth::tests::confirms;
    use crate::auth::tests::jwt;
    use crate::auth::tests::refresh_token_jwt;
    use crate::auth::tests::STEAM_ID;
    use crate::connection::tests::key;
    use crate::connection::tests::within;
    use crate::connection::tests::FakeCm;
    use crate::protocol::proto_message;
    use crate::protocol::tests::logs;
    use crate::protocol::tests::ANONYMOUS_LOGON_RESPONSE;
    use crate::protocol::tests::ANONYMOUS_STEAM_ID;
    use crate::protocol::CmProtocol;

    #[test]
    fn anonymous_steam_id_is_an_anonymous_public_user() {
        let steam_id = anonymous_steam_id();
        assert_eq!(steam_id.to_steam64(), 0x01A0_0000_0000_0000);
        assert_eq!(steam_id.universe(), Some(EUniverse::Public));
        assert_eq!(steam_id.account_type(), Some(EAccountType::AnonUser));
        assert_eq!(steam_id.instance(), 0);
        assert_eq!(steam_id.account_id(), 0);
    }

    #[test]
    fn writes_the_anonymous_logon() {
        let body = anonymous_logon();
        assert_eq!(body.protocol_version(), 65581);
        assert_eq!(body.client_os_type(), OS_TYPE as i32 as u32);
        assert_eq!(body.client_language(), "english");
        assert!(body.has_cell_id());
        assert!(!body.has_account_name());
        assert!(!body.has_access_token());
        assert!(!body.has_chat_mode());
    }

    #[test]
    fn writes_the_account_logon_with_the_refresh_token() {
        // individual, public universe, desktop instance, no account id yet
        assert_eq!(user_logon_steam_id().to_steam64(), 0x0110_0001_0000_0000);

        let refresh_token = RefreshToken::new(refresh_token_jwt()).unwrap();
        let body = user_logon("account", &refresh_token);
        assert_eq!(body.protocol_version(), 65581);
        assert_eq!(body.account_name(), "account");
        assert_eq!(body.access_token(), refresh_token_jwt());
        assert!(!body.has_password());
        assert!(body.should_remember_password());
        assert!(body.supports_rate_limit_response());
        assert_eq!(body.client_package_version(), 1771);
        assert_eq!(body.machine_id(), &machine_id("account")[..]);
        assert_eq!(body.chat_mode(), 2);
    }

    #[test]
    fn machine_ids_are_node_steam_users_account_name_generated() {
        let machine_id = machine_id("account");

        assert_eq!(machine_id.len(), 155);
        assert!(machine_id.starts_with(b"\0MessageObject\0\x01BB3\0"));
        // sha1("SteamUser Hash BB3 account"), from Python's hashlib
        assert_eq!(&machine_id[20..60], b"06faf51f5f008e190ca45d6666b57065713248ee");
        assert_eq!(&machine_id[61..66], b"\x01FF2\0");
        assert_eq!(&machine_id[107..112], b"\x013B3\0");
        assert!(machine_id.ends_with(b"\0\x08\x08"));
        assert_eq!(super::machine_id("account"), machine_id);
        assert_ne!(super::machine_id("another"), machine_id);
    }

    #[test]
    fn tries_the_first_servers_but_the_one_to_avoid() {
        let servers: Vec<_> = (0..8).map(|server| format!("10.0.0.{}:27017", server)).collect();
        for _ in 0..20 {
            let mut tried = servers_to_try(servers.clone(), Some("10.0.0.1:27017"));
            assert_eq!(tried.len(), SERVERS_TO_TRY);
            tried.sort();
            assert_eq!(
                tried,
                [
                    "10.0.0.0:27017",
                    "10.0.0.2:27017",
                    "10.0.0.3:27017",
                    "10.0.0.4:27017",
                    "10.0.0.5:27017"
                ]
            );
        }
        let mut tried = servers_to_try(servers.clone(), None);
        tried.sort();
        assert_eq!(tried, servers[..SERVERS_TO_TRY]);
        // in a random order: not always the list's
        assert!((0..20).any(|_| servers_to_try(servers.clone(), None) != servers[..SERVERS_TO_TRY]));
        assert!(servers_to_try(vec![], Some("10.0.0.1:27017")).is_empty());
        let only = vec!["10.0.0.1:27017".to_string()];
        assert_eq!(servers_to_try(only.clone(), Some("10.0.0.1:27017")), only);
    }

    #[test]
    fn renewing_keeps_the_refresh_token_unless_steam_replaced_it() {
        let mut response = CAuthentication_AccessToken_GenerateForApp_Response::new();
        response.set_access_token(refresh_token_jwt());
        let renewed = renewed_tokens(response.clone()).unwrap();
        assert_eq!(renewed.access_token.steam_id().to_steam64(), 76561197960287930);
        assert!(renewed.refresh_token.is_none());

        response.set_refresh_token(refresh_token_jwt());
        assert!(renewed_tokens(response.clone()).unwrap().refresh_token.is_some());

        response.set_access_token(String::new());
        assert!(matches!(renewed_tokens(response), Err(ClientError::InvalidToken(_))));
    }

    /// Fails the test if `future` takes longer than a few seconds: these tests run in real time, over TCP.
    #[track_caller]
    fn quickly<F: std::future::Future>(future: F) -> impl std::future::Future<Output = F::Output> {
        within(Duration::from_secs(10), future)
    }

    /// A client connected over loopback TCP to a fake CM, the channel already encrypted, and the fake CM.
    async fn connected() -> (SteamClient<Connected>, FakeCm<TcpStream>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let (stream, accepted) = tokio::join!(TcpStream::connect(&endpoint), listener.accept());
        let connection = Connection::new(stream.unwrap(), CmProtocol::encrypted(key()));
        let mut cm = FakeCm::new(accepted.unwrap().0, key());

        let (client, hello) = tokio::join!(SteamClient::hello(connection, &endpoint), cm.read());
        assert_eq!(hello.emsg, EMsg::ClientHello);
        assert_eq!(hello.body::<CMsgClientHello>().protocol_version(), PROTOCOL_VERSION);
        let client = client.unwrap();
        assert_eq!(client.endpoint(), endpoint);
        (client, cm)
    }

    fn logged_off() -> Vec<u8> {
        let mut logged_off = CMsgClientLoggedOff::new();
        logged_off.set_eresult(EResult::OK as i32);
        proto_message(EMsg::ClientLoggedOff, CMsgProtoBufHeader::new(), logged_off)
    }

    #[tokio::test]
    async fn connects_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let server = tokio::spawn(async move {
            let mut steam = FakeCm::new(listener.accept().await.unwrap().0, key());
            steam.encrypt_channel(EResult::OK).await;
            // the hello is encrypted with a key the fake CM can't decrypt
            steam.closed().await;
        });

        let client = SteamClient::connect_to(&endpoint).await.unwrap();
        assert_eq!(client.endpoint(), endpoint);
        drop(client);
        quickly(server).await.unwrap();
    }

    #[tokio::test]
    async fn connecting_to_nothing_is_an_error() {
        // bound, so the port isn't taken meanwhile, but never accepting
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        assert!(matches!(
            SteamClient::connect_to(&endpoint).await,
            Err(ClientError::Packet(_))
        ));
    }

    #[tokio::test]
    async fn calls_before_logon() {
        let (mut client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            let (call, _) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(EMsg::ServiceMethodCallFromClientNonAuthed)
                .await;
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            )
            .await;
        });

        client
            .call_service_method(CAuthentication_GetPasswordRSAPublicKey_Request::new())
            .await
            .unwrap();
        quickly(server).await.unwrap();
    }

    #[tokio::test]
    async fn logs_on_anonymously_calls_and_logs_off() {
        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            let logon = cm.read().await;
            assert_eq!(logon.emsg, EMsg::ClientLogon);
            assert_eq!(logon.header.steamid(), 0x01A0_0000_0000_0000);
            assert!(!logon.body::<CMsgClientLogon>().has_access_token());
            cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;

            let (call, _) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(EMsg::ServiceMethodCallFromClient)
                .await;
            assert_eq!(call.header.steamid(), ANONYMOUS_STEAM_ID);
            let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
            response.set_publickey_exp("010001".to_string());
            cm.reply(&call, EResult::OK, response).await;

            assert_eq!(cm.read().await.emsg, EMsg::ClientLogOff);
            cm.send(logged_off()).await;
        });

        let mut client = client.login_anonymous().await.unwrap();
        assert_eq!(client.steam_id().to_steam64(), ANONYMOUS_STEAM_ID);
        assert_eq!(client.session_id(), -139116189);
        assert_eq!(client.heartbeat_interval(), Duration::from_secs(9));
        assert_eq!(client.cell_id(), 25);
        assert_eq!(client.public_ip(), Some(Ipv4Addr::new(203, 0, 113, 7)));

        let response = client
            .call_service_method(CAuthentication_GetPasswordRSAPublicKey_Request::new())
            .await
            .unwrap();
        assert_eq!(response.publickey_exp(), "010001");
        client.log_off().await.unwrap();
        quickly(server).await.unwrap();
    }

    #[tokio::test]
    async fn spawns_calls_and_logs_off() {
        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            assert_eq!(cm.read().await.emsg, EMsg::ClientLogon);
            cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;
            let (call, _) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(EMsg::ServiceMethodCallFromClient)
                .await;
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            )
            .await;
            assert_eq!(cm.read().await.emsg, EMsg::ClientLogOff);
            cm.send(logged_off()).await;
        });

        let (handle, mut events, task) = client.login_anonymous().await.unwrap().spawn();
        assert_eq!(handle.steam_id().to_steam64(), ANONYMOUS_STEAM_ID);
        handle
            .call_service_method(CAuthentication_GetPasswordRSAPublicKey_Request::new())
            .await
            .unwrap();
        handle.log_off().await.unwrap();
        assert!(matches!(
            quickly(events.recv()).await,
            Some(Event::LoggedOff(EResult::OK))
        ));
        assert!(quickly(events.recv()).await.is_none());
        assert_eq!(quickly(task).await, handle::Stopped::LoggedOff);
        quickly(server).await.unwrap();
    }

    #[tokio::test]
    async fn reads_events_without_a_task() {
        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            assert_eq!(cm.read().await.emsg, EMsg::ClientLogon);
            cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;
            cm.send(proto_message(
                EMsg::ClientHello,
                CMsgProtoBufHeader::new(),
                CMsgClientHello::new(),
            ))
            .await;
            // logged off once the client is surely logged on
            let (call, _) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(EMsg::ServiceMethodCallFromClient)
                .await;
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            )
            .await;
            cm.send(logged_off()).await;
        });
        let mut client = client.login_anonymous().await.unwrap();
        assert!(
            matches!(quickly(client.next_event()).await, Ok(Event::Message(message)) if message.emsg() == EMsg::ClientHello)
        );
        client
            .call_service_method(CAuthentication_GetPasswordRSAPublicKey_Request::new())
            .await
            .unwrap();
        assert!(matches!(
            quickly(client.next_event()).await,
            Ok(Event::LoggedOff(EResult::OK))
        ));
        assert!(matches!(quickly(client.next_event()).await, Err(ClientError::Dropped)));
        quickly(server).await.unwrap();
    }

    /// Answers an account's `ClientLogon` with a session.
    async fn logs_on_the_account<S>(cm: &mut FakeCm<S>, refresh_token: &str)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let logon = cm.read().await;
        assert_eq!(logon.emsg, EMsg::ClientLogon);
        assert_eq!(logon.header.steamid(), 0x0110_0001_0000_0000);
        let body = logon.body::<CMsgClientLogon>();
        assert_eq!(body.access_token(), refresh_token);
        assert_eq!(body.chat_mode(), 2);

        let mut header = CMsgProtoBufHeader::new();
        header.set_steamid(STEAM_ID);
        header.set_client_sessionid(5);
        let mut response = CMsgClientLogonResponse::new();
        response.set_eresult(EResult::OK as i32);
        response.set_heartbeat_seconds(9);
        cm.send(proto_message(EMsg::ClientLogOnResponse, header, response))
            .await;
    }

    #[tokio::test]
    async fn logs_on_with_a_refresh_token_and_renews_tokens() {
        logs::capture();
        let (client, mut cm) = connected().await;
        let renewed = jwt(&format!(r#"{{"sub":"{}","exp":1900000000}}"#, STEAM_ID));
        let access_token = jwt(&format!(r#"{{"sub":"{}","exp":1700086400}}"#, STEAM_ID));
        let (server_renewed, server_access_token) = (renewed.clone(), access_token.clone());
        let server = tokio::spawn(async move {
            logs_on_the_account(&mut cm, &refresh_token_jwt()).await;

            // the token sent, and the one Steam replaces it with
            for (renewal, sent, refresh_token) in [
                (ETokenRenewalType::k_ETokenRenewalType_None, refresh_token_jwt(), None),
                (
                    ETokenRenewalType::k_ETokenRenewalType_Allow,
                    refresh_token_jwt(),
                    Some(server_renewed.clone()),
                ),
                (ETokenRenewalType::k_ETokenRenewalType_Allow, server_renewed, None),
            ] {
                let (call, request) = cm
                    .expect::<CAuthentication_AccessToken_GenerateForApp_Request>(EMsg::ServiceMethodCallFromClient)
                    .await;
                assert_eq!(call.header.steamid(), STEAM_ID);
                assert_eq!(call.header.client_sessionid(), 5);
                assert_eq!(request.refresh_token(), sent);
                assert_eq!(request.steamid(), STEAM_ID);
                assert_eq!(request.renewal_type(), renewal);

                let mut response = CAuthentication_AccessToken_GenerateForApp_Response::new();
                response.set_access_token(server_access_token.clone());
                if let Some(refresh_token) = refresh_token {
                    response.set_refresh_token(refresh_token);
                }
                cm.reply(&call, EResult::OK, response).await;
            }
        });

        let refresh_token = RefreshToken::new(refresh_token_jwt()).unwrap();
        let mut client = client.login_with_refresh_token("account", refresh_token).await.unwrap();
        assert_eq!(client.steam_id().to_steam64(), STEAM_ID);
        assert_eq!(client.account_name(), "account");

        let token = client.web_access_token().await.unwrap();
        assert_eq!(token.expose_secret(), access_token);
        assert_eq!(client.refresh_token().expose_secret(), refresh_token_jwt());

        let renewed_tokens = client.renew_tokens().await.unwrap();
        assert_eq!(renewed_tokens.refresh_token.unwrap().expose_secret(), renewed);
        assert_eq!(client.refresh_token().expose_secret(), renewed);

        // kept when Steam doesn't replace it
        assert!(client.renew_tokens().await.unwrap().refresh_token.is_none());
        assert_eq!(client.refresh_token().expose_secret(), renewed);
        quickly(server).await.unwrap();
        logs::assert_none_contains(&[&refresh_token_jwt(), &renewed, &access_token]);
    }

    #[tokio::test]
    async fn sets_the_persona_state() {
        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            logs_on_the_account(&mut cm, &refresh_token_jwt()).await;
            for state in [EPersonaState::Online, EPersonaState::Invisible] {
                let status = cm.read().await;
                assert_eq!(status.emsg, EMsg::ClientChangeStatus);
                assert_eq!(status.header.steamid(), STEAM_ID);
                assert_eq!(status.header.client_sessionid(), 5);
                let body = status.body::<CMsgClientChangeStatus>();
                assert_eq!(body.persona_state(), state as i32 as u32);
                assert!(!body.has_player_name());
            }
        });

        let refresh_token = RefreshToken::new(refresh_token_jwt()).unwrap();
        let mut client = client.login_with_refresh_token("account", refresh_token).await.unwrap();
        client.set_persona_state(EPersonaState::Online).await.unwrap();
        client.set_persona_state(EPersonaState::Invisible).await.unwrap();
        quickly(server).await.unwrap();
        // kept, for a spawned client to set again
        assert_eq!(client.state.account.persona_state, Some(EPersonaState::Invisible));
    }

    #[tokio::test]
    async fn plays_a_game_and_talks_to_its_game_coordinator_without_a_task() {
        use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
        use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgGCClient;

        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            logs_on_the_account(&mut cm, &refresh_token_jwt()).await;
            let games = cm.read().await;
            assert_eq!(games.emsg, EMsg::ClientGamesPlayedWithDataBlob);
            assert_eq!(games.header.client_sessionid(), 5);
            let ids: Vec<_> = games
                .body::<CMsgClientGamesPlayed>()
                .games_played
                .iter()
                .map(|game| game.game_id())
                .collect();
            assert_eq!(ids, [730]);

            let sent = cm.read().await;
            assert_eq!(sent.emsg, EMsg::ClientToGC);
            assert_eq!(sent.header.routing_appid(), 730);
            assert_eq!(sent.header.steamid(), STEAM_ID);
            let envelope = sent.body::<CMsgGCClient>();
            assert_eq!((envelope.appid(), envelope.msgtype()), (730, 4006 | 0x8000_0000));
            // the GC answers
            let payload = gc::tests::proto_payload(4004, &CMsgProtoBufHeader::new(), b"");
            cm.send(gc::tests::from_gc(730, 4004 | 0x8000_0000, payload)).await;
            cm
        });

        let refresh_token = RefreshToken::new(refresh_token_jwt()).unwrap();
        let mut client = client.login_with_refresh_token("account", refresh_token).await.unwrap();
        client.set_games_played(&[730]).await.unwrap();
        client.send_to_gc(730, 4006, &CMsgClientHello::new()).await.unwrap();
        let _cm = server.await.unwrap();
        let Event::Message(message) = client.next_event().await.unwrap() else {
            panic!("expected a message");
        };
        let message = message.gc_message().unwrap().unwrap();
        assert_eq!((message.app_id(), message.msg_type()), (730, 4004));
        // kept, for a spawned client to set again
        assert_eq!(client.state.account.games_played, [730]);
    }

    #[test]
    fn the_persona_state_is_steams_value() {
        assert_eq!(change_status(EPersonaState::Offline).persona_state(), 0);
        assert_eq!(change_status(EPersonaState::Online).persona_state(), 1);
        assert_eq!(change_status(EPersonaState::Invisible).persona_state(), 7);
    }

    #[tokio::test]
    async fn logs_on_with_a_password() {
        logs::capture();
        let (client, mut cm) = connected().await;
        let key = crate::auth::tests::key();
        let server = tokio::spawn(async move {
            begins(
                &mut cm,
                &key,
                vec![allowed(EAuthSessionGuardType::k_EAuthSessionGuardType_None, "")],
            )
            .await;
            confirms(&mut cm).await;
            logs_on_the_account(&mut cm, &refresh_token_jwt()).await;
        });

        let client = client.login("account", "hunter2", SteamGuard::NoCode).await.unwrap();
        // as Steam spells it
        assert_eq!(client.account_name(), "Account");
        assert!(!format!("{:?}", client).contains(&refresh_token_jwt()));
        quickly(server).await.unwrap();
        logs::assert_none_contains(&["hunter2", &refresh_token_jwt()]);
    }

    #[tokio::test]
    async fn refused_logons_are_errors() {
        let (client, mut cm) = connected().await;
        let server = tokio::spawn(async move {
            cm.read().await;
            let mut response = CMsgClientLogonResponse::new();
            response.set_eresult(EResult::ServiceUnavailable as i32);
            cm.send(proto_message(
                EMsg::ClientLogOnResponse,
                CMsgProtoBufHeader::new(),
                response,
            ))
            .await;
        });

        assert!(matches!(
            client.login_anonymous().await,
            Err(ClientError::LogonFailed {
                result: EResult::ServiceUnavailable,
                extended: None,
            })
        ));
        quickly(server).await.unwrap();
    }
}

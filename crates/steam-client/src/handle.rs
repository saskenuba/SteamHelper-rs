//! A spawned client: a task drives the connection, and handles call through it.
//!
//! [`SteamClient::spawn`](crate::client::SteamClient::spawn) moves a logged on client into a task. The task owns the
//! connection: it sends the heartbeats, reads what Steam sends, answers each call when its response comes, and
//! forwards the rest as [`Events`]. A [`SteamHandle`] clones, and its calls run concurrently.
//!
//! When the connection is lost, the task connects and logs on again, with the refresh token of a [`User`] (the latest
//! one, if [`SteamHandle::renew_tokens`] replaced it) or anonymously, to another CM server than the one it lost. It
//! waits 1 s before the first attempt, then twice as long after each failure, up to 60 s, as node-steam-user does; the
//! delay starts over only once a session lasted a minute, so a server that takes the logon and drops it at once isn't
//! retried every second. The events say so: [`Event::Disconnected`], with the reason, then [`Event::Reconnected`].
//! Calls waiting when the connection is lost, and calls made until it is back, fail with [`ClientError::Dropped`].
//! Once logged on again, a [`User`] sets the persona state it was last set to, if any
//! ([`SteamHandle::set_persona_state`]), then the games it was last set to play ([`SteamHandle::set_games_played`]),
//! before anything else. Each [`GcChannel`] gets a [`GcEvent::SessionLost`] when the connection is lost, and keeps
//! its app's messages after the reconnect.
//!
//! Steam logging the client off, or refusing a logon, only ends it for good when retrying can't help. Retried, as
//! node-steam-user (`nonFatalLogOffResults`, and its logon retries): a `ClientLoggedOff` with `Invalid`, `Fail`,
//! `NoConnection`, `ServiceUnavailable` or `TryAnotherCM`, and a logon refused with `Fail`, `ServiceUnavailable` or
//! `TryAnotherCM`. Anything else, such as `LogonSessionReplaced` (the account logged on elsewhere) or an expired or
//! revoked token (`AccessDenied`, `Expired`, `InvalidPassword`), ends the task: the last event is
//! [`Event::LoggedOff`] with the result, the events end, and calls fail with [`ClientError::Closed`]. So
//! `Event::LoggedOff` always means the client stopped.
//!
//! A connection Steam drops silently, without closing it, fails once three calls in a row time out
//! ([`DisconnectReason::Unresponsive`]); otherwise only when TCP gives up on it, which takes minutes.
//!
//! The task ends when [`SteamHandle::log_off`] is called, when Steam ends the session for good, or once every
//! [`SteamHandle`] was dropped: then it logs off, waiting at most 5 s for Steam to confirm. Dropping the [`Events`]
//! doesn't stop it (the events are then dropped), so a reader can loop until they end. The [`SteamTask`] says when it
//! ended, and why.

use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::net::Ipv4Addr;
use std::panic;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EPersonaState;
use steam_language_gen::generated::enums::EResult;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::ETokenRenewalType;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgGCClient;
use steamid_parser::SteamID;
use thiserror::Error;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio::time::Instant;

use crate::auth::AccessToken;
use crate::auth::RefreshToken;
use crate::client::access_token_request;
use crate::client::change_status;
use crate::client::games_played;
use crate::client::hello;
use crate::client::renewed_tokens;
use crate::client::Account;
use crate::client::RenewedTokens;
use crate::client::User;
use crate::connection::Connection;
use crate::connection::RESPONSE_TIMEOUT;
use crate::errors::ClientError;
use crate::events::DisconnectReason;
use crate::events::Event;
use crate::events::RawMessage;
use crate::gc;
use crate::gc::GcChannel;
#[cfg(doc)]
use crate::gc::GcEvent;
use crate::gc::GcRoutes;
use crate::gc::Route;
use crate::job::Job;
use crate::job::JobMessage;
use crate::job::JobRequest;
use crate::job::JobResponseOf;
use crate::protocol::free_if_big;
use crate::protocol::CmProtocol;
use crate::protocol::Ended;
use crate::protocol::Exchange;
use crate::protocol::LogonInfo;
use crate::protocol::EVENT_QUEUE_CAPACITY;
use crate::service;
use crate::service::methods::GenerateAccessTokenForApp;
use crate::service::ResponseOf;
use crate::service::Service;
use crate::service::ServiceMethod;
use crate::service::ServiceRequest;

/// How many messages from earlier reads wait to be read; past this, the oldest are dropped, and an [`Event::Lagged`]
/// in their place counts them. The messages of the latest read, such as those of a big `Multi`, are all kept. A
/// message takes 48 bytes, plus the read buffer it is a slice of.
pub const EVENT_CAPACITY: usize = EVENT_QUEUE_CAPACITY;
/// How long the task waits before connecting again the first time; it doubles after each failure.
const FIRST_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);
/// A session that lasted this long worked: the delay before connecting again starts over after it.
const STABLE_SESSION: Duration = Duration::from_secs(60);
/// How long connecting again may take: the CM list, the connection and the channel encryption.
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the task waits for Steam to confirm the log off, once every handle was dropped.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Past this many bytes waiting to be written, calls wait in the task, unsent, until the connection takes them.
const MAX_UNSENT: usize = 64 * 1024;
/// After this many calls in a row time out, the connection is taken for lost.
const TIMEOUTS_BEFORE_RECONNECT: u32 = 3;
/// Past this many `Disconnected` and `Reconnected` waiting to be read, those around sessions whose messages were all
/// dropped are merged, so that events nobody reads take bounded memory.
const MAX_LIFECYCLE_EVENTS: usize = 64;

/// Calls through a [spawned](crate::client::SteamClient::spawn) client. Clones are cheap, and call through the same
/// connection, concurrently. The client stops once every handle is dropped.
pub struct SteamHandle<A> {
    commands: mpsc::UnboundedSender<Command>,
    shared: Arc<Shared<A>>,
}

/// What a spawned client's Steam sends that no call waits for, in order. See [`Event`].
///
/// The task never waits for the reader: messages that aren't read in time go, the oldest first (see
/// [`EVENT_CAPACITY`]), and an [`Event::Lagged`] takes their place. Calls are answered anyway, and the other events
/// (`Disconnected`, `Reconnected`, `LoggedOff`) are always kept. They end once the client stopped.
pub struct Events(Arc<EventQueue>);

impl Events {
    /// The next event, or `None` once the client stopped and every event was read. Cancel-safe: it can wait in a
    /// `select!`.
    pub async fn recv(&mut self) -> Option<Event> {
        loop {
            {
                let mut queue = self.0.lock();
                if let Some(event) = queue.pop() {
                    return Some(event);
                }
                if queue.stopped {
                    return None;
                }
            }
            self.0.ready.notified().await;
        }
    }

    /// The next event if there is one now; [`TryRecvError::Stopped`] once the client stopped and every event was
    /// read.
    pub fn try_recv(&mut self) -> Result<Event, TryRecvError> {
        let mut queue = self.0.lock();
        match queue.pop() {
            Some(event) => Ok(event),
            None if queue.stopped => Err(TryRecvError::Stopped),
            None => Err(TryRecvError::Empty),
        }
    }
}

impl Drop for Events {
    fn drop(&mut self) {
        let mut queue = self.0.lock();
        queue.reader_gone = true;
        queue.clear();
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Events")
            .field("waiting", &self.0.lock().events.len())
            .finish()
    }
}

/// Why [`Events::try_recv`] has no event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TryRecvError {
    /// None waits now.
    #[error("no event waits")]
    Empty,
    /// The client stopped, and every event was read.
    #[error("the client stopped")]
    Stopped,
}

/// The task of a spawned client: awaiting it waits until the client stopped, and says why.
///
/// Dropping it doesn't stop the client. To stop it, call [`SteamHandle::log_off`], or drop every handle, which logs
/// off too (waiting at most 5 s for Steam); await the task then, so that the program doesn't exit before.
#[must_use = "the client logs off once its handles are dropped: await the task to let it"]
pub struct SteamTask(JoinHandle<Stopped>);

impl Future for SteamTask {
    type Output = Stopped;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Stopped> {
        Pin::new(&mut self.0).poll(cx).map(|ended| match ended {
            Ok(stopped) => stopped,
            Err(error) if error.is_panic() => panic::resume_unwind(error.into_panic()),
            Err(_) => Stopped::Cancelled,
        })
    }
}

impl fmt::Debug for SteamTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SteamTask")
            .field("finished", &self.0.is_finished())
            .finish()
    }
}

/// Why a spawned client stopped: what its [`SteamTask`] gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stopped {
    /// [`SteamHandle::log_off`] logged off.
    LoggedOff,
    /// Every [`SteamHandle`] was dropped, and the client logged off.
    HandlesDropped,
    /// Steam logged the client off for good, such as with `LogonSessionReplaced`.
    Ended(EResult),
    /// Steam refused to log the client on again, such as with `AccessDenied` for a revoked token.
    LogonRefused(EResult),
    /// The runtime shut down first.
    Cancelled,
}

/// What the task and the handles share: what the latest logon gave, and the account it logs on again as.
struct Shared<A> {
    state: Mutex<State<A>>,
}

struct State<A> {
    logon: LogonInfo,
    endpoint: String,
    account: A,
}

impl<A> Shared<A> {
    /// Only held to read or write it, never across an await.
    fn state(&self) -> MutexGuard<'_, State<A>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<A> Clone for SteamHandle<A> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            shared: self.shared.clone(),
        }
    }
}

impl<A> fmt::Debug for SteamHandle<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.shared.state();
        f.debug_struct("SteamHandle")
            .field("steam_id", &state.logon.session.steam_id)
            .field("endpoint", &state.endpoint)
            .finish_non_exhaustive()
    }
}

impl<A> SteamHandle<A> {
    /// Calls service method `M`, in the session, and waits for its response, at most 30 s. Calls from every handle
    /// run concurrently. See [`ServiceMethod`] to call a method the crate doesn't know.
    ///
    /// Dropping the future forgets the call: a response that still comes is an [`Event::Message`].
    pub async fn call<M: ServiceMethod>(&self, request: M::Request) -> Result<M::Response, ClientError> {
        self.exchange::<Service<M>>(request).await
    }

    /// Calls the method of `request`, one of [`service::methods`], as [`SteamHandle::call`] does.
    pub async fn call_service_method<R: ServiceRequest>(&self, request: R) -> Result<ResponseOf<R>, ClientError> {
        self.call::<R::Method>(request).await
    }

    /// Sends job message `J`, in the session, and waits for the message answering it, as
    /// [`SteamClient::call_job`](crate::client::SteamClient::call_job) does: at most 30 s, concurrently with the
    /// other calls. See [`JobMessage`] to send a message the crate doesn't know.
    ///
    /// Dropping the future forgets the call: an answer that still comes is an [`Event::Message`].
    pub async fn call_job<J: JobMessage>(&self, request: J::Request) -> Result<J::Response, ClientError> {
        self.exchange::<Job<J>>(request).await
    }

    /// Sends the job message of `request`, one of [`job::messages`](crate::job::messages), as [`SteamHandle::call_job`]
    /// does.
    pub async fn call_job_message<R: JobRequest>(&self, request: R) -> Result<JobResponseOf<R>, ClientError> {
        self.call_job::<R::Job>(request).await
    }

    async fn exchange<E: Exchange>(&self, request: E::Request) -> Result<E::Response, ClientError> {
        service::check(&request)?;
        let (sender, receiver) = oneshot::channel();
        self.command(Command::Call(Box::new(TypedCall::<E> {
            request,
            sender,
            deadline: Instant::now() + RESPONSE_TIMEOUT,
        })))?;
        receiver.await.map_err(|_| ClientError::Closed)?
    }

    /// Logs off, waits for Steam to confirm (at most 30 s), and stops the client. Calls from other handles fail with
    /// [`ClientError::Closed`] from then on.
    pub async fn log_off(self) -> Result<(), ClientError> {
        let (sender, receiver) = oneshot::channel();
        self.command(Command::LogOff(sender))?;
        receiver.await.map_err(|_| ClientError::Closed)?
    }

    /// Our SteamID, which Steam assigned on logon.
    pub fn steam_id(&self) -> SteamID {
        self.shared.state().logon.session.steam_id
    }

    /// The session id Steam assigned on the latest logon.
    pub fn session_id(&self) -> i32 {
        self.shared.state().logon.session.session_id
    }

    /// How often the client sends a heartbeat.
    pub fn heartbeat_interval(&self) -> Duration {
        self.shared.state().logon.heartbeat_interval
    }

    /// The cell (region) Steam placed us in.
    pub fn cell_id(&self) -> u32 {
        self.shared.state().logon.cell_id
    }

    /// Our public IP, as Steam sees it.
    pub fn public_ip(&self) -> Option<Ipv4Addr> {
        self.shared.state().logon.public_ip
    }

    /// The CM server, as `ip:port`. Another one after reconnecting.
    pub fn endpoint(&self) -> String {
        self.shared.state().endpoint.clone()
    }

    fn command(&self, command: Command) -> Result<(), ClientError> {
        self.commands.send(command).map_err(|_| ClientError::Closed)
    }
}

impl SteamHandle<User> {
    /// The account's name.
    pub fn account_name(&self) -> String {
        self.shared.state().account.account_name().to_string()
    }

    /// The refresh token the client logged on with, or the one that replaced it in [`SteamHandle::renew_tokens`].
    pub fn refresh_token(&self) -> RefreshToken {
        self.shared.state().account.refresh_token().clone()
    }

    /// Generates an access token for the Steam websites and Web API, as
    /// [`SteamClient::web_access_token`](crate::client::SteamClient::web_access_token) does.
    pub async fn web_access_token(&self) -> Result<AccessToken, ClientError> {
        let request = {
            let state = self.shared.state();
            token_request(&state, ETokenRenewalType::k_ETokenRenewalType_None)
        };
        let response = self.call::<GenerateAccessTokenForApp>(request).await?;
        AccessToken::new(response.access_token().to_string())
    }

    /// Generates an access token, and lets Steam replace the refresh token, as
    /// [`SteamClient::renew_tokens`](crate::client::SteamClient::renew_tokens) does. Save a new refresh token: Steam
    /// revoked the old one.
    ///
    /// The client keeps the new token, and logs on again with it, as soon as Steam's response comes: even if this
    /// future was dropped, or timed out, before. [`SteamHandle::refresh_token`] has it then.
    pub async fn renew_tokens(&self) -> Result<RenewedTokens, ClientError> {
        let (sender, receiver) = oneshot::channel();
        self.command(Command::Call(Box::new(RenewCall {
            shared: self.shared.clone(),
            sender,
            deadline: Instant::now() + RESPONSE_TIMEOUT,
        })))?;
        receiver.await.map_err(|_| ClientError::Closed)?
    }

    /// Sets the account's persona state, as
    /// [`SteamClient::set_persona_state`](crate::client::SteamClient::set_persona_state) does, and sets it again each
    /// time the client logs on again. The task sends it: this doesn't wait. While the client is connecting again,
    /// it is set once logged on.
    pub fn set_persona_state(&self, state: EPersonaState) -> Result<(), ClientError> {
        // under the lock, so the task sends the states in the order they were kept
        let mut shared = self.shared.state();
        shared.account.set_persona_state(state);
        self.command(Command::SetPersonaState(state))
    }

    /// Sets the apps the account plays, as
    /// [`SteamClient::set_games_played`](crate::client::SteamClient::set_games_played) does, and sets them again each
    /// time the client logs on again. The task sends it: this doesn't wait. While the client is connecting again,
    /// they are set once logged on.
    pub fn set_games_played(&self, app_ids: &[u32]) -> Result<(), ClientError> {
        // under the lock, so the task sends the apps in the order they were kept
        let mut shared = self.shared.state();
        shared.account.set_games_played(app_ids);
        self.command(Command::SetGamesPlayed(app_ids.into()))
    }

    /// Opens a channel to the game coordinator of app `app_id`: its messages go there instead of the [`Events`],
    /// until the channel is dropped. The account must play the app ([`SteamHandle::set_games_played`]) for the
    /// coordinator to talk to it. See [`crate::gc`].
    ///
    /// The latest channel of an app takes its messages: an earlier one ends. Channels last across reconnects, each
    /// getting a [`GcEvent::SessionLost`] when the connection is lost.
    pub fn game_coordinator(&self, app_id: u32) -> Result<GcChannel, ClientError> {
        let (channel, route) = gc::channel(app_id, self.commands.downgrade());
        self.command(Command::RouteGc(route))?;
        Ok(channel)
    }
}

/// `GenerateAccessTokenForApp` with the account's latest refresh token.
fn token_request(
    state: &State<User>,
    renewal: ETokenRenewalType,
) -> CAuthentication_AccessToken_GenerateForApp_Request {
    access_token_request(state.account.refresh_token(), state.logon.session.steam_id, renewal)
}

/// Connects to a CM server other than the one given, if it can, and encrypts the channel, as the task does to
/// reconnect. Returns the connection and the server, as `ip:port`.
pub(crate) type Connector<T> = Arc<
    dyn Fn(Option<String>) -> Pin<Box<dyn Future<Output = Result<(Connection<T>, String), ClientError>> + Send>>
        + Send
        + Sync,
>;

pub(crate) fn connector<T, F>(connect: F) -> Connector<T>
where
    F: Fn(Option<String>) -> Pin<Box<dyn Future<Output = Result<(Connection<T>, String), ClientError>> + Send>>
        + Send
        + Sync
        + 'static,
{
    Arc::new(connect)
}

/// Starts the task of a logged on connection. Returns its handle, its events, and the task.
pub(crate) fn spawn<T, A>(
    connection: Connection<T>,
    endpoint: String,
    logon: LogonInfo,
    account: A,
    connect: Connector<T>,
) -> (SteamHandle<A>, Events, SteamTask)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    A: Account,
{
    let (commands, command_receiver) = mpsc::unbounded_channel();
    let events = Arc::new(EventQueue::default());
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            logon,
            endpoint,
            account,
        }),
    });

    let driver = Driver {
        link: Link::Connected(connection),
        shared: shared.clone(),
        commands: command_receiver,
        events: EventSender(events.clone()),
        calls: Calls::default(),
        routes: GcRoutes::default(),
        connect,
        attempt: 0,
        session_start: Instant::now(),
        avoid: None,
    };
    let task = tokio::spawn(driver.run());
    (SteamHandle { commands, shared }, Events(events), SteamTask(task))
}

/// What a handle, or a [`GcChannel`], asks of the task.
pub(crate) enum Command {
    Call(Box<dyn StartCall>),
    SetPersonaState(EPersonaState),
    SetGamesPlayed(Box<[u32]>),
    /// Sends a game coordinator's messages to a channel.
    RouteGc(Route),
    /// Sends a message to a game coordinator.
    ToGc(Box<CMsgGCClient>),
    LogOff(oneshot::Sender<Result<(), ClientError>>),
}

/// A call to make, its types erased.
pub(crate) trait StartCall: Send {
    /// When the caller stops waiting.
    fn deadline(&self) -> Instant;
    /// Whether the caller is gone: the call isn't made.
    fn is_closed(&self) -> bool;
    /// The message that answers it.
    fn response(&self) -> EMsg;
    fn start(self: Box<Self>, protocol: &mut CmProtocol) -> PendingCall;
    fn fail(self: Box<Self>, error: ClientError);
}

struct TypedCall<E: Exchange> {
    request: E::Request,
    sender: oneshot::Sender<Result<E::Response, ClientError>>,
    deadline: Instant,
}

impl<E: Exchange> StartCall for TypedCall<E> {
    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    fn response(&self) -> EMsg {
        E::RESPONSE
    }

    fn start(self: Box<Self>, protocol: &mut CmProtocol) -> PendingCall {
        let call = protocol.exchange::<E>(self.request);
        PendingCall {
            job_id: call.job_id(),
            response: E::RESPONSE,
            deadline: Some(self.deadline),
            responder: Box::new(Responder::<E>(self.sender)),
        }
    }

    fn fail(self: Box<Self>, error: ClientError) {
        let _ = self.sender.send(Err(error));
    }
}

/// `renew_tokens`: its request has the token as it is when the call is made, and the token Steam replaces it with is
/// kept as soon as the response comes, whether or not the caller still waits.
struct RenewCall {
    shared: Arc<Shared<User>>,
    sender: oneshot::Sender<Result<RenewedTokens, ClientError>>,
    deadline: Instant,
}

impl StartCall for RenewCall {
    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    fn response(&self) -> EMsg {
        EMsg::ServiceMethodResponse
    }

    fn start(self: Box<Self>, protocol: &mut CmProtocol) -> PendingCall {
        let request = token_request(&self.shared.state(), ETokenRenewalType::k_ETokenRenewalType_Allow);
        let call = protocol.call::<GenerateAccessTokenForApp>(request);
        PendingCall {
            job_id: call.job_id(),
            response: EMsg::ServiceMethodResponse,
            deadline: Some(self.deadline),
            responder: Box::new(RenewResponder {
                shared: self.shared,
                sender: Some(self.sender),
            }),
        }
    }

    fn fail(self: Box<Self>, error: ClientError) {
        let _ = self.sender.send(Err(error));
    }
}

/// A call the task made, until its response comes.
pub(crate) struct PendingCall {
    job_id: u64,
    /// The message that answers it.
    response: EMsg,
    /// None once nobody waits for it, but its response still matters.
    deadline: Option<Instant>,
    responder: Box<dyn Respond>,
}

/// Answers a caller, its types erased.
trait Respond: Send {
    /// Sends the answer. Gives the response back if nobody took it.
    fn respond(self: Box<Self>, answer: Result<RawMessage, ClientError>) -> Option<RawMessage>;

    /// Whether nothing needs the answer anymore, so the call can be forgotten.
    fn is_closed(&self) -> bool;

    /// Tells the caller the call timed out waiting for `response`. Returns what still waits for the response, if it
    /// matters when it comes late.
    fn time_out(self: Box<Self>, response: EMsg) -> Option<Box<dyn Respond>> {
        self.respond(Err(ClientError::Timeout(response)));
        None
    }
}

struct Responder<E: Exchange>(oneshot::Sender<Result<E::Response, ClientError>>);

impl<E: Exchange> Respond for Responder<E> {
    fn respond(self: Box<Self>, answer: Result<RawMessage, ClientError>) -> Option<RawMessage> {
        if self.0.is_closed() {
            return answer.ok();
        }
        let response = answer.and_then(|message| message.parse().map_err(ClientError::from).and_then(E::response));
        let _ = self.0.send(response);
        None
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

struct RenewResponder {
    shared: Arc<Shared<User>>,
    /// None once the caller was told the call timed out.
    sender: Option<oneshot::Sender<Result<RenewedTokens, ClientError>>>,
}

impl Respond for RenewResponder {
    fn respond(self: Box<Self>, answer: Result<RawMessage, ClientError>) -> Option<RawMessage> {
        let renewed = answer
            .and_then(|message| message.parse().map_err(ClientError::from))
            .and_then(service::response::<GenerateAccessTokenForApp>)
            .and_then(renewed_tokens);
        if let Ok(RenewedTokens {
            refresh_token: Some(refresh_token),
            ..
        }) = &renewed
        {
            // Steam revoked the old one: kept even if nobody waits anymore
            self.shared.state().account.set_refresh_token(refresh_token.clone());
            info!("Steam replaced the refresh token");
        }
        if let Some(sender) = self.sender {
            let _ = sender.send(renewed);
        }
        None
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn time_out(mut self: Box<Self>, response: EMsg) -> Option<Box<dyn Respond>> {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(Err(ClientError::Timeout(response)));
        }
        Some(self)
    }
}

/// The events a reader hasn't read, shared with [`Events`]. The task only holds the lock to add to it, so it never
/// waits for the reader.
#[derive(Default)]
struct EventQueue {
    queue: Mutex<Queue>,
    /// Woken when events are added.
    ready: Notify,
}

#[derive(Default)]
struct Queue {
    events: VecDeque<Event>,
    /// How many of `events` are messages, which go when there are too many.
    messages: usize,
    /// How many are `Disconnected` or `Reconnected`.
    lifecycle: usize,
    /// Messages were dropped since the reader last read a `Lagged`: logged once.
    lagging: bool,
    /// The task stopped: nothing more comes.
    stopped: bool,
    /// The [`Events`] were dropped: nothing is kept.
    reader_gone: bool,
}

impl EventQueue {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes the events the protocol kept, from one read: all of them, dropping the oldest messages from earlier
    /// reads past [`EVENT_CAPACITY`]. Game coordinators' messages go to their channel, if they have one.
    fn push_read(&self, protocol: &mut CmProtocol, routes: &mut GcRoutes) {
        let mut queue = self.lock();
        let mut read = 0;
        while let Some(event) = protocol.poll_event() {
            let event = match event {
                Event::Message(message) => match routes.route(message) {
                    Some(message) => Event::Message(message),
                    None => continue,
                },
                event => event,
            };
            if queue.reader_gone {
                continue;
            }
            match event {
                Event::Message(_) => {
                    queue.push_message(event);
                    read += 1;
                }
                // what this read held past the protocol's own bound
                Event::Lagged(count) => queue.lag(count),
                // the task says how the session ended, once it knows whether it connects again; the protocol has no
                // other events
                _ => {}
            }
        }
        if read > 0 {
            queue.trim(read.max(EVENT_CAPACITY));
            drop(queue);
            self.ready.notify_one();
        }
    }

    /// Keeps a message that came outside a read: a response nobody waits for anymore.
    fn push_message(&self, message: RawMessage) {
        let mut queue = self.lock();
        if queue.reader_gone {
            return;
        }
        queue.push_message(Event::Message(message));
        queue.trim(EVENT_CAPACITY);
        drop(queue);
        self.ready.notify_one();
    }

    /// Keeps `Disconnected`, `Reconnected` or the last `LoggedOff`: never dropped.
    fn push_lifecycle(&self, event: Event) {
        let mut queue = self.lock();
        if queue.reader_gone {
            return;
        }
        if let Event::Disconnected { .. } | Event::Reconnected = event {
            if queue.lifecycle >= MAX_LIFECYCLE_EVENTS {
                queue.merge_sessions();
            }
            queue.lifecycle += 1;
        }
        queue.events.push_back(event);
        drop(queue);
        self.ready.notify_one();
    }
}

/// The task's end of the events. Dropping it ends them, so the reader gets `None` once it read what is left, even if
/// the task panicked or the runtime dropped it.
struct EventSender(Arc<EventQueue>);

impl std::ops::Deref for EventSender {
    type Target = EventQueue;

    fn deref(&self) -> &EventQueue {
        &self.0
    }
}

impl Drop for EventSender {
    fn drop(&mut self) {
        self.lock().stopped = true;
        self.ready.notify_one();
    }
}

impl Queue {
    fn pop(&mut self) -> Option<Event> {
        let event = self.events.pop_front()?;
        match event {
            Event::Message(_) => self.messages -= 1,
            Event::Disconnected { .. } | Event::Reconnected => self.lifecycle -= 1,
            Event::Lagged(_) => self.lagging = false,
            _ => {}
        }
        free_if_big(&mut self.events);
        Some(event)
    }

    fn clear(&mut self) {
        self.events = VecDeque::new();
        self.messages = 0;
        self.lifecycle = 0;
    }

    fn push_message(&mut self, message: Event) {
        self.messages += 1;
        self.events.push_back(message);
    }

    /// Counts `count` messages dropped here, with the `Lagged` before, if any.
    fn lag(&mut self, count: u64) {
        match self.events.back_mut() {
            Some(Event::Lagged(lagged)) => *lagged += count,
            _ => self.events.push_back(Event::Lagged(count)),
        }
    }

    /// Drops the oldest messages past `capacity`, a `Lagged` in their place.
    fn trim(&mut self, capacity: usize) {
        if self.messages > capacity && !self.lagging {
            warn!("Events aren't read as fast as they come: dropping the oldest");
            self.lagging = true;
        }
        while self.messages > capacity {
            self.drop_oldest_message();
        }
    }

    fn drop_oldest_message(&mut self) {
        let Some(index) = self.events.iter().position(|event| matches!(event, Event::Message(_))) else {
            return;
        };
        self.messages -= 1;
        let mut count = 1;
        if let Some(&Event::Lagged(after)) = self.events.get(index + 1) {
            count += after;
            self.events.remove(index + 1);
        }
        match index.checked_sub(1).and_then(|before| self.events.get_mut(before)) {
            Some(Event::Lagged(before)) => {
                *before += count;
                self.events.remove(index);
            }
            _ => self.events[index] = Event::Lagged(count),
        }
    }

    /// Merges the sessions nothing is left of: a `Reconnected`, then nothing but dropped messages, then
    /// `Disconnected`, are taken out, so the reader sees one longer disconnection, and the messages dropped.
    fn merge_sessions(&mut self) {
        let mut index = 0;
        while index < self.events.len() {
            if !matches!(self.events[index], Event::Reconnected) {
                index += 1;
                continue;
            }
            let lagged = match self.events.get(index + 1) {
                Some(&Event::Lagged(count)) => Some(count),
                _ => None,
            };
            let next = index + 1 + usize::from(lagged.is_some());
            if !matches!(self.events.get(next), Some(Event::Disconnected { .. })) {
                index += 1;
                continue;
            }
            // `Reconnected`, `Lagged`, `Disconnected` go; the counts around them join into one `Lagged`
            self.events.drain(index..=next);
            self.lifecycle -= 2;
            let mut count = lagged.unwrap_or(0);
            if let Some(&Event::Lagged(after)) = self.events.get(index) {
                count += after;
                self.events.remove(index);
            }
            if count > 0 {
                match index.checked_sub(1).and_then(|before| self.events.get_mut(before)) {
                    Some(Event::Lagged(before)) => *before += count,
                    _ => self.events.insert(index, Event::Lagged(count)),
                }
            }
        }
    }
}

/// The calls of a task.
#[derive(Default)]
struct Calls {
    /// Made, until their response comes.
    pending: Vec<PendingCall>,
    /// Not made yet, while too much waits to be written: in order.
    parked: VecDeque<Box<dyn StartCall>>,
    /// How many calls in a row timed out.
    timeouts: u32,
}

impl Calls {
    /// Makes the calls waiting, while the connection takes them.
    fn start<T: AsyncRead + AsyncWrite + Unpin>(&mut self, connection: &mut Connection<T>) {
        while connection.unsent() < MAX_UNSENT {
            let Some(call) = self.parked.pop_front() else {
                break;
            };
            // nobody waits for it: not made
            if !call.is_closed() {
                self.pending.push(call.start(connection.protocol()));
            }
        }
    }

    /// Answers the calls whose response came, or forgets those whose caller is gone, and passes on the events.
    fn deliver(&mut self, protocol: &mut CmProtocol, events: &EventQueue, routes: &mut GcRoutes) {
        let mut index = 0;
        while index < self.pending.len() {
            let job_id = self.pending[index].job_id;
            if let Some(answer) = protocol.take_answer(job_id) {
                let call = self.pending.swap_remove(index);
                if answer.is_ok() {
                    self.timeouts = 0;
                }
                if let Some(late) = call.responder.respond(answer.map_err(call_error)) {
                    events.push_message(late);
                }
            } else if self.pending[index].responder.is_closed() {
                self.pending.swap_remove(index);
                protocol.forget(job_id);
            } else {
                index += 1;
            }
        }
        events.push_read(protocol, routes);
    }

    /// When the next call times out.
    fn next_deadline(&self) -> Option<Instant> {
        let pending = self.pending.iter().filter_map(|call| call.deadline);
        let parked = self.parked.iter().map(|call| call.deadline());
        pending.chain(parked).min()
    }

    /// Fails the calls whose deadline passed.
    fn time_out(&mut self, protocol: &mut CmProtocol) {
        let now = Instant::now();
        let mut index = 0;
        while index < self.pending.len() {
            if self.pending[index].deadline.is_none_or(|deadline| deadline > now) {
                index += 1;
                continue;
            }
            let call = self.pending.swap_remove(index);
            self.timeouts += 1;
            match call.responder.time_out(call.response) {
                // still waits for the response, with nobody waiting for it
                Some(responder) => self.pending.push(PendingCall {
                    job_id: call.job_id,
                    response: call.response,
                    deadline: None,
                    responder,
                }),
                None => protocol.forget(call.job_id),
            }
        }
        let mut index = 0;
        while index < self.parked.len() {
            if self.parked[index].deadline() > now {
                index += 1;
            } else if let Some(call) = self.parked.remove(index) {
                // never sent: the connection doesn't take what it's given
                self.timeouts += 1;
                let response = call.response();
                call.fail(ClientError::Timeout(response));
            }
        }
    }

    /// Fails every call, as the connection was lost.
    fn fail(&mut self) {
        for call in self.pending.drain(..) {
            call.responder.respond(Err(ClientError::Dropped));
        }
        for call in self.parked.drain(..) {
            call.fail(ClientError::Dropped);
        }
        self.timeouts = 0;
    }
}

/// How a call fails when the session ended: as `Dropped` if the client connects again.
fn call_error(ended: Ended) -> ClientError {
    match ended {
        Ended::LoggedOff(result) if retries_log_off(result) => ClientError::Dropped,
        ended => ended.into(),
    }
}

/// The task of a spawned client.
struct Driver<T, A> {
    link: Link<T>,
    shared: Arc<Shared<A>>,
    commands: mpsc::UnboundedReceiver<Command>,
    events: EventSender,
    calls: Calls,
    /// Where game coordinators' messages go.
    routes: GcRoutes,
    connect: Connector<T>,
    /// How many times in a row connecting again failed, or gave a session that didn't last: the delay before the next
    /// attempt doubles with each.
    attempt: u32,
    /// When the client logged on.
    session_start: Instant,
    /// The CM server that failed the client last, which the next connection avoids.
    avoid: Option<String>,
}

type Relog<T> = Pin<Box<dyn Future<Output = Result<Relogged<T>, RelogFailed>> + Send>>;

/// The connection, or the attempt to make a new one.
// the connection is the usual one, and moves only when it is replaced
#[allow(clippy::large_enum_variant)]
enum Link<T> {
    Connected(Connection<T>),
    Reconnecting(Relog<T>),
}

struct Relogged<T> {
    connection: Connection<T>,
    endpoint: String,
    logon: LogonInfo,
}

/// Why logging on again failed, and the server it was, once connected.
struct RelogFailed {
    error: ClientError,
    endpoint: Option<String>,
}

/// What the task does after a round.
enum Next<T> {
    Continue,
    /// The connection was lost, or Steam logged us off in a way worth retrying.
    Reconnect(DisconnectReason),
    Reconnected(Box<Relogged<T>>),
    /// The attempt to log on again failed in a way worth retrying, after connecting to that server, if it did.
    Retry(Option<String>),
    Stop(Stopped),
}

impl<T, A> Driver<T, A>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    A: Account,
{
    async fn run(mut self) -> Stopped {
        let stopped = loop {
            let next = match &mut self.link {
                Link::Connected(connection) => {
                    connected(
                        connection,
                        &mut self.commands,
                        &self.events,
                        &mut self.calls,
                        &mut self.routes,
                    )
                    .await
                }
                Link::Reconnecting(relog) => {
                    reconnecting(relog, &mut self.commands, &self.events, &mut self.routes).await
                }
            };
            match next {
                Next::Continue => {}
                Next::Reconnect(reason) => {
                    self.calls.fail();
                    self.events.push_lifecycle(Event::Disconnected { reason });
                    self.routes.session_lost();
                    if self.session_start.elapsed() >= STABLE_SESSION {
                        self.attempt = 0;
                    }
                    self.avoid = Some(self.shared.state().endpoint.clone());
                    // drops the connection
                    self.link = Link::Reconnecting(self.relog());
                }
                Next::Retry(endpoint) => {
                    if endpoint.is_some() {
                        self.avoid = endpoint;
                    }
                    self.link = Link::Reconnecting(self.relog());
                }
                Next::Reconnected(relogged) => self.reconnected(*relogged),
                Next::Stop(stopped) => break stopped,
            }
        };
        // the connection closes before the events end, and before the task does; callers get `Closed`
        let Self {
            link,
            calls,
            events,
            routes,
            ..
        } = self;
        drop((link, calls, routes, events));
        stopped
    }

    /// Waits, then connects and logs on again: the next attempt.
    fn relog(&mut self) -> Relog<T> {
        let delay = reconnect_delay(self.attempt);
        self.attempt = self.attempt.saturating_add(1);
        relog(self.connect.clone(), self.shared.clone(), delay, self.avoid.clone())
    }

    fn reconnected(&mut self, relogged: Relogged<T>) {
        let Relogged {
            mut connection,
            endpoint,
            logon,
        } = relogged;
        let (persona_state, games) = {
            let mut state = self.shared.state();
            state.logon = logon;
            state.endpoint = endpoint;
            (
                state.account.persona_state(),
                games_played(state.account.games_played()),
            )
        };
        info!("Logged on again as {}", logon.session.steam_id);
        // a new session shows as offline, and plays nothing, until set otherwise; what is set later is sent by its
        // command
        if let Some(persona_state) = persona_state {
            connection
                .protocol()
                .send(EMsg::ClientChangeStatus, change_status(persona_state));
        }
        if !games.games_played.is_empty() {
            connection.protocol().send(EMsg::ClientGamesPlayedWithDataBlob, games);
        }
        self.session_start = Instant::now();
        self.link = Link::Connected(connection);
        self.events.push_lifecycle(Event::Reconnected);
    }
}

/// A round of a connected task: answers what came, then waits for a command, the connection, or a call's deadline.
async fn connected<T>(
    connection: &mut Connection<T>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    events: &EventQueue,
    calls: &mut Calls,
    routes: &mut GcRoutes,
) -> Next<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    calls.deliver(connection.protocol(), events, routes);
    match connection.protocol().logged_off() {
        Some(Ok(result)) if retries_log_off(result) => {
            info!("Steam logged us off ({:?}): logging on again", result);
            return Next::Reconnect(DisconnectReason::LoggedOff(result));
        }
        Some(Ok(result)) => {
            warn!("Steam logged us off ({:?}): stopping", result);
            events.push_lifecycle(Event::LoggedOff(result));
            return Next::Stop(Stopped::Ended(result));
        }
        Some(Err(error)) => {
            info!("Lost the connection ({}): connecting again", error);
            return Next::Reconnect(DisconnectReason::ConnectionLost);
        }
        None => {}
    }
    if calls.timeouts >= TIMEOUTS_BEFORE_RECONNECT {
        warn!("{} calls in a row timed out: connecting again", calls.timeouts);
        return Next::Reconnect(DisconnectReason::Unresponsive);
    }
    calls.start(connection);

    let deadline = calls.next_deadline();
    tokio::select! {
        command = commands.recv() => match command {
            Some(Command::Call(call)) => calls.parked.push_back(call),
            Some(Command::SetPersonaState(state)) => {
                connection.protocol().send(EMsg::ClientChangeStatus, change_status(state));
            }
            Some(Command::SetGamesPlayed(app_ids)) => {
                connection.protocol().send(EMsg::ClientGamesPlayedWithDataBlob, games_played(&app_ids));
            }
            Some(Command::RouteGc(route)) => routes.add(route),
            Some(Command::ToGc(message)) => {
                connection.protocol().send_routed(EMsg::ClientToGC, message.appid(), *message);
            }
            Some(Command::LogOff(done)) => {
                let logged_off = connection.log_off().await;
                logged_off_for_good(connection.protocol(), calls, events, routes);
                let _ = done.send(logged_off);
                return Next::Stop(Stopped::LoggedOff);
            }
            None => {
                // every handle was dropped
                if timeout(SHUTDOWN_TIMEOUT, connection.log_off()).await.is_err() {
                    debug!("Steam didn't confirm the log off");
                }
                logged_off_for_good(connection.protocol(), calls, events, routes);
                return Next::Stop(Stopped::HandlesDropped);
            }
        },
        // its error is in the protocol, which the next round reads
        _ = connection.step() => {}
        () = sleep_until(deadline) => calls.time_out(connection.protocol()),
    }
    Next::Continue
}

/// Passes on what came while logging off, then that Steam logged us off, if it said so.
fn logged_off_for_good(protocol: &mut CmProtocol, calls: &mut Calls, events: &EventQueue, routes: &mut GcRoutes) {
    calls.deliver(protocol, events, routes);
    if let Some(Ok(result)) = protocol.logged_off() {
        events.push_lifecycle(Event::LoggedOff(result));
    }
}

/// A round of a reconnecting task: calls fail meanwhile.
async fn reconnecting<T>(
    relog: &mut Relog<T>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    events: &EventQueue,
    routes: &mut GcRoutes,
) -> Next<T> {
    tokio::select! {
        command = commands.recv() => match command {
            Some(Command::Call(call)) => call.fail(ClientError::Dropped),
            // kept in the account: set once logged on again
            Some(Command::SetPersonaState(_) | Command::SetGamesPlayed(_)) => {}
            Some(Command::RouteGc(route)) => routes.add(route),
            // the session it was for is gone: its channel got `SessionLost`
            Some(Command::ToGc(message)) => {
                debug!("Not sending to app {}'s game coordinator while connecting again", message.appid());
            }
            Some(Command::LogOff(done)) => {
                let _ = done.send(Ok(()));
                return Next::Stop(Stopped::LoggedOff);
            }
            None => return Next::Stop(Stopped::HandlesDropped),
        },
        relogged = relog.as_mut() => match relogged {
            Ok(relogged) => return Next::Reconnected(Box::new(relogged)),
            Err(RelogFailed { error: ClientError::LogonFailed { result, .. }, .. }) if !retries_logon(result) => {
                warn!("Steam refused the logon ({:?}): stopping", result);
                events.push_lifecycle(Event::LoggedOff(result));
                return Next::Stop(Stopped::LogonRefused(result));
            }
            Err(RelogFailed { error: ClientError::LoggedOff(result), .. }) if !retries_log_off(result) => {
                warn!("Steam logged us off ({:?}): stopping", result);
                events.push_lifecycle(Event::LoggedOff(result));
                return Next::Stop(Stopped::Ended(result));
            }
            Err(RelogFailed { error, endpoint }) => {
                info!("Could not log on again ({}): retrying", error);
                return Next::Retry(endpoint);
            }
        },
    }
    Next::Continue
}

/// Waits `delay`, then connects, avoiding `avoid`, and logs on again as the account.
fn relog<T, A>(connect: Connector<T>, shared: Arc<Shared<A>>, delay: Duration, avoid: Option<String>) -> Relog<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    A: Account,
{
    Box::pin(async move {
        sleep(delay).await;
        let connecting = |error| RelogFailed { error, endpoint: None };
        let (mut connection, endpoint) = timeout(RECONNECT_TIMEOUT, connect(avoid))
            .await
            .map_err(|_| connecting(ClientError::Timeout(EMsg::ChannelEncryptResult)))?
            .map_err(connecting)?;
        let logged_on = async {
            connection.send(EMsg::ClientHello, hello()).await?;
            // the account as it is now: the latest refresh token
            let (steam_id, logon) = shared.state().account.logon();
            connection.log_on(steam_id, logon).await
        }
        .await;
        match logged_on {
            Ok(logon) => Ok(Relogged {
                connection,
                endpoint,
                logon,
            }),
            Err(error) => Err(RelogFailed {
                error,
                endpoint: Some(endpoint),
            }),
        }
    })
}

fn reconnect_delay(attempt: u32) -> Duration {
    FIRST_RECONNECT_DELAY
        .saturating_mul(2u32.saturating_pow(attempt))
        .min(MAX_RECONNECT_DELAY)
}

/// Whether to log on again after Steam logged us off with `result`: node-steam-user's `nonFatalLogOffResults`.
fn retries_log_off(result: EResult) -> bool {
    matches!(
        result,
        EResult::Invalid | EResult::Fail | EResult::NoConnection | EResult::ServiceUnavailable | EResult::TryAnotherCM
    )
}

/// Whether to log on again after Steam refused a logon with `result`: what node-steam-user retries.
fn retries_logon(result: EResult) -> bool {
    matches!(
        result,
        EResult::Fail | EResult::ServiceUnavailable | EResult::TryAnotherCM
    )
}

/// Sleeps until `deadline`, or forever if there is none.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::AtomicU16;
    use std::sync::atomic::Ordering;

    use steam_protobuf::protobuf::Message;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Response;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
    use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGamesPlayed;
    use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicense;
    use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicenseResponse;
    use steam_protobuf::protobufs::steammessages_clientserver_friends::CMsgClientChangeStatus;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;
    use tokio::io::duplex;
    use tokio::io::DuplexStream;

    use super::*;
    use crate::auth::tests::jwt;
    use crate::auth::tests::refresh_token_jwt;
    use crate::auth::tests::STEAM_ID;
    use crate::client::Anonymous;
    use crate::connection::tests::connection;
    use crate::connection::tests::key;
    use crate::connection::tests::soon;
    use crate::connection::tests::Call;
    use crate::connection::tests::FakeCm;
    use crate::connection::tests::TEST_TIMEOUT;
    use crate::gc::GcEvent;
    use crate::protocol::proto_message;
    use crate::protocol::tests::logs;
    use crate::protocol::tests::ANONYMOUS_LOGON_RESPONSE;
    use crate::protocol::tests::ANONYMOUS_STEAM_ID;
    use crate::service::methods::GetPasswordRSAPublicKey;

    const AUTHED: EMsg = EMsg::ServiceMethodCallFromClient;

    /// A spawned client, the fake CM it is connected to, and the fake CMs it connects to again, in order.
    pub(crate) struct Spawned<A> {
        pub(crate) handle: SteamHandle<A>,
        pub(crate) events: Events,
        pub(crate) task: SteamTask,
        pub(crate) cm: FakeCm,
        pub(crate) reconnects: mpsc::UnboundedReceiver<Reconnection>,
    }

    /// A connection the task made again: the fake CM, and the server the task asked to avoid.
    pub(crate) struct Reconnection {
        cm: FakeCm,
        avoid: Option<String>,
    }

    async fn spawned() -> Spawned<Anonymous> {
        spawned_as(Anonymous).await
    }

    /// Logs `account` on over a fake connection to `127.0.0.1:27017`, and spawns it. Connecting again connects to a
    /// new fake CM at the next port, which the test takes from `reconnects`.
    pub(crate) async fn spawned_as<A: Account>(account: A) -> Spawned<A> {
        let (sender, reconnects) = mpsc::unbounded_channel();
        let port = Arc::new(AtomicU16::new(27018));
        let connect = connector(move |avoid| {
            let sender = sender.clone();
            let port = port.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
                let (client, server) = duplex(64 * 1024);
                let cm = FakeCm::new(server, key());
                let _ = sender.send(Reconnection { cm, avoid });
                Ok((
                    Connection::new(client, CmProtocol::encrypted(key())),
                    format!("127.0.0.1:{}", port),
                ))
            })
        });
        let (connection, cm) = connection();
        let (handle, events, task, cm) = spawn_on(connection, cm, account, connect).await;
        Spawned {
            handle,
            events,
            task,
            cm,
            reconnects,
        }
    }

    /// Logs `account` on over `connection`, whose other end is `cm`, and spawns it, connecting again with `connect`.
    async fn spawn_on<A: Account>(
        mut connection: Connection<DuplexStream>,
        mut cm: FakeCm,
        account: A,
        connect: Connector<DuplexStream>,
    ) -> (SteamHandle<A>, Events, SteamTask, FakeCm) {
        let (steam_id, logon) = account.logon();
        let (logon, _) = tokio::join!(connection.log_on(steam_id, logon), logs_on(&mut cm));
        let (handle, events, task) = spawn(
            connection,
            "127.0.0.1:27017".to_string(),
            logon.unwrap(),
            account,
            connect,
        );
        (handle, events, task, cm)
    }

    /// The next fake CM the client connects to.
    pub(crate) async fn reconnected(reconnects: &mut mpsc::UnboundedReceiver<Reconnection>) -> FakeCm {
        soon(reconnects.recv()).await.unwrap().cm
    }

    /// Answers a `ClientLogon` with a session. Returns the logon.
    async fn logs_on(cm: &mut FakeCm) -> CMsgClientLogon {
        let logon = next(cm).await;
        assert_eq!(logon.emsg, EMsg::ClientLogon);
        cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;
        logon.body()
    }

    /// Answers a client connecting again: its hello, then its logon. Returns the logon.
    pub(crate) async fn relogs(cm: &mut FakeCm) -> CMsgClientLogon {
        assert_eq!(next(cm).await.emsg, EMsg::ClientHello);
        logs_on(cm).await
    }

    /// The next message the client sent that isn't a heartbeat.
    pub(crate) async fn next(cm: &mut FakeCm) -> Call {
        soon(async {
            loop {
                let call = cm.read().await;
                if call.emsg != EMsg::ClientHeartBeat {
                    return call;
                }
            }
        })
        .await
    }

    /// The next event.
    pub(crate) async fn recv(events: &mut Events) -> Option<Event> {
        soon(events.recv()).await
    }

    /// Reads the persona state the client sets next, with the session on the header.
    async fn persona_state(cm: &mut FakeCm) -> u32 {
        let status = next(cm).await;
        assert_eq!(status.emsg, EMsg::ClientChangeStatus);
        assert_eq!(status.header.steamid(), ANONYMOUS_STEAM_ID);
        assert!(status.header.has_client_sessionid());
        status.body::<CMsgClientChangeStatus>().persona_state()
    }

    pub(crate) fn user() -> User {
        User::new("account", RefreshToken::new(refresh_token_jwt()).unwrap())
    }

    fn rsa_key_request(name: &str) -> CAuthentication_GetPasswordRSAPublicKey_Request {
        let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
        request.set_account_name(name.to_string());
        request
    }

    /// Answers a call of `rsa_key_request`, with the account name as the key.
    async fn answer(cm: &mut FakeCm, call: &Call) {
        let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
        response.set_publickey_mod(
            call.body::<CAuthentication_GetPasswordRSAPublicKey_Request>()
                .account_name()
                .to_string(),
        );
        cm.reply(call, EResult::OK, response).await;
    }

    /// Calls with `rsa_key_request(name)`, and returns the key.
    async fn call(handle: &SteamHandle<impl Sized>, name: &str) -> Result<String, ClientError> {
        let response = soon(handle.call_service_method(rsa_key_request(name))).await?;
        Ok(response.publickey_mod().to_string())
    }

    /// Calls through the fake CM: once answered, the task handled everything the CM sent before.
    pub(crate) async fn round_trip(handle: &SteamHandle<impl Sized>, cm: &mut FakeCm) {
        let steam = async {
            let call = next(cm).await;
            answer(cm, &call).await;
        };
        tokio::join!(call(handle, "sync"), steam).0.unwrap();
    }

    /// Sends hellos of `versions`, in reads of a few, so that no read holds more than [`EVENT_CAPACITY`].
    async fn send_hellos(handle: &SteamHandle<impl Sized>, cm: &mut FakeCm, versions: std::ops::Range<u32>) {
        let versions: Vec<_> = versions.collect();
        for chunk in versions.chunks(50) {
            for version in chunk {
                cm.send(hello(*version)).await;
            }
            round_trip(handle, cm).await;
        }
    }

    fn logged_off(result: EResult) -> Vec<u8> {
        let mut logged_off = CMsgClientLoggedOff::new();
        logged_off.set_eresult(result as i32);
        proto_message(EMsg::ClientLoggedOff, CMsgProtoBufHeader::new(), logged_off)
    }

    fn logon_refused(result: EResult) -> Vec<u8> {
        let mut response = CMsgClientLogonResponse::new();
        response.set_eresult(result as i32);
        proto_message(EMsg::ClientLogOnResponse, CMsgProtoBufHeader::new(), response)
    }

    fn hello(version: u32) -> Vec<u8> {
        let mut hello = CMsgClientHello::new();
        hello.set_protocol_version(version);
        proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), hello)
    }

    fn multi(packets: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
        let mut body = vec![];
        for packet in packets {
            body.extend_from_slice(&(packet.len() as u32).to_le_bytes());
            body.extend_from_slice(&packet);
        }
        let mut multi = CMsgMulti::new();
        multi.set_message_body(body.into());
        proto_message(EMsg::Multi, CMsgProtoBufHeader::new(), multi)
    }

    fn version(event: &Event) -> Option<u32> {
        match event {
            Event::Message(message) if message.emsg() == EMsg::ClientHello => {
                Some(message.parse_proto::<CMsgClientHello>().unwrap().protocol_version())
            }
            _ => None,
        }
    }

    /// Waits for the task to end, at most `within` (and a moment to run). Returns why it ended.
    async fn ends(task: SteamTask, within: Duration) -> Stopped {
        timeout(within + Duration::from_millis(1), task)
            .await
            .expect("the task still runs")
    }

    fn disconnected(reason: DisconnectReason) -> impl Fn(&Option<Event>) -> bool {
        move |event| matches!(event, Some(Event::Disconnected { reason: got }) if *got == reason)
    }

    #[tokio::test(start_paused = true)]
    async fn sends_heartbeats_while_nothing_calls() {
        let Spawned {
            mut cm, handle, events, ..
        } = spawned().await;
        let start = Instant::now();
        for k in 1..=20u32 {
            let heartbeat = soon(cm.read()).await;
            assert_eq!(heartbeat.emsg, EMsg::ClientHeartBeat);
            assert_eq!(heartbeat.header.steamid(), ANONYMOUS_STEAM_ID);
            assert_eq!(start.elapsed(), Duration::from_secs(9) * k);
        }
        drop((handle, events));
    }

    #[tokio::test(start_paused = true)]
    async fn calls_run_concurrently_and_are_answered_in_any_order() {
        let Spawned { mut cm, handle, .. } = spawned().await;
        let other = handle.clone();

        let steam = async {
            let first = next(&mut cm).await;
            let second = next(&mut cm).await;
            assert_ne!(first.header.jobid_source(), second.header.jobid_source());
            assert_eq!(first.emsg, AUTHED);
            assert_eq!(first.header.steamid(), ANONYMOUS_STEAM_ID);
            answer(&mut cm, &second).await;
            answer(&mut cm, &first).await;
        };
        let (first, second, ()) = tokio::join!(call(&handle, "first"), call(&other, "second"), steam);
        assert_eq!(first.unwrap(), "first");
        assert_eq!(second.unwrap(), "second");
    }

    #[tokio::test(start_paused = true)]
    async fn calls_by_marker_type() {
        let Spawned { mut cm, handle, .. } = spawned().await;
        let steam = async {
            let call = next(&mut cm).await;
            answer(&mut cm, &call).await;
        };
        let (response, ()) = tokio::join!(
            soon(handle.call::<GetPasswordRSAPublicKey>(rsa_key_request("marker"))),
            steam
        );
        assert_eq!(response.unwrap().publickey_mod(), "marker");
    }

    fn free_license_request(app_id: u32) -> CMsgClientRequestFreeLicense {
        let mut request = CMsgClientRequestFreeLicense::new();
        request.appids = vec![app_id];
        request
    }

    /// A `ClientRequestFreeLicenseResponse` body, as `emsg`, to job `job_id`.
    fn free_license_response(emsg: EMsg, job_id: u64, result: EResult, granted: &[u32]) -> Vec<u8> {
        let mut response = CMsgClientRequestFreeLicenseResponse::new();
        response.set_eresult(result as u32);
        response.granted_appids = granted.to_vec();
        let mut header = CMsgProtoBufHeader::new();
        header.set_jobid_target(job_id);
        proto_message(emsg, header, response)
    }

    #[tokio::test(start_paused = true)]
    async fn job_messages_are_answered_by_their_own_message() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        let steam = async {
            let call = next(&mut cm).await;
            assert_eq!(call.emsg, EMsg::ClientRequestFreeLicense);
            assert_eq!(call.header.steamid(), ANONYMOUS_STEAM_ID);
            assert!(call.header.has_client_sessionid());
            assert!(!call.header.has_target_job_name());
            assert_eq!(call.body::<CMsgClientRequestFreeLicense>().appids, [440]);
            let job_id = call.header.jobid_source();
            // another message to the same job doesn't answer it
            cm.send(free_license_response(
                EMsg::ServiceMethodResponse,
                job_id,
                EResult::OK,
                &[],
            ))
            .await;
            cm.send(free_license_response(
                EMsg::ClientRequestFreeLicenseResponse,
                job_id,
                EResult::OK,
                &[440],
            ))
            .await;
        };
        let (granted, ()) = tokio::join!(soon(handle.call_job_message(free_license_request(440))), steam);
        assert_eq!(granted.unwrap().granted_appids, [440]);
        assert!(
            matches!(recv(&mut events).await, Some(Event::Message(message)) if message.emsg() == EMsg::ServiceMethodResponse)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn job_messages_fail_with_their_result_or_time_out_waiting_for_their_answer() {
        let Spawned { mut cm, handle, .. } = spawned().await;
        let steam = async {
            let call = next(&mut cm).await;
            cm.send(free_license_response(
                EMsg::ClientRequestFreeLicenseResponse,
                call.header.jobid_source(),
                EResult::RateLimitExceeded,
                &[],
            ))
            .await;
        };
        let (refused, ()) = tokio::join!(
            soon(handle.call_job::<crate::job::messages::RequestFreeLicense>(free_license_request(440))),
            steam
        );
        assert!(
            matches!(
                refused,
                Err(ClientError::JobMessage {
                    request: EMsg::ClientRequestFreeLicense,
                    result: EResult::RateLimitExceeded,
                })
            ),
            "{:?}",
            refused
        );

        let start = Instant::now();
        let (result, _) = tokio::join!(handle.call_job_message(free_license_request(570)), next(&mut cm));
        assert!(
            matches!(
                result,
                Err(ClientError::Timeout(EMsg::ClientRequestFreeLicenseResponse))
            ),
            "{:?}",
            result
        );
        assert_eq!(start.elapsed(), RESPONSE_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_calls_response_is_an_event() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;

        let job_id = tokio::select! {
            _ = call(&handle, "dropped") => panic!("nothing answered"),
            call = next(&mut cm) => call.header.jobid_source(),
        };
        // the caller is gone before the response comes
        let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
        response.set_publickey_mod("late".to_string());
        let mut header = CMsgProtoBufHeader::new();
        header.set_jobid_target(job_id);
        cm.send(proto_message(EMsg::ServiceMethodResponse, header, response))
            .await;

        match recv(&mut events).await.unwrap() {
            Event::Message(message) => assert_eq!(
                message
                    .parse_proto::<CAuthentication_GetPasswordRSAPublicKey_Response>()
                    .unwrap()
                    .publickey_mod(),
                "late"
            ),
            event => panic!("expected the late response, got {:?}", event),
        }
        // and the next calls are answered
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_call_is_forgotten_before_its_response() {
        let Spawned { mut cm, handle, .. } = spawned().await;
        tokio::select! {
            _ = call(&handle, "dropped") => panic!("nothing answered"),
            _ = next(&mut cm) => {}
        };
        // a heartbeat wakes the task, which sees the caller gone; the call doesn't time out later
        assert_eq!(soon(cm.read()).await.emsg, EMsg::ClientHeartBeat);
        round_trip(&handle, &mut cm).await;
    }

    #[test]
    fn a_call_whose_caller_is_gone_is_forgotten() {
        let mut protocol = CmProtocol::encrypted(key());
        let (sender, receiver) = oneshot::channel();
        let typed = TypedCall::<Service<GetPasswordRSAPublicKey>> {
            request: rsa_key_request("gone"),
            sender,
            deadline: Instant::now() + RESPONSE_TIMEOUT,
        };
        let mut calls = Calls::default();
        let pending = Box::new(typed).start(&mut protocol);
        let job_id = pending.job_id;
        calls.pending.push(pending);
        drop(receiver);

        let events = EventQueue::default();
        calls.deliver(&mut protocol, &events, &mut GcRoutes::default());
        assert!(calls.pending.is_empty());
        // forgotten by the protocol too: a response that still comes is an event
        let mut header = CMsgProtoBufHeader::new();
        header.set_jobid_target(job_id);
        let response = proto_message(
            EMsg::ServiceMethodResponse,
            header,
            CAuthentication_GetPasswordRSAPublicKey_Response::new(),
        );
        protocol
            .handle_input(
                &crate::protocol::framing::encrypt(&response, &key()),
                Instant::now().into_std(),
            )
            .unwrap();
        assert!(protocol.take_answer(job_id).is_none());
        calls.deliver(&mut protocol, &events, &mut GcRoutes::default());
        assert!(matches!(events.lock().pop(), Some(Event::Message(_))));
    }

    #[tokio::test(start_paused = true)]
    async fn calls_time_out() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        let start = Instant::now();
        let (result, late) = tokio::join!(call(&handle, "slow"), next(&mut cm));
        assert!(matches!(result, Err(ClientError::Timeout(EMsg::ServiceMethodResponse))));
        assert_eq!(start.elapsed(), RESPONSE_TIMEOUT);

        // a response that still comes is an event
        answer(&mut cm, &late).await;
        assert!(
            matches!(recv(&mut events).await, Some(Event::Message(message)) if message.emsg() == EMsg::ServiceMethodResponse)
        );
        // and one timeout doesn't take the connection for lost
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn calls_that_keep_timing_out_connect_again() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned().await;
        let calls = async {
            let (first, second, third) = tokio::join!(call(&handle, "1"), call(&handle, "2"), call(&handle, "3"));
            for result in [first, second, third] {
                assert!(matches!(result, Err(ClientError::Timeout(_))));
            }
        };
        // a call made later still waits when the connection is taken for lost: it fails as when it is lost
        let later = async {
            sleep(Duration::from_secs(1)).await;
            call(&handle, "4").await
        };
        // Steam reads the calls, and never answers
        let steam = async {
            for _ in 0..4 {
                next(&mut cm).await;
            }
        };
        let ((), later, ()) = tokio::join!(calls, later, steam);
        assert!(matches!(later, Err(ClientError::Dropped)));
        assert!(disconnected(DisconnectReason::Unresponsive)(&recv(&mut events).await));
        relogs(&mut reconnected(&mut reconnects).await).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn an_answered_call_starts_the_timeouts_over() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        for _ in 0..2 {
            let (result, _) = tokio::join!(call(&handle, "slow"), next(&mut cm));
            assert!(matches!(result, Err(ClientError::Timeout(_))));
        }
        round_trip(&handle, &mut cm).await;
        for _ in 0..2 {
            let (result, _) = tokio::join!(call(&handle, "slow"), next(&mut cm));
            assert!(matches!(result, Err(ClientError::Timeout(_))));
        }
        round_trip(&handle, &mut cm).await;
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test(start_paused = true)]
    async fn events_not_read_drop_the_oldest_and_count_them_and_calls_are_still_answered() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        let sent = EVENT_CAPACITY as u32 + 10;
        // with nothing read, calls go through: the task never waits for the reader
        send_hellos(&handle, &mut cm, 0..sent).await;

        // the oldest went, and their count comes first
        assert!(matches!(recv(&mut events).await, Some(Event::Lagged(10))));
        let mut versions = vec![];
        while let Ok(event) = events.try_recv() {
            versions.push(version(&event).unwrap());
        }
        assert_eq!(versions, (10..sent).collect::<Vec<_>>());

        // and newer events after them
        cm.send(hello(sent)).await;
        assert_eq!(version(&recv(&mut events).await.unwrap()), Some(sent));
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_big_multi_is_kept_whole() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        let count = EVENT_CAPACITY as u32 + 44;
        cm.send(multi((0..count).map(hello))).await;
        round_trip(&handle, &mut cm).await;
        let mut versions = vec![];
        while let Ok(event) = events.try_recv() {
            versions.push(version(&event).expect("no lag"));
        }
        assert_eq!(versions, (0..count).collect::<Vec<_>>());
    }

    #[tokio::test(start_paused = true)]
    async fn a_read_past_the_protocols_bound_is_counted() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned().await;
        let count = crate::protocol::MAX_EVENTS_PER_READ as u32 + 5;
        cm.send(multi((0..count).map(hello))).await;
        round_trip(&handle, &mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Lagged(5))));
        assert_eq!(version(&recv(&mut events).await.unwrap()), Some(5));
    }

    #[tokio::test(start_paused = true)]
    async fn lifecycle_events_are_kept_however_slow_the_reader() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            task,
        } = spawned().await;
        send_hellos(&handle, &mut cm, 0..EVENT_CAPACITY as u32 + 5).await;
        drop(cm);
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        // logged on again, with the events left unread
        soon(async {
            while handle.endpoint() != "127.0.0.1:27018" {
                sleep(Duration::from_millis(1)).await;
            }
        })
        .await;
        send_hellos(&handle, &mut cm, 0..EVENT_CAPACITY as u32 + 3).await;
        cm.send(logged_off(EResult::LogonSessionReplaced)).await;
        assert_eq!(
            ends(task, TEST_TIMEOUT).await,
            Stopped::Ended(EResult::LogonSessionReplaced)
        );

        // read once the task is gone: nothing is missing, and the messages of each session are where they were
        let mut all = vec![];
        while let Some(event) = recv(&mut events).await {
            all.push(event);
        }
        let lagged: u64 = all
            .iter()
            .map(|event| match event {
                Event::Lagged(count) => *count,
                _ => 0,
            })
            .sum();
        let messages = all.iter().filter(|event| version(event).is_some()).count() as u64;
        assert_eq!(lagged + messages, 2 * EVENT_CAPACITY as u64 + 8);
        assert_eq!(messages, EVENT_CAPACITY as u64);
        let disconnected = all
            .iter()
            .position(|event| matches!(event, Event::Disconnected { .. }))
            .unwrap();
        assert!(matches!(all[disconnected + 1], Event::Reconnected));
        assert!(matches!(
            all.last(),
            Some(Event::LoggedOff(EResult::LogonSessionReplaced))
        ));
        assert!(matches!(events.try_recv(), Err(TryRecvError::Stopped)));
    }

    #[test]
    fn dropped_messages_leave_a_count_in_their_place() {
        logs::capture();
        let mut queue = Queue::default();
        let message = || Event::Message(RawMessage::new(EMsg::ClientHello, hello(0).into()));
        queue.push_message(message());
        queue.events.push_back(Event::Reconnected);
        queue.lifecycle += 1;
        queue.push_message(message());
        queue.push_message(message());
        queue.trim(2);
        queue.trim(1);
        queue.trim(0);
        assert!(matches!(
            queue.events.make_contiguous(),
            [Event::Lagged(1), Event::Reconnected, Event::Lagged(2)]
        ));
        let warned = || {
            logs::take()
                .into_iter()
                .filter(|line| line.contains("aren't read as fast"))
                .count()
        };
        assert_eq!(warned(), 1);
        // logged once while the reader lags
        queue.push_message(message());
        queue.trim(0);
        assert_eq!(warned(), 0);
        // and again once it read a count
        assert!(matches!(queue.pop(), Some(Event::Lagged(1))));
        queue.push_message(message());
        queue.trim(0);
        assert_eq!(warned(), 1);
        assert!(matches!(queue.pop(), Some(Event::Reconnected)));
        assert!(matches!(queue.pop(), Some(Event::Lagged(4))));
        assert!(queue.pop().is_none());
        assert_eq!((queue.messages, queue.lifecycle), (0, 0));

        // a count after the message dropped joins it, as one before does
        queue.push_message(message());
        queue.lag(2);
        queue.push_message(message());
        queue.trim(1);
        assert!(matches!(
            queue.events.make_contiguous(),
            [Event::Lagged(3), Event::Message(_)]
        ));
        // and the protocol's count joins one before it
        queue.trim(0);
        queue.lag(4);
        assert!(matches!(queue.events.make_contiguous(), [Event::Lagged(8)]));
    }

    #[test]
    fn events_are_not_kept_once_the_reader_is_gone() {
        let queue = Arc::new(EventQueue::default());
        let events = Events(queue.clone());
        let message = || RawMessage::new(EMsg::ClientHello, hello(0).into());
        queue.push_message(message());
        drop(events);
        assert!(queue.lock().events.is_empty());
        queue.push_message(message());
        queue.push_lifecycle(Event::Reconnected);
        let mut protocol = CmProtocol::encrypted(key());
        protocol
            .handle_input(
                &crate::protocol::framing::encrypt(&hello(1), &key()),
                Instant::now().into_std(),
            )
            .unwrap();
        queue.push_read(&mut protocol, &mut GcRoutes::default());
        let queue = queue.lock();
        assert!(queue.events.is_empty());
        assert_eq!((queue.messages, queue.lifecycle), (0, 0));
    }

    #[tokio::test(start_paused = true)]
    async fn calls_time_out_at_their_deadline() {
        let mut protocol = CmProtocol::encrypted(key());
        let call = |deadline| {
            let (sender, receiver) = oneshot::channel();
            let typed = Box::new(TypedCall::<Service<GetPasswordRSAPublicKey>> {
                request: rsa_key_request("due"),
                sender,
                deadline,
            });
            (typed, receiver)
        };
        let mut calls = Calls::default();
        let (made, mut made_answer) = call(Instant::now());
        calls.pending.push(made.start(&mut protocol));
        let (not_due, _not_due_answer) = call(Instant::now() + Duration::from_secs(1));
        calls.parked.push_back(not_due);
        let (parked, mut parked_answer) = call(Instant::now());
        calls.parked.push_back(parked);
        // the task wakes at the deadline, the clock still on it: the calls are due then, not later
        calls.time_out(&mut protocol);
        assert!(calls.pending.is_empty() && calls.parked.len() == 1);
        assert!(matches!(made_answer.try_recv(), Ok(Err(ClientError::Timeout(_)))));
        assert!(matches!(parked_answer.try_recv(), Ok(Err(ClientError::Timeout(_)))));
        assert_eq!(calls.timeouts, 2);
    }

    #[test]
    fn merging_sessions_joins_the_counts_around_them() {
        let mut queue = Queue::default();
        let reason = DisconnectReason::ConnectionLost;
        for event in [
            Event::Lagged(1),
            Event::Reconnected,
            Event::Lagged(2),
            Event::Disconnected { reason },
            Event::Lagged(3),
        ] {
            queue.events.push_back(event);
        }
        queue.lifecycle = 2;
        queue.merge_sessions();
        assert!(matches!(queue.events.make_contiguous(), [Event::Lagged(6)]));
        assert_eq!(queue.lifecycle, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn timeouts_count_again_on_a_new_connection() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned().await;
        for _ in 0..TIMEOUTS_BEFORE_RECONNECT - 1 {
            let (result, _) = tokio::join!(call(&handle, "slow"), next(&mut cm));
            assert!(matches!(result, Err(ClientError::Timeout(_))));
        }
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        // one more doesn't make three in a row
        let (result, _) = tokio::join!(call(&handle, "slow"), next(&mut cm));
        assert!(matches!(result, Err(ClientError::Timeout(_))));
        round_trip(&handle, &mut cm).await;
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn sessions_nothing_is_left_of_are_merged() {
        let events = EventQueue::default();
        let message = || RawMessage::new(EMsg::ClientHello, hello(0).into());
        let reason = |first| DisconnectReason::LoggedOff(if first { EResult::Fail } else { EResult::TryAnotherCM });
        events.push_lifecycle(Event::Disconnected { reason: reason(true) });
        // sessions whose messages are dropped, but the latest's
        for _ in 0..MAX_LIFECYCLE_EVENTS {
            events.push_lifecycle(Event::Reconnected);
            events.push_message(message());
            events.lock().trim(1);
            events.push_lifecycle(Event::Disconnected { reason: reason(false) });
        }

        let mut queue = events.lock();
        assert!(queue.lifecycle <= MAX_LIFECYCLE_EVENTS / 2 + 2, "{}", queue.lifecycle);
        let events = queue.events.make_contiguous();
        // the first disconnection says why; the latest session, with its message, stays
        assert!(matches!(
            events.first(),
            Some(Event::Disconnected {
                reason: DisconnectReason::LoggedOff(EResult::Fail)
            })
        ));
        assert!(matches!(
            &events[events.len() - 3..],
            [Event::Reconnected, Event::Message(_), Event::Disconnected { .. }]
        ));
        let lagged: u64 = queue
            .events
            .iter()
            .map(|event| match event {
                Event::Lagged(count) => *count,
                _ => 0,
            })
            .sum();
        assert_eq!(lagged + queue.messages as u64, MAX_LIFECYCLE_EVENTS as u64);
        // lifecycle events still alternate, and counts never follow each other
        let kinds: Vec<_> = queue
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Disconnected { .. } => Some(true),
                Event::Reconnected => Some(false),
                _ => None,
            })
            .collect();
        assert!(kinds.windows(2).all(|pair| pair[0] != pair[1]), "{:?}", kinds);
        assert!(queue
            .events
            .iter()
            .zip(queue.events.iter().skip(1))
            .all(|pair| !matches!(pair, (Event::Lagged(_), Event::Lagged(_)))));
        assert_eq!(queue.lifecycle, kinds.len());
    }

    #[tokio::test(start_paused = true)]
    async fn connects_again_elsewhere_when_the_connection_is_lost() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned().await;
        let mut cm = Some(cm);

        // a call waiting fails
        let steam = async {
            let mut steam = cm.take().unwrap();
            next(&mut steam).await;
            drop(steam);
        };
        assert!(matches!(
            tokio::join!(call(&handle, "lost"), steam).0,
            Err(ClientError::Dropped)
        ));
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let lost = Instant::now();

        // calls fail until the client logged on again
        assert!(matches!(call(&handle, "early").await, Err(ClientError::Dropped)));
        let Reconnection { mut cm, avoid } = soon(reconnects.recv()).await.unwrap();
        assert_eq!(lost.elapsed(), FIRST_RECONNECT_DELAY);
        // to another server than the one lost
        assert_eq!(avoid.as_deref(), Some("127.0.0.1:27017"));
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        assert_eq!(handle.endpoint(), "127.0.0.1:27018");

        // then go to the new connection
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn retries_with_a_growing_delay_avoiding_the_server_that_refused() {
        let Spawned {
            cm,
            mut events,
            mut reconnects,
            handle: _handle,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let lost = Instant::now();

        // the connection closes before the logon
        let first = soon(reconnects.recv()).await.unwrap();
        assert_eq!(lost.elapsed(), Duration::from_secs(1));
        drop(first.cm);
        // Steam refuses it, in a way worth retrying
        let Reconnection { mut cm, avoid } = soon(reconnects.recv()).await.unwrap();
        assert_eq!(lost.elapsed(), Duration::from_secs(1 + 2));
        // closed before it said hello: that server is still the one to avoid
        assert_eq!(avoid.as_deref(), Some("127.0.0.1:27018"));
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientHello);
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogon);
        cm.send(logon_refused(EResult::TryAnotherCM)).await;

        let Reconnection { mut cm, avoid } = soon(reconnects.recv()).await.unwrap();
        assert_eq!(lost.elapsed(), Duration::from_secs(1 + 2 + 4));
        assert_eq!(avoid.as_deref(), Some("127.0.0.1:27019"));
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
    }

    #[test]
    fn the_delay_is_capped() {
        let delays: Vec<_> = (0..10).map(|attempt| reconnect_delay(attempt).as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 32, 60, 60, 60, 60]);
        assert_eq!(reconnect_delay(u32::MAX), MAX_RECONNECT_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn the_delay_starts_over_only_once_a_session_lasted() {
        let Spawned {
            mut cm,
            mut events,
            mut reconnects,
            handle,
            ..
        } = spawned().await;
        // Steam takes each logon, and logs off at once
        let mut delays = vec![];
        for _ in 0..4 {
            cm.send(logged_off(EResult::TryAnotherCM)).await;
            let kicked = Instant::now();
            cm = reconnected(&mut reconnects).await;
            delays.push(kicked.elapsed().as_secs());
            relogs(&mut cm).await;
            let reason = DisconnectReason::LoggedOff(EResult::TryAnotherCM);
            assert!(disconnected(reason)(&recv(&mut events).await));
            assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        }
        assert_eq!(delays, [1, 2, 4, 8]);

        // a session that lasts a minute: the next delay is the first again
        sleep(STABLE_SESSION).await;
        round_trip(&handle, &mut cm).await;
        cm.send(logged_off(EResult::TryAnotherCM)).await;
        let kicked = Instant::now();
        relogs(&mut reconnected(&mut reconnects).await).await;
        assert_eq!(kicked.elapsed(), FIRST_RECONNECT_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn connecting_again_that_never_ends_is_retried() {
        let (sender, mut reconnects) = mpsc::unbounded_channel();
        let attempts = Arc::new(AtomicU16::new(0));
        let connect = connector(move |_| {
            let sender = sender.clone();
            let attempt = attempts.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
                if attempt == 0 {
                    // as Steam's directory, when it never answers
                    std::future::pending::<()>().await;
                }
                let (client, server) = duplex(64 * 1024);
                let _ = sender.send(FakeCm::new(server, key()));
                Ok((
                    Connection::new(client, CmProtocol::encrypted(key())),
                    "127.0.0.1:27018".to_string(),
                ))
            })
        });
        let (connection, cm) = connection();
        let (_handle, mut events, _task, cm) = spawn_on(connection, cm, Anonymous, connect).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let lost = Instant::now();
        let mut cm = soon(reconnects.recv()).await.unwrap();
        assert_eq!(
            lost.elapsed(),
            Duration::from_secs(1) + RECONNECT_TIMEOUT + Duration::from_secs(2)
        );
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
    }

    #[test]
    fn retries_what_node_steam_user_retries() {
        use EResult::*;
        for result in [Invalid, Fail, NoConnection, ServiceUnavailable, TryAnotherCM] {
            assert!(retries_log_off(result), "{:?}", result);
        }
        for result in [
            OK,
            LogonSessionReplaced,
            AccessDenied,
            Expired,
            InvalidPassword,
            RateLimitExceeded,
            Banned,
        ] {
            assert!(!retries_log_off(result), "{:?}", result);
        }
        for result in [Fail, ServiceUnavailable, TryAnotherCM] {
            assert!(retries_logon(result), "{:?}", result);
        }
        for result in [
            Invalid,
            NoConnection,
            AccessDenied,
            Expired,
            InvalidPassword,
            RateLimitExceeded,
        ] {
            assert!(!retries_logon(result), "{:?}", result);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn logs_on_again_when_steam_logs_off_in_a_way_worth_retrying() {
        use EResult::*;
        for result in [Invalid, Fail, NoConnection, ServiceUnavailable, TryAnotherCM] {
            let Spawned {
                mut cm,
                handle,
                mut events,
                mut reconnects,
                ..
            } = spawned().await;
            // a call waiting fails as when the connection is lost
            let steam = async {
                next(&mut cm).await;
                cm.send(logged_off(result)).await;
            };
            assert!(
                matches!(tokio::join!(call(&handle, "x"), steam).0, Err(ClientError::Dropped)),
                "{:?}",
                result
            );
            // no `LoggedOff`: the client goes on
            assert!(disconnected(DisconnectReason::LoggedOff(result))(
                &recv(&mut events).await
            ));
            let Reconnection { mut cm, avoid } = soon(reconnects.recv()).await.unwrap();
            assert_eq!(avoid.as_deref(), Some("127.0.0.1:27017"));
            relogs(&mut cm).await;
            assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_steam_logs_off_for_good() {
        use EResult::*;
        for result in [LogonSessionReplaced, AccessDenied, OK, RateLimitExceeded] {
            let Spawned {
                mut cm,
                handle,
                mut events,
                task,
                mut reconnects,
            } = spawned().await;
            let steam = async {
                next(&mut cm).await;
                cm.send(logged_off(result)).await;
            };
            // a call waiting fails with why
            let (failed, ()) = tokio::join!(call(&handle, "x"), steam);
            assert!(matches!(failed, Err(ClientError::LoggedOff(got)) if got == result));
            assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(got)) if got == result));
            assert!(recv(&mut events).await.is_none());
            assert!(matches!(call(&handle, "after").await, Err(ClientError::Closed)));
            assert!(matches!(handle.clone().log_off().await, Err(ClientError::Closed)));
            assert_eq!(ends(task, Duration::ZERO).await, Stopped::Ended(result));
            // without connecting again
            assert!(reconnects.recv().await.is_none());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retries_a_logon_steam_refuses_in_a_way_worth_retrying() {
        use EResult::*;
        for result in [Fail, ServiceUnavailable, TryAnotherCM] {
            let Spawned {
                cm,
                mut events,
                mut reconnects,
                handle: _handle,
                ..
            } = spawned().await;
            drop(cm);
            assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
            let mut cm = reconnected(&mut reconnects).await;
            next(&mut cm).await;
            next(&mut cm).await;
            cm.send(logon_refused(result)).await;
            relogs(&mut reconnected(&mut reconnects).await).await;
            assert!(
                matches!(recv(&mut events).await, Some(Event::Reconnected)),
                "{:?}",
                result
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_steam_refuses_to_log_on_again() {
        use EResult::*;
        for result in [AccessDenied, Expired, InvalidPassword, Invalid, NoConnection] {
            let Spawned {
                cm,
                mut events,
                task,
                mut reconnects,
                handle: _handle,
                ..
            } = spawned().await;
            drop(cm);
            assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
            let mut cm = reconnected(&mut reconnects).await;
            next(&mut cm).await;
            next(&mut cm).await;
            cm.send(logon_refused(result)).await;

            assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(got)) if got == result));
            assert!(recv(&mut events).await.is_none());
            assert_eq!(ends(task, Duration::ZERO).await, Stopped::LogonRefused(result));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_steam_logs_off_for_good_while_logging_on_again() {
        let Spawned {
            cm,
            mut events,
            task,
            mut reconnects,
            handle: _handle,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        next(&mut cm).await;
        next(&mut cm).await;
        cm.send(logged_off(EResult::LogonSessionReplaced)).await;

        assert!(matches!(
            recv(&mut events).await,
            Some(Event::LoggedOff(EResult::LogonSessionReplaced))
        ));
        assert!(recv(&mut events).await.is_none());
        assert_eq!(
            ends(task, Duration::ZERO).await,
            Stopped::Ended(EResult::LogonSessionReplaced)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_session_that_goes_while_logging_on_again_is_retried() {
        let Spawned {
            cm,
            mut events,
            mut reconnects,
            handle: _handle,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        next(&mut cm).await;
        next(&mut cm).await;
        cm.send(logged_off(EResult::TryAnotherCM)).await;
        relogs(&mut reconnected(&mut reconnects).await).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn events_nobody_reads_are_dropped() {
        let Spawned {
            mut cm, handle, events, ..
        } = spawned().await;
        send_hellos(&handle, &mut cm, 0..EVENT_CAPACITY as u32 + 2).await;
        drop(events);
        cm.send(hello(0)).await;
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn logs_off_once_every_handle_is_dropped() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            task,
            ..
        } = spawned().await;

        // dropping the events alone keeps it running
        let other = handle.clone();
        drop(handle);
        round_trip(&other, &mut cm).await;
        drop(other);
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogOff);
        cm.send(logged_off(EResult::OK)).await;
        // a reader looping until the events end stops too
        assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(EResult::OK))));
        assert!(recv(&mut events).await.is_none());
        assert_eq!(ends(task, Duration::ZERO).await, Stopped::HandlesDropped);
        soon(cm.closed()).await;
    }

    #[tokio::test(start_paused = true)]
    async fn the_task_ends_once_logged_off_and_the_connection_closed() {
        let Spawned {
            mut cm,
            handle,
            events,
            task,
            ..
        } = spawned().await;
        drop(events);
        assert_eq!(soon(cm.read()).await.emsg, EMsg::ClientHeartBeat);
        drop(handle);
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogOff);
        // Steam doesn't confirm: it has a while
        let start = Instant::now();
        assert_eq!(soon(task).await, Stopped::HandlesDropped);
        assert_eq!(start.elapsed(), SHUTDOWN_TIMEOUT);
        soon(cm.closed()).await;
    }

    #[tokio::test(start_paused = true)]
    async fn stops_while_reconnecting_once_the_handles_are_dropped() {
        let Spawned {
            cm,
            handle,
            mut events,
            task,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        drop(handle);
        assert!(recv(&mut events).await.is_none());
        assert_eq!(ends(task, Duration::ZERO).await, Stopped::HandlesDropped);
    }

    #[tokio::test(start_paused = true)]
    async fn logs_off_explicitly() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            task,
            ..
        } = spawned().await;
        let other = handle.clone();

        let steam = async {
            assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogOff);
            cm.send(logged_off(EResult::OK)).await;
        };
        tokio::join!(async { soon(handle.log_off()).await.unwrap() }, steam);
        assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(EResult::OK))));
        assert!(recv(&mut events).await.is_none());
        assert!(matches!(call(&other, "after").await, Err(ClientError::Closed)));
        assert_eq!(ends(task, Duration::ZERO).await, Stopped::LoggedOff);
    }

    #[tokio::test(start_paused = true)]
    async fn logging_off_while_reconnecting_stops() {
        let Spawned {
            cm,
            handle,
            mut events,
            task,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        soon(handle.log_off()).await.unwrap();
        assert!(recv(&mut events).await.is_none());
        assert_eq!(ends(task, Duration::ZERO).await, Stopped::LoggedOff);
    }

    #[tokio::test(start_paused = true)]
    async fn try_recv_tells_empty_from_stopped() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            task,
            ..
        } = spawned().await;
        assert_eq!(events.try_recv().unwrap_err(), TryRecvError::Empty);
        cm.send(hello(1)).await;
        round_trip(&handle, &mut cm).await;
        assert_eq!(version(&events.try_recv().unwrap()), Some(1));
        assert_eq!(events.try_recv().unwrap_err(), TryRecvError::Empty);
        let steam = async {
            next(&mut cm).await;
            drop(cm);
        };
        tokio::join!(async { soon(handle.log_off()).await.unwrap() }, steam);
        assert_eq!(ends(task, Duration::ZERO).await, Stopped::LoggedOff);
        assert_eq!(events.try_recv().unwrap_err(), TryRecvError::Stopped);
        assert_eq!(TryRecvError::Stopped.to_string(), "the client stopped");
    }

    #[tokio::test(start_paused = true)]
    async fn has_what_the_logon_gave() {
        let Spawned {
            handle, events, task, ..
        } = spawned().await;
        assert_eq!(handle.steam_id().to_steam64(), ANONYMOUS_STEAM_ID);
        assert_eq!(handle.session_id(), -139116189);
        assert_eq!(handle.heartbeat_interval(), Duration::from_secs(9));
        assert_eq!(handle.cell_id(), 25);
        assert_eq!(handle.public_ip(), Some(Ipv4Addr::new(203, 0, 113, 7)));
        assert_eq!(handle.endpoint(), "127.0.0.1:27017");
        assert!(format!("{:?}", handle).contains("127.0.0.1:27017"));
        assert_eq!(format!("{:?}", events), "Events { waiting: 0 }");
        assert_eq!(format!("{:?}", task), "SteamTask { finished: false }");
    }

    /// A logon response for another session than [`ANONYMOUS_LOGON_RESPONSE`].
    fn another_session() -> Vec<u8> {
        let mut header = CMsgProtoBufHeader::new();
        header.set_steamid(ANONYMOUS_STEAM_ID);
        header.set_client_sessionid(77);
        let mut response = CMsgClientLogonResponse::new();
        response.set_eresult(EResult::OK as i32);
        response.set_heartbeat_seconds(20);
        response.set_cell_id(3);
        proto_message(EMsg::ClientLogOnResponse, header, response)
    }

    #[tokio::test(start_paused = true)]
    async fn has_what_the_latest_logon_gave() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned().await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientHello);
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogon);
        cm.send(another_session()).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));

        assert_eq!(handle.session_id(), 77);
        assert_eq!(handle.cell_id(), 3);
        assert_eq!(handle.heartbeat_interval(), Duration::from_secs(20));
        assert_eq!(handle.public_ip(), None);
        let steam = async {
            let call = next(&mut cm).await;
            assert_eq!(call.header.client_sessionid(), 77);
            answer(&mut cm, &call).await;
        };
        tokio::join!(call(&handle, "new session"), steam).0.unwrap();
    }

    fn renewed_token() -> String {
        jwt(&format!(r#"{{"sub":"{}","exp":1900000000}}"#, STEAM_ID))
    }

    /// A `GenerateAccessTokenForApp` response, with a new refresh token if one is given.
    fn tokens(refresh_token: Option<String>) -> CAuthentication_AccessToken_GenerateForApp_Response {
        let mut response = CAuthentication_AccessToken_GenerateForApp_Response::new();
        response.set_access_token(jwt(&format!(r#"{{"sub":"{}","exp":1700086400}}"#, STEAM_ID)));
        if let Some(refresh_token) = refresh_token {
            response.set_refresh_token(refresh_token);
        }
        response
    }

    #[tokio::test(start_paused = true)]
    async fn a_user_logs_on_again_with_the_latest_refresh_token() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        assert_eq!(handle.account_name(), "account");

        let steam = async {
            for (renewal, refresh_token) in [
                (ETokenRenewalType::k_ETokenRenewalType_None, None),
                (ETokenRenewalType::k_ETokenRenewalType_Allow, Some(renewed_token())),
            ] {
                let (call, request) =
                    soon(cm.expect::<CAuthentication_AccessToken_GenerateForApp_Request>(AUTHED)).await;
                assert_eq!(request.refresh_token(), refresh_token_jwt());
                assert_eq!(request.renewal_type(), renewal);
                cm.reply(&call, EResult::OK, tokens(refresh_token)).await;
            }
        };
        let tokens = async {
            soon(handle.web_access_token()).await.unwrap();
            soon(handle.renew_tokens()).await.unwrap()
        };
        let (renewed, ()) = tokio::join!(tokens, steam);
        assert_eq!(renewed.refresh_token.unwrap().expose_secret(), renewed_token());
        assert_eq!(handle.refresh_token().expose_secret(), renewed_token());

        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let logon = relogs(&mut reconnected(&mut reconnects).await).await;
        assert_eq!(logon.account_name(), "account");
        assert_eq!(logon.access_token(), renewed_token());
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_renewed_token_is_kept_though_nobody_waits_for_it() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        let call = tokio::select! {
            _ = handle.renew_tokens() => panic!("nothing answered"),
            call = soon(cm.expect::<CAuthentication_AccessToken_GenerateForApp_Request>(AUTHED)) => call.0,
        };
        // the caller is gone when Steam replaces the token, and revokes the old one
        cm.reply(&call, EResult::OK, tokens(Some(renewed_token()))).await;
        round_trip(&handle, &mut cm).await;
        assert_eq!(handle.refresh_token().expose_secret(), renewed_token());
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));

        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let logon = relogs(&mut reconnected(&mut reconnects).await).await;
        assert_eq!(logon.access_token(), renewed_token());
    }

    #[tokio::test(start_paused = true)]
    async fn a_renewed_token_that_comes_late_is_kept() {
        let Spawned { mut cm, handle, .. } = spawned_as(user()).await;
        let (renewed, (call, _)) = tokio::join!(
            handle.renew_tokens(),
            soon(cm.expect::<CAuthentication_AccessToken_GenerateForApp_Request>(AUTHED))
        );
        assert!(matches!(renewed, Err(ClientError::Timeout(_))));
        cm.reply(&call, EResult::OK, tokens(Some(renewed_token()))).await;
        round_trip(&handle, &mut cm).await;
        assert_eq!(handle.refresh_token().expose_secret(), renewed_token());
    }

    #[tokio::test(start_paused = true)]
    async fn renewing_while_logging_on_again_fails_and_keeps_the_token() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        assert!(matches!(soon(handle.renew_tokens()).await, Err(ClientError::Dropped)));
        let logon = relogs(&mut reconnected(&mut reconnects).await).await;
        assert_eq!(logon.access_token(), refresh_token_jwt());
    }

    #[tokio::test(start_paused = true)]
    async fn renewing_asks_with_the_latest_token() {
        let Spawned { mut cm, handle, .. } = spawned_as(user()).await;
        let steam = async {
            for (sent, refresh_token) in [(refresh_token_jwt(), Some(renewed_token())), (renewed_token(), None)] {
                let (call, request) =
                    soon(cm.expect::<CAuthentication_AccessToken_GenerateForApp_Request>(AUTHED)).await;
                assert_eq!(request.refresh_token(), sent);
                cm.reply(&call, EResult::OK, tokens(refresh_token)).await;
            }
        };
        let renew = async {
            soon(handle.renew_tokens()).await.unwrap();
            // kept when Steam doesn't replace it
            assert!(soon(handle.renew_tokens()).await.unwrap().refresh_token.is_none());
        };
        tokio::join!(renew, steam);
        assert_eq!(handle.refresh_token().expose_secret(), renewed_token());
    }

    #[tokio::test(start_paused = true)]
    async fn logs_on_again_with_the_account_as_it_is_once_connected() {
        // a token kept while connecting, as a late renewal's
        let handles: Arc<Mutex<Option<SteamHandle<User>>>> = Arc::default();
        let (sender, mut reconnects) = mpsc::unbounded_channel();
        let renewing = handles.clone();
        let connect = connector(move |_| {
            let sender = sender.clone();
            let handle = renewing.lock().unwrap().take();
            Box::pin(async move {
                if let Some(handle) = handle {
                    handle
                        .shared
                        .state()
                        .account
                        .set_refresh_token(RefreshToken::new(renewed_token()).unwrap());
                }
                let (client, server) = duplex(64 * 1024);
                let _ = sender.send(FakeCm::new(server, key()));
                Ok((
                    Connection::new(client, CmProtocol::encrypted(key())),
                    "127.0.0.1:27018".to_string(),
                ))
            })
        });
        let (connection, cm) = connection();
        let (handle, mut events, _task, cm) = spawn_on(connection, cm, user(), connect).await;
        *handles.lock().unwrap() = Some(handle.clone());
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let logon = relogs(&mut soon(reconnects.recv()).await.unwrap()).await;
        assert_eq!(logon.access_token(), renewed_token());
    }

    #[tokio::test(start_paused = true)]
    async fn a_user_sets_its_persona_state_and_again_after_logging_on_again() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        handle.set_persona_state(EPersonaState::Online).unwrap();
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Online as u32);
        handle.clone().set_persona_state(EPersonaState::Away).unwrap();
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Away as u32);

        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        assert_eq!(relogs(&mut cm).await.chat_mode(), 2);
        // the latest, once
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Away as u32);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_persona_state_set_while_logging_on_again_is_set_once_logged_on() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        handle.set_persona_state(EPersonaState::Busy).unwrap();

        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Busy as u32);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        // and only once: the call comes next
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_persona_state_set_before_spawning_is_set_again_after_logging_on_again() {
        let mut account = user();
        // as `SteamClient::set_persona_state` keeps it
        account.set_persona_state(EPersonaState::LookingToTrade);
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(account).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert_eq!(persona_state(&mut cm).await, EPersonaState::LookingToTrade as u32);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn no_persona_state_is_set_unless_one_was() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        // the call is the first thing sent
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn setting_the_persona_state_fails_once_stopped() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned_as(user()).await;
        cm.send(logged_off(EResult::LogonSessionReplaced)).await;
        assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(_))));
        assert!(recv(&mut events).await.is_none());
        assert!(matches!(
            handle.set_persona_state(EPersonaState::Online),
            Err(ClientError::Closed)
        ));
    }

    /// Reads the games the client sets as played next, with the session on the header.
    async fn games(cm: &mut FakeCm) -> Vec<u64> {
        let games = next(cm).await;
        assert_eq!(games.emsg, EMsg::ClientGamesPlayedWithDataBlob);
        assert_eq!(games.header.steamid(), ANONYMOUS_STEAM_ID);
        assert!(games.header.has_client_sessionid());
        let body = games.body::<CMsgClientGamesPlayed>();
        body.games_played.iter().map(|game| game.game_id()).collect()
    }

    /// A message from CS2's GC, of type `msg_type`, its body a hello of `version`.
    fn from_gc(msg_type: u32, version: u32) -> Vec<u8> {
        let mut hello = CMsgClientHello::new();
        hello.set_protocol_version(version);
        let payload = gc::tests::proto_payload(msg_type, &CMsgProtoBufHeader::new(), &hello.write_to_bytes().unwrap());
        gc::tests::from_gc(CS2, msg_type | 0x8000_0000, payload)
    }

    fn gc_version(event: &GcEvent) -> Option<u32> {
        match event {
            GcEvent::Message(message) => Some(message.parse::<CMsgClientHello>().unwrap().protocol_version()),
            _ => None,
        }
    }

    async fn gc_recv(channel: &mut GcChannel) -> Option<GcEvent> {
        soon(channel.recv()).await
    }

    /// Sends a hello to CS2's GC through `channel`, and checks Steam got it, routed: then the task took every command
    /// before it, such as the channel's route.
    async fn greets(channel: &GcChannel, cm: &mut FakeCm) {
        let mut hello = CMsgClientHello::new();
        hello.set_protocol_version(1);
        channel.send(4006, &hello).unwrap();
        let sent = next(cm).await;
        assert_eq!(sent.emsg, EMsg::ClientToGC);
        assert_eq!(sent.header.routing_appid(), CS2);
        assert_eq!(sent.header.steamid(), ANONYMOUS_STEAM_ID);
        let envelope = sent.body::<CMsgGCClient>();
        assert_eq!(envelope.appid(), CS2);
        assert_eq!(envelope.msgtype(), 4006 | 0x8000_0000);
        let body = gc::gc_body(envelope.msgtype(), envelope.payload.unwrap()).unwrap();
        assert_eq!(CMsgClientHello::parse_from_tokio_bytes(&body).unwrap(), hello);
    }

    const CS2: u32 = 730;

    #[tokio::test(start_paused = true)]
    async fn a_game_coordinators_messages_go_to_its_channel() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned_as(user()).await;
        let mut channel = handle.game_coordinator(CS2).unwrap();
        assert_eq!(channel.app_id(), CS2);
        greets(&channel, &mut cm).await;

        cm.send(from_gc(4004, 1)).await;
        let event = gc_recv(&mut channel).await.unwrap();
        assert_eq!(gc_version(&event), Some(1));
        match event {
            GcEvent::Message(message) => assert_eq!((message.app_id(), message.msg_type()), (CS2, 4004)),
            event => panic!("expected a message, got {:?}", event),
        }
        // another app's are events
        let payload = gc::tests::proto_payload(4004, &CMsgProtoBufHeader::new(), b"");
        let other = gc::tests::from_gc(570, 4004 | 0x8000_0000, payload);
        cm.send(other).await;
        assert!(
            matches!(recv(&mut events).await, Some(Event::Message(message)) if message.emsg() == EMsg::ClientFromGC)
        );
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_channels_messages_are_events_again() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned_as(user()).await;
        let channel = handle.game_coordinator(CS2).unwrap();
        greets(&channel, &mut cm).await;
        drop(channel);

        cm.send(from_gc(4004, 3)).await;
        let event = recv(&mut events).await.unwrap();
        let Event::Message(message) = event else {
            panic!("expected a message, got {:?}", event);
        };
        let message = message.gc_message().unwrap().unwrap();
        assert_eq!(message.parse::<CMsgClientHello>().unwrap().protocol_version(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_channel_takes_the_apps_messages() {
        let Spawned { mut cm, handle, .. } = spawned_as(user()).await;
        let mut old = handle.game_coordinator(CS2).unwrap();
        let mut new = handle.game_coordinator(CS2).unwrap();
        greets(&new, &mut cm).await;
        assert!(gc_recv(&mut old).await.is_none());
        assert!(matches!(
            old.send(4006, &CMsgClientHello::new()),
            Err(ClientError::Closed)
        ));
        cm.send(from_gc(4004, 4)).await;
        assert_eq!(gc_version(&gc_recv(&mut new).await.unwrap()), Some(4));
    }

    #[tokio::test(start_paused = true)]
    async fn a_channel_not_read_drops_the_newest_and_counts_them_and_calls_are_still_answered() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned_as(user()).await;
        let mut channel = handle.game_coordinator(CS2).unwrap();
        greets(&channel, &mut cm).await;
        let sent = gc::GC_CAPACITY as u32 + 5;
        for chunk in (0..sent).collect::<Vec<_>>().chunks(50) {
            for version in chunk {
                cm.send(from_gc(4004, *version)).await;
            }
            round_trip(&handle, &mut cm).await;
        }

        for version in 0..gc::GC_CAPACITY as u32 {
            assert_eq!(gc_version(&gc_recv(&mut channel).await.unwrap()), Some(version));
        }
        assert!(matches!(gc_recv(&mut channel).await, Some(GcEvent::Lagged(5))));
        cm.send(from_gc(4004, 1000)).await;
        assert_eq!(gc_version(&gc_recv(&mut channel).await.unwrap()), Some(1000));
        // none went to the events
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test(start_paused = true)]
    async fn channels_lose_their_session_on_a_reconnect_and_keep_their_route() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        let mut channel = handle.game_coordinator(CS2).unwrap();
        greets(&channel, &mut cm).await;
        cm.send(from_gc(4004, 1)).await;
        assert_eq!(gc_version(&gc_recv(&mut channel).await.unwrap()), Some(1));

        drop(cm);
        assert!(matches!(gc_recv(&mut channel).await, Some(GcEvent::SessionLost)));
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        // dropped while connecting again: the call is the first thing sent
        channel.send(4006, &CMsgClientHello::new()).unwrap();
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        round_trip(&handle, &mut cm).await;

        // the route is kept
        greets(&channel, &mut cm).await;
        cm.send(from_gc(4004, 2)).await;
        assert_eq!(gc_version(&gc_recv(&mut channel).await.unwrap()), Some(2));
    }

    #[tokio::test(start_paused = true)]
    async fn a_channel_opened_while_logging_on_again_gets_the_new_sessions_messages() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut channel = handle.game_coordinator(CS2).unwrap();
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));

        cm.send(from_gc(4004, 5)).await;
        assert_eq!(gc_version(&gc_recv(&mut channel).await.unwrap()), Some(5));
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test(start_paused = true)]
    async fn channels_end_once_the_client_stopped() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            ..
        } = spawned_as(user()).await;
        let mut channel = handle.game_coordinator(CS2).unwrap();
        greets(&channel, &mut cm).await;
        cm.send(logged_off(EResult::LogonSessionReplaced)).await;
        assert!(matches!(recv(&mut events).await, Some(Event::LoggedOff(_))));
        assert!(gc_recv(&mut channel).await.is_none());
        assert!(matches!(
            channel.send(4006, &CMsgClientHello::new()),
            Err(ClientError::Closed)
        ));
        assert!(matches!(handle.game_coordinator(CS2), Err(ClientError::Closed)));
        assert!(matches!(handle.set_games_played(&[CS2]), Err(ClientError::Closed)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_channel_doesnt_keep_the_client_running() {
        let Spawned {
            mut cm, handle, task, ..
        } = spawned_as(user()).await;
        let mut channel = handle.game_coordinator(CS2).unwrap();
        greets(&channel, &mut cm).await;
        drop(handle);
        assert_eq!(next(&mut cm).await.emsg, EMsg::ClientLogOff);
        assert_eq!(ends(task, SHUTDOWN_TIMEOUT).await, Stopped::HandlesDropped);
        assert!(gc_recv(&mut channel).await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn a_user_sets_its_games_played_and_again_after_logging_on_again() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        handle.set_games_played(&[CS2]).unwrap();
        assert_eq!(games(&mut cm).await, [730]);
        handle.set_persona_state(EPersonaState::Online).unwrap();
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Online as u32);
        handle.clone().set_games_played(&[CS2, 440]).unwrap();
        assert_eq!(games(&mut cm).await, [730, 440]);

        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        // the persona, then the latest games, once, before anything else
        assert_eq!(persona_state(&mut cm).await, EPersonaState::Online as u32);
        assert_eq!(games(&mut cm).await, [730, 440]);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn games_played_set_while_logging_on_again_are_set_once_logged_on() {
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        handle.set_games_played(&[440]).unwrap();
        handle.set_games_played(&[CS2]).unwrap();

        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert_eq!(games(&mut cm).await, [730]);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        // and only once: the call comes next
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn games_played_set_before_spawning_are_set_again_after_logging_on_again() {
        let mut account = user();
        // as `SteamClient::set_games_played` keeps them
        account.set_games_played(&[CS2]);
        let Spawned {
            cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(account).await;
        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert_eq!(games(&mut cm).await, [730]);
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn playing_nothing_isnt_set_again() {
        let Spawned {
            mut cm,
            handle,
            mut events,
            mut reconnects,
            ..
        } = spawned_as(user()).await;
        handle.set_games_played(&[CS2]).unwrap();
        assert_eq!(games(&mut cm).await, [730]);
        handle.set_games_played(&[]).unwrap();
        assert!(games(&mut cm).await.is_empty());

        drop(cm);
        assert!(disconnected(DisconnectReason::ConnectionLost)(&recv(&mut events).await));
        let mut cm = reconnected(&mut reconnects).await;
        relogs(&mut cm).await;
        assert!(matches!(recv(&mut events).await, Some(Event::Reconnected)));
        // a new session plays nothing anyway: the call is the first thing sent
        round_trip(&handle, &mut cm).await;
    }

    #[tokio::test(start_paused = true)]
    async fn calls_wait_unsent_while_the_connection_takes_nothing_and_reads_go_on() {
        // Steam reads nothing: 1 KiB goes, then writes block
        let (client, server) = duplex(1024);
        let connection = Connection::new(client, CmProtocol::encrypted(key()));
        let connect = connector(|_| Box::pin(async { Err(ClientError::NoServerAvailable) }));
        let (handle, mut events, _task, mut cm) =
            spawn_on(connection, FakeCm::new(server, key()), Anonymous, connect).await;

        let size = 2000;
        let count = 2 * MAX_UNSENT / size;
        let callers: Vec<_> = (0..count)
            .map(|index| {
                let handle = handle.clone();
                tokio::spawn(async move { call(&handle, &format!("{:0>1$}", index, size)).await })
            })
            .collect();
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        // what Steam sends still comes, while the client's writes are blocked
        cm.send(hello(7)).await;
        assert_eq!(version(&recv(&mut events).await.unwrap()), Some(7));

        // the callers go: the calls not sent yet are never sent
        for caller in &callers {
            caller.abort();
        }
        let steam = async {
            let mut sent = 0;
            loop {
                let call = next(&mut cm).await;
                let name = call
                    .body::<CAuthentication_GetPasswordRSAPublicKey_Request>()
                    .account_name()
                    .to_string();
                if name == "sync" {
                    answer(&mut cm, &call).await;
                    return sent;
                }
                sent += 1;
            }
        };
        let (synced, sent) = tokio::join!(call(&handle, "sync"), steam);
        synced.unwrap();
        assert!(sent < count, "{} of {} calls sent", sent, count);
        assert!(sent >= MAX_UNSENT / (size + 100), "{} calls sent", sent);
    }

    #[tokio::test(start_paused = true)]
    async fn calls_that_wait_unsent_time_out() {
        let (client, server) = duplex(1024);
        let connection = Connection::new(client, CmProtocol::encrypted(key()));
        let connect = connector(|_| Box::pin(async { Err(ClientError::NoServerAvailable) }));
        let (handle, _events, _task, _cm) = spawn_on(connection, FakeCm::new(server, key()), Anonymous, connect).await;
        // more than the connection takes before the next one waits, whatever the pipe holds
        let name = "x".repeat(2 * MAX_UNSENT);
        let start = Instant::now();
        let (first, second) = tokio::join!(call(&handle, &name), call(&handle, "waits"));
        assert!(matches!(first, Err(ClientError::Timeout(_))));
        assert!(matches!(second, Err(ClientError::Timeout(_))));
        assert_eq!(start.elapsed(), RESPONSE_TIMEOUT);
    }
}

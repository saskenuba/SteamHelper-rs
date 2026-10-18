//! A session with CS2's game coordinator, to inspect items.
//!
//! As node-globaloffensive (v3.3.0) does: play CS2, send `ClientHello` until the game coordinator answers
//! `ClientWelcome`, then ask `Client2GCEconPreviewDataBlockRequest` for each inspect link and wait for the
//! `Client2GCEconPreviewDataBlockResponse` of the same item.

use std::convert::TryFrom;
use std::time::Duration;

use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_ClientLogonFatalError;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::ECsgoGCMsg;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientHello;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgConnectionStatus;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::GCConnectionStatus;
use steam_protobuf::protobufs::csgo::gcsystemmsgs::EGCBaseClientMsg;
use tokio::time::sleep_until;
use tokio::time::Instant;

use crate::client::User;
use crate::errors::ClientError;
use crate::gc::cs2::InspectLink;
use crate::gc::cs2::ItemDetails;
use crate::gc::cs2::ItemDetailsError;
use crate::gc::GcChannel;
use crate::gc::GcEvent;
use crate::gc::GcMessage;
use crate::handle::SteamHandle;
use crate::handle::TryRecvError;

/// CS2's app id.
pub const APP_ID: u32 = 730;
/// How long [`Cs2::connect`] (and an inspect after the session was lost) waits for the game coordinator's welcome.
pub const GREET_TIMEOUT: Duration = Duration::from_secs(60);
/// How long an inspect waits for the game coordinator's answer by default, as node-globaloffensive's `inspectItem`.
pub const INSPECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The time between two requests by default, from one sent to the next, as csfloat's inspect service waits
/// (`request_delay`): the game coordinator drops requests that come faster, without a word.
pub const INSPECT_COOLDOWN: Duration = Duration::from_millis(1100);

/// The first hello waits this long, as node-globaloffensive's: Steam has just been told the account plays CS2.
const FIRST_HELLO: Duration = Duration::from_millis(500);
/// The time until the second hello, doubled after each one, up to [`MAX_HELLO_INTERVAL`].
const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const MAX_HELLO_INTERVAL: Duration = Duration::from_secs(60);
/// The version of the game node-globaloffensive (v3.3.0) says it is, in its hello.
const CLIENT_VERSION: u32 = 2000244;

const CLIENT_WELCOME: u32 = EGCBaseClientMsg::k_EMsgGCClientWelcome as u32;
const CLIENT_HELLO: u32 = EGCBaseClientMsg::k_EMsgGCClientHello as u32;
const CONNECTION_STATUS: u32 = EGCBaseClientMsg::k_EMsgGCClientConnectionStatus as u32;
const LOGON_FATAL_ERROR: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_ClientLogonFatalError as u32;
const PREVIEW_REQUEST: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest as u32;
const PREVIEW_RESPONSE: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse as u32;

/// A session with CS2's game coordinator, over a [spawned](crate::client::SteamClient::spawn) client: inspects
/// items, one at a time.
///
/// ```no_run
/// # async fn run(handle: steam_client::handle::SteamHandle<steam_client::client::User>)
/// # -> Result<(), steam_client::errors::ClientError> {
/// use steam_client::gc::cs2::Cs2;
///
/// let mut cs2 = Cs2::connect(&handle).await?;
/// let link = concat!(
///     "steam://rungame/730/76561202255233023/+csgo_econ_action_preview",
///     "%20S76561198084749846A698323590D7935523998312483177",
/// );
/// let item = cs2.inspect_str(link).await?;
/// println!("pattern {:?}, float {:?}", item.pattern(), item.float());
/// # Ok(())
/// # }
/// ```
///
/// The account must own CS2 (the free license). Each inspect waits [`INSPECT_COOLDOWN`] after the request before it,
/// and gives up after [`INSPECT_TIMEOUT`]: change them with [`with_cooldown`](Self::with_cooldown) and
/// [`with_timeout`](Self::with_timeout). One request at a time, per account: more accounts inspect more.
///
/// When the session with the game coordinator ends (the client lost its connection to Steam, or the game
/// coordinator says it has no session for the account), the next inspect greets it again before asking. Nothing
/// runs in the background: the session's messages are read while inspecting. Like a [`GcChannel`], it doesn't keep
/// the client running.
#[derive(Debug)]
pub struct Cs2 {
    channel: GcChannel,
    /// Welcomed, and no word since that the session ended.
    ready: bool,
    cooldown: Duration,
    timeout: Duration,
    /// When the latest request was sent, which the cooldown counts from.
    last_request: Option<Instant>,
}

impl Cs2 {
    /// Plays CS2 on `handle`'s account (the only game it then plays, also after reconnects), and greets the game
    /// coordinator until it welcomes the account: a hello after half a second, then after 2 s, 4 s… up to a minute
    /// apart, as node-globaloffensive does.
    ///
    /// Takes CS2's game coordinator's messages from any earlier [`GcChannel`] of the app, or `Cs2`.
    ///
    /// Fails with [`ClientError::GcTimeout`] if no welcome came within [`GREET_TIMEOUT`],
    /// [`ClientError::GcRefused`] if the game coordinator refused the session, and [`ClientError::Closed`] once the
    /// client stopped.
    pub async fn connect(handle: &SteamHandle<User>) -> Result<Self, ClientError> {
        handle.set_games_played(&[APP_ID])?;
        let mut cs2 = Self::new(handle.game_coordinator(APP_ID)?);
        cs2.greet().await?;
        Ok(cs2)
    }

    fn new(channel: GcChannel) -> Self {
        Self {
            channel,
            ready: false,
            cooldown: INSPECT_COOLDOWN,
            timeout: INSPECT_TIMEOUT,
            last_request: None,
        }
    }

    /// Waits `cooldown` between two requests, from one sent to the next, instead of [`INSPECT_COOLDOWN`].
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Waits `timeout` for each answer, instead of [`INSPECT_TIMEOUT`].
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Parses `link` as an [`InspectLink`] and inspects it.
    pub async fn inspect_str(&mut self, link: &str) -> Result<ItemDetails, ClientError> {
        self.inspect(&link.parse()?).await
    }

    /// The details of the item `link` names, from the game coordinator, or from the link when it embeds them (then
    /// offline, at once).
    ///
    /// First waits for the cooldown, and greets the game coordinator again if the session ended. Then asks, and
    /// waits for the answer about the same asset id: answers about other items, such as a late one to an earlier
    /// request, are skipped. Dropping the future forgets the request; its answer, if it comes, is skipped then.
    ///
    /// Fails with [`ClientError::GcTimeout`] if no answer came within the timeout (the game coordinator doesn't
    /// answer about items it can't find, nor requests that come too fast), [`ClientError::ItemDetails`] if the answer
    /// has no item or one without a def index, and as [`connect`](Self::connect) when greeting.
    pub async fn inspect(&mut self, link: &InspectLink) -> Result<ItemDetails, ClientError> {
        let params = match link.params() {
            Ok(params) => params,
            Err(item) => return Ok(item.details()?),
        };
        if let Some(last) = self.last_request {
            sleep_until(last + self.cooldown).await;
        }
        self.read_waiting()?;
        if !self.ready {
            self.greet().await?;
        }

        let mut request = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest::new();
        request.set_param_s(params.s);
        request.set_param_a(params.a);
        request.set_param_d(params.d);
        request.set_param_m(params.m);
        self.channel.send(PREVIEW_REQUEST, &request)?;
        let sent = Instant::now();
        self.last_request = Some(sent);

        let deadline = sent + self.timeout;
        loop {
            let event = tokio::select! {
                event = self.channel.recv() => event,
                () = sleep_until(deadline) => return Err(ClientError::GcTimeout(PREVIEW_RESPONSE)),
            };
            let Some(message) = self.track(event)? else {
                continue;
            };
            if message.msg_type() != PREVIEW_RESPONSE {
                continue;
            }
            let response = message.parse::<CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse>()?;
            let Some(item) = response.iteminfo.into_option() else {
                return Err(ItemDetailsError::NoItem.into());
            };
            if item.itemid() != params.a {
                debug!("Skipping the game coordinator's answer about another item");
                continue;
            }
            return Ok(ItemDetails::try_from(item)?);
        }
    }

    /// Sends hellos until the game coordinator welcomes the account.
    async fn greet(&mut self) -> Result<(), ClientError> {
        let start = Instant::now();
        let deadline = start + GREET_TIMEOUT;
        let mut next_hello = start + FIRST_HELLO;
        let mut interval = HELLO_INTERVAL;
        while !self.ready {
            tokio::select! {
                event = self.channel.recv() => {
                    self.track(event)?;
                }
                () = sleep_until(next_hello) => {
                    debug!("Greeting CS2's game coordinator, again in {:?}", interval);
                    self.channel.send(CLIENT_HELLO, &hello())?;
                    next_hello = Instant::now() + interval;
                    interval = (interval * 2).min(MAX_HELLO_INTERVAL);
                }
                () = sleep_until(deadline) => return Err(ClientError::GcTimeout(CLIENT_WELCOME)),
            }
        }
        Ok(())
    }

    /// Reads what came since the last inspect, to know whether the session is still on.
    fn read_waiting(&mut self) -> Result<(), ClientError> {
        loop {
            match self.channel.try_recv() {
                Ok(event) => {
                    self.track(Some(event))?;
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Stopped) => return Err(ClientError::Closed),
            }
        }
    }

    /// Follows the session through `event`, from the channel. Gives back the messages that aren't about the session.
    fn track(&mut self, event: Option<GcEvent>) -> Result<Option<GcMessage>, ClientError> {
        let message = match event.ok_or(ClientError::Closed)? {
            GcEvent::Message(message) => message,
            GcEvent::SessionLost => {
                debug!("Lost the session with CS2's game coordinator: the connection to Steam was lost");
                self.ready = false;
                return Ok(None);
            }
            // an answer may have been dropped; it may also still come
            GcEvent::Lagged(missed) => {
                warn!("Missed {} messages of CS2's game coordinator", missed);
                return Ok(None);
            }
        };
        match message.msg_type() {
            CLIENT_WELCOME => {
                debug!("CS2's game coordinator welcomed the account");
                self.ready = true;
            }
            CONNECTION_STATUS => match message.parse::<CMsgConnectionStatus>() {
                // the raw value, as node-globaloffensive compares it: one unknown here isn't a session
                Ok(status) => {
                    let status = status.status.map_or(0, |status| status.value());
                    if status != GCConnectionStatus::GCConnectionStatus_HAVE_SESSION as i32 {
                        debug!("CS2's game coordinator has no session for the account (status {})", status);
                        self.ready = false;
                    }
                }
                Err(error) => warn!("Unreadable connection status from CS2's game coordinator: {}", error),
            },
            LOGON_FATAL_ERROR => {
                self.ready = false;
                let error = message.parse::<CMsgGCCStrike15_v2_ClientLogonFatalError>()?;
                return Err(ClientError::GcRefused {
                    code: error.errorcode(),
                    message: error.message().to_string(),
                });
            }
            _ => return Ok(Some(message)),
        }
        Ok(None)
    }
}

/// The hello node-globaloffensive (v3.3.0) sends.
fn hello() -> CMsgClientHello {
    let mut hello = CMsgClientHello::new();
    hello.set_version(CLIENT_VERSION);
    hello.set_client_session_need(0);
    hello.set_client_launcher(0);
    hello.set_steam_launcher(0);
    hello
}

#[cfg(test)]
mod tests;

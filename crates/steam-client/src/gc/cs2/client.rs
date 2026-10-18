//! A session with CS2's game coordinator: inspects items, asks for profiles and matches, keeps the inventory, and
//! passes on what else the game coordinator says.
//!
//! As node-globaloffensive (v3.3.0) does: play CS2, send `ClientHello` until the game coordinator answers
//! `ClientWelcome`, then send each request and wait for its answer, told from others by what it's about.

use std::collections::VecDeque;
use std::convert::TryFrom;
use std::mem;
use std::time::Duration;

use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_ClientLogonFatalError;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_ClientRequestPlayersProfile;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchList;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchListRequestCurrentLiveGames;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchListRequestFullGameInfo;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchListRequestLiveGameForUser;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchListRequestRecentUserGames;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchmakingClient2GCHello;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchmakingGC2ClientHello;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_PlayersProfile;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::ECsgoGCMsg;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientHello;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientWelcome;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgConnectionStatus;
use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::GCConnectionStatus;
use steam_protobuf::protobufs::csgo::gcsystemmsgs::EGCBaseClientMsg;
use steamid_parser::SteamID;
use tokio::time::sleep_until;
use tokio::time::Instant;

use crate::client::User;
use crate::errors::ClientError;
use crate::gc::cs2::inventory::Change;
use crate::gc::cs2::InspectLink;
use crate::gc::cs2::Inventory;
use crate::gc::cs2::InventoryItem;
use crate::gc::cs2::ItemDetails;
use crate::gc::cs2::ItemDetailsError;
use crate::gc::cs2::MatchList;
use crate::gc::cs2::PlayerProfile;
use crate::gc::cs2::ShareCode;
use crate::gc::GcChannel;
use crate::gc::GcEvent;
use crate::gc::GcMessage;
use crate::handle::SteamHandle;
use crate::handle::TryRecvError;
use crate::protocol::free_if_big;

/// CS2's app id.
pub const APP_ID: u32 = 730;
/// How long [`Cs2::connect`] (and a request after the session was lost) waits for the game coordinator's welcome.
pub const GREET_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a request of any kind waits for the game coordinator's answer by default, as node-globaloffensive's
/// `inspectItem` (its other requests wait forever).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// The time between two requests of any kind by default, from one sent to the next, as csfloat's inspect service
/// waits (`request_delay`): the game coordinator drops requests that come faster, without a word.
pub const REQUEST_COOLDOWN: Duration = Duration::from_millis(1100);
/// How many events wait for [`Cs2::next_event`]; past this, newer ones are dropped and a [`Cs2Event::Lagged`]
/// counts them.
pub const EVENT_CAPACITY: usize = 256;

/// The first hello waits this long, as node-globaloffensive's: Steam has just been told the account plays CS2.
const FIRST_HELLO: Duration = Duration::from_millis(500);
/// The time until the second hello, doubled after each one, up to [`MAX_HELLO_INTERVAL`].
const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const MAX_HELLO_INTERVAL: Duration = Duration::from_secs(60);
/// The version of the game node-globaloffensive (v3.3.0) says it is, in its hello.
const CLIENT_VERSION: u32 = 2000244;
/// What node-globaloffensive and ValvePython's csgo ask a profile with: what it is, nobody says.
const PROFILE_REQUEST_LEVEL: u32 = 32;

const CLIENT_WELCOME: u32 = EGCBaseClientMsg::k_EMsgGCClientWelcome as u32;
const CLIENT_HELLO: u32 = EGCBaseClientMsg::k_EMsgGCClientHello as u32;
const CONNECTION_STATUS: u32 = EGCBaseClientMsg::k_EMsgGCClientConnectionStatus as u32;
const LOGON_FATAL_ERROR: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_ClientLogonFatalError as u32;
const PREVIEW_REQUEST: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest as u32;
const PREVIEW_RESPONSE: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse as u32;
const MATCHMAKING_CLIENT_HELLO: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchmakingClient2GCHello as u32;
const MATCHMAKING_HELLO: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchmakingGC2ClientHello as u32;
const REQUEST_PLAYERS_PROFILE: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_ClientRequestPlayersProfile as u32;
const PLAYERS_PROFILE: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_PlayersProfile as u32;
const MATCH_LIST: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchList as u32;
const LIVE_GAMES: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchListRequestCurrentLiveGames as u32;
const RECENT_GAMES: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchListRequestRecentUserGames as u32;
const FULL_GAME_INFO: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchListRequestFullGameInfo as u32;
const LIVE_GAME_FOR_USER: u32 = ECsgoGCMsg::k_EMsgGCCStrike15_v2_MatchListRequestLiveGameForUser as u32;

/// A session with CS2's game coordinator, over a [spawned](crate::client::SteamClient::spawn) client: inspects
/// items, asks for profiles and matches, one request at a time, keeps the account's inventory, and passes on the
/// rest as [events](Self::next_event).
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
/// println!("{} items in the inventory", cs2.inventory().len());
/// # Ok(())
/// # }
/// ```
///
/// The account must own CS2 (the free license). Each request waits [`REQUEST_COOLDOWN`] after the request before it,
/// whatever its kind, and gives up after [`REQUEST_TIMEOUT`]: change them with [`with_cooldown`](Self::with_cooldown)
/// and [`with_timeout`](Self::with_timeout). One request at a time, per account: more accounts ask more.
///
/// When the session with the game coordinator ends (the client lost its connection to Steam, or the game
/// coordinator says it has no session for the account), the next request greets it again before asking, as
/// [`next_event`](Self::next_event) does while waiting. Nothing runs in the background: the session's messages are
/// read during requests and by [`next_event`](Self::next_event) and [`try_next_event`](Self::try_next_event); those
/// a request doesn't wait for become [events](Cs2Event), kept until read (up to [`EVENT_CAPACITY`]). Like a
/// [`GcChannel`], it doesn't keep the client running.
#[derive(Debug)]
pub struct Cs2 {
    channel: GcChannel,
    /// The account, whose inventory is kept.
    account: SteamID,
    session: Session,
    /// Welcomed once: the welcomes after the first are events.
    welcomed_before: bool,
    cooldown: Duration,
    timeout: Duration,
    /// When the latest request was sent, which the cooldown counts from.
    last_request: Option<Instant>,
    events: EventQueue,
    inventory: Inventory,
    matchmaking_stats: Option<PlayerProfile>,
}

/// Where the session with the game coordinator is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Session {
    /// Welcomed, and no word since that the session ended.
    Welcomed,
    /// No session, and no hello sent since it ended.
    Lost,
    /// Greeting: the next hello is due at `next_hello`, and the one after it `interval` later.
    Greeting { next_hello: Instant, interval: Duration },
}

/// What happens in a [`Cs2`] session that no request waits for, from [`Cs2::next_event`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Cs2Event {
    /// The session with the game coordinator ended: the client lost its connection to Steam, or the game
    /// coordinator says it has no session for the account. The next request greets it again, as `next_event` does
    /// while waiting. Never dropped.
    SessionLost,
    /// The game coordinator welcomed the account again, after the session was lost or unasked (it restarted). The
    /// inventory is the welcome's, if it sent one; changes in between came as no events. Never dropped.
    Welcomed,
    /// The account's own matchmaking stats (ranks, wins, level, cooldown…), which the game coordinator sends after
    /// its welcome and when they change. Also kept: [`Cs2::matchmaking_stats`].
    MatchmakingStats(PlayerProfile),
    /// The game coordinator sent the whole inventory again (`CacheSubscribed`). Changes in between came as no events.
    InventoryLoaded,
    /// An item came into the inventory, or out of a storage unit.
    ItemAcquired(InventoryItem),
    /// An item changed, such as its position, name or stickers.
    ItemChanged {
        /// As it was.
        old: InventoryItem,
        /// As it is.
        new: InventoryItem,
    },
    /// An item left the inventory, or went into a storage unit.
    ItemRemoved(InventoryItem),
    /// A message `Cs2` doesn't read, or an answer no request waits for (any more).
    Other(GcMessage),
    /// This many events, or messages from the game coordinator, were dropped because they weren't read in time. The
    /// inventory may have missed changes since.
    Lagged(u64),
}

impl Cs2 {
    /// Plays CS2 on `handle`'s account (the only game it then plays, also after reconnects), and greets the game
    /// coordinator until it welcomes the account: a hello after half a second, then after 2 s, 4 s… up to a minute
    /// apart, as node-globaloffensive does. The welcome carries the inventory.
    ///
    /// Takes CS2's game coordinator's messages from any earlier [`GcChannel`] of the app, or `Cs2`.
    ///
    /// Fails with [`ClientError::GcTimeout`] if no welcome came within [`GREET_TIMEOUT`],
    /// [`ClientError::GcRefused`] if the game coordinator refused the session, and [`ClientError::Closed`] once the
    /// client stopped.
    pub async fn connect(handle: &SteamHandle<User>) -> Result<Self, ClientError> {
        handle.set_games_played(&[APP_ID])?;
        let mut cs2 = Self::new(handle.game_coordinator(APP_ID)?, handle.steam_id());
        cs2.greet().await?;
        Ok(cs2)
    }

    fn new(channel: GcChannel, account: SteamID) -> Self {
        Self {
            channel,
            account,
            session: Session::Lost,
            welcomed_before: false,
            cooldown: REQUEST_COOLDOWN,
            timeout: REQUEST_TIMEOUT,
            last_request: None,
            events: EventQueue::default(),
            inventory: Inventory::default(),
            matchmaking_stats: None,
        }
    }

    /// Waits `cooldown` between two requests, from one sent to the next, instead of [`REQUEST_COOLDOWN`].
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Waits `timeout` for each answer, instead of [`REQUEST_TIMEOUT`].
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Whether the game coordinator welcomed the account, and said nothing since that the session ended, as far as
    /// the messages read tell.
    pub fn has_session(&self) -> bool {
        self.session == Session::Welcomed
    }

    /// The account's inventory, as the game coordinator last said, as far as the messages read tell.
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The account's own matchmaking stats the game coordinator last sent, if it did.
    pub fn matchmaking_stats(&self) -> Option<&PlayerProfile> {
        self.matchmaking_stats.as_ref()
    }

    /// Parses `link` as an [`InspectLink`] and inspects it.
    pub async fn inspect_str(&mut self, link: &str) -> Result<ItemDetails, ClientError> {
        self.inspect(&link.parse()?).await
    }

    /// The details of the item `link` names, from the game coordinator, or from the link when it embeds them (then
    /// offline, at once).
    ///
    /// First waits for the cooldown, and greets the game coordinator again if the session ended. Then asks
    /// (`Client2GCEconPreviewDataBlockRequest`, 9156), and waits for the answer (9157) about the same asset id:
    /// answers about other items, such as a late one to an earlier request, are passed on as events. Dropping the
    /// future forgets the request; its answer, if it comes, is an event then.
    ///
    /// Fails with [`ClientError::GcTimeout`] if no answer came within the timeout (the game coordinator doesn't
    /// answer about items it can't find, nor requests that come too fast), [`ClientError::ItemDetails`] if the answer
    /// has no item or one without a def index, and as [`connect`](Self::connect) when greeting.
    pub async fn inspect(&mut self, link: &InspectLink) -> Result<ItemDetails, ClientError> {
        let params = match link.params() {
            Ok(params) => params,
            Err(item) => return Ok(item.details()?),
        };
        let mut request = CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockRequest::new();
        request.set_param_s(params.s);
        request.set_param_a(params.a);
        request.set_param_d(params.d);
        request.set_param_m(params.m);
        self.request(PREVIEW_REQUEST, &request, PREVIEW_RESPONSE, |message| {
            let response = message.parse::<CMsgGCCStrike15_v2_Client2GCEconPreviewDataBlockResponse>()?;
            let Some(item) = response.iteminfo.into_option() else {
                return Err(ItemDetailsError::NoItem.into());
            };
            if item.itemid() != params.a {
                return Ok(None);
            }
            Ok(Some(ItemDetails::try_from(item)?))
        })
        .await
    }

    /// Asks the game coordinator for the account's matchmaking stats again (`MatchmakingClient2GCHello`, 9109), and
    /// waits for them (`MatchmakingGC2ClientHello`, 9110), as ValvePython's csgo `request_matchmaking_stats` does. They
    /// are kept: [`matchmaking_stats`](Self::matchmaking_stats).
    ///
    /// Fails as [`inspect`](Self::inspect) does.
    pub async fn request_matchmaking_stats(&mut self) -> Result<PlayerProfile, ClientError> {
        let request = CMsgGCCStrike15_v2_MatchmakingClient2GCHello::new();
        let stats = self
            .request(MATCHMAKING_CLIENT_HELLO, &request, MATCHMAKING_HELLO, |message| {
                Ok(Some(PlayerProfile::new(message.parse()?)))
            })
            .await?;
        self.matchmaking_stats = Some(stats.clone());
        Ok(stats)
    }

    /// The CS2 profile of `player` (`ClientRequestPlayersProfile`, 9127): ranks, wins, level, commendations. Waits for
    /// the answer (`PlayersProfile`, 9128) whose first profile is `player`'s, as node-globaloffensive matches it. The
    /// game coordinator only answers about players who play CS2 and are friends of the account, node says.
    ///
    /// Fails with [`ClientError::NotIndividual`] if `player` isn't an individual account, and as
    /// [`inspect`](Self::inspect) does.
    pub async fn player_profile(&mut self, player: SteamID) -> Result<PlayerProfile, ClientError> {
        let account_id = individual(player)?;
        let mut request = CMsgGCCStrike15_v2_ClientRequestPlayersProfile::new();
        request.set_account_id(account_id);
        request.set_request_level(PROFILE_REQUEST_LEVEL);
        self.request(REQUEST_PLAYERS_PROFILE, &request, PLAYERS_PROFILE, |message| {
            let mut profiles = message.parse::<CMsgGCCStrike15_v2_PlayersProfile>()?;
            match profiles.account_profiles.first() {
                Some(profile) if profile.account_id() == account_id => {
                    Ok(Some(PlayerProfile::new(profiles.account_profiles.swap_remove(0))))
                }
                _ => Ok(None),
            }
        })
        .await
    }

    /// The details of the match `code` names (`MatchListRequestFullGameInfo`, 9147): its rounds, score, demo… Waits
    /// for the match list (9139) that answers it: the request's type in `msgrequestid`, as ValvePython's csgo tells
    /// them, and the match, unless the game coordinator found none.
    ///
    /// Fails as [`inspect`](Self::inspect) does.
    pub async fn match_info(&mut self, code: &ShareCode) -> Result<MatchList, ClientError> {
        let mut request = CMsgGCCStrike15_v2_MatchListRequestFullGameInfo::new();
        request.set_matchid(code.match_id());
        request.set_outcomeid(code.outcome_id());
        request.set_token(u32::from(code.token()));
        let match_id = code.match_id();
        self.match_list(FULL_GAME_INFO, &request, |list| {
            list.matches.is_empty() || list.matches.iter().any(|info| info.matchid() == match_id)
        })
        .await
    }

    /// The recent matches of `player` (`MatchListRequestRecentUserGames`, 9141), as the game's "Your Matches" lists
    /// them. Waits for the match list (9139) that answers it, as [`match_info`](Self::match_info) does.
    ///
    /// Fails as [`player_profile`](Self::player_profile) does.
    pub async fn recent_matches(&mut self, player: SteamID) -> Result<MatchList, ClientError> {
        let account_id = individual(player)?;
        let mut request = CMsgGCCStrike15_v2_MatchListRequestRecentUserGames::new();
        request.set_accountid(account_id);
        self.match_list(RECENT_GAMES, &request, |list| {
            list.accountid.is_none_or(|id| id == account_id)
        })
        .await
    }

    /// The match `player` is playing (`MatchListRequestLiveGameForUser`, 9154). ValvePython's csgo says the game
    /// coordinator doesn't answer it any more: then this fails with [`ClientError::GcTimeout`].
    ///
    /// Fails as [`player_profile`](Self::player_profile) does.
    pub async fn live_match(&mut self, player: SteamID) -> Result<MatchList, ClientError> {
        let account_id = individual(player)?;
        let mut request = CMsgGCCStrike15_v2_MatchListRequestLiveGameForUser::new();
        request.set_accountid(account_id);
        self.match_list(LIVE_GAME_FOR_USER, &request, |list| {
            list.accountid.is_none_or(|id| id == account_id)
        })
        .await
    }

    /// The tournament matches being played (`MatchListRequestCurrentLiveGames`, 9140), as the game's "Watch" lists
    /// them.
    ///
    /// Fails as [`inspect`](Self::inspect) does.
    pub async fn live_matches(&mut self) -> Result<MatchList, ClientError> {
        let request = CMsgGCCStrike15_v2_MatchListRequestCurrentLiveGames::new();
        self.match_list(LIVE_GAMES, &request, |_| true).await
    }

    /// The next [event](Cs2Event): one kept, or the next one the game coordinator causes. While the session is lost,
    /// greets the game coordinator as [`connect`](Self::connect) does, without giving up, until it welcomes the
    /// account ([`Cs2Event::Welcomed`]). Cancel-safe: it can wait in a `select!`.
    ///
    /// Fails with [`ClientError::Closed`] once the client stopped and every event was read, and with
    /// [`ClientError::GcRefused`] if the game coordinator refused the session (the next call greets it again).
    ///
    /// ```no_run
    /// # async fn run(mut cs2: steam_client::gc::cs2::Cs2)
    /// # -> Result<(), steam_client::errors::ClientError> {
    /// use steam_client::gc::cs2::Cs2Event;
    ///
    /// loop {
    ///     match cs2.next_event().await? {
    ///         Cs2Event::ItemAcquired(item) => println!("Acquired {}", item.id()),
    ///         Cs2Event::ItemChanged { new, .. } => println!("Changed {}", new.id()),
    ///         Cs2Event::ItemRemoved(item) => println!("Removed {}", item.id()),
    ///         Cs2Event::MatchmakingStats(stats) => println!("Level {:?}", stats.level()),
    ///         Cs2Event::SessionLost => println!("Lost the session, greeting again"),
    ///         Cs2Event::Welcomed => println!("Welcomed again: {} items", cs2.inventory().len()),
    ///         Cs2Event::Lagged(missed) => println!("Missed {} events", missed),
    ///         _ => {}
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn next_event(&mut self) -> Result<Cs2Event, ClientError> {
        loop {
            if let Some(event) = self.events.pop() {
                return Ok(event);
            }
            match self.hello_due() {
                Some(next_hello) => tokio::select! {
                    event = self.channel.recv() => self.handle(event)?,
                    () = sleep_until(next_hello) => self.send_hello()?,
                },
                None => {
                    let event = self.channel.recv().await;
                    self.handle(event)?;
                }
            }
        }
    }

    /// The next [event](Cs2Event) if one waits, without waiting nor greeting.
    ///
    /// Fails as [`next_event`](Self::next_event) does.
    pub fn try_next_event(&mut self) -> Result<Option<Cs2Event>, ClientError> {
        loop {
            if let Some(event) = self.events.pop() {
                return Ok(Some(event));
            }
            match self.channel.try_recv() {
                Ok(event) => self.handle(Some(event))?,
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Stopped) => return Err(ClientError::Closed),
            }
        }
    }

    /// Asks for a match list with `body`, a request of type `msg_type`, and waits for the one about it.
    async fn match_list<M: Message>(
        &mut self,
        msg_type: u32,
        body: &M,
        about: impl Fn(&CMsgGCCStrike15_v2_MatchList) -> bool,
    ) -> Result<MatchList, ClientError> {
        self.request(msg_type, body, MATCH_LIST, |message| {
            let list = message.parse::<CMsgGCCStrike15_v2_MatchList>()?;
            Ok((list.msgrequestid() == msg_type && about(&list)).then(|| MatchList::new(list)))
        })
        .await
    }

    /// Sends `body`, a request of type `msg_type`, once the cooldown passed and the game coordinator welcomed the
    /// account, then waits for its answer: the first message of type `answer_type` that `answer` reads as one. The
    /// others are kept as events.
    async fn request<M: Message, T>(
        &mut self,
        msg_type: u32,
        body: &M,
        answer_type: u32,
        mut answer: impl FnMut(&GcMessage) -> Result<Option<T>, ClientError>,
    ) -> Result<T, ClientError> {
        if let Some(last) = self.last_request {
            sleep_until(last + self.cooldown).await;
        }
        self.read_waiting()?;
        if !self.has_session() {
            self.greet().await?;
        }

        self.channel.send(msg_type, body)?;
        let sent = Instant::now();
        self.last_request = Some(sent);

        let deadline = sent + self.timeout;
        loop {
            let event = tokio::select! {
                event = self.channel.recv() => event,
                () = sleep_until(deadline) => return Err(ClientError::GcTimeout(answer_type)),
            };
            if let Some(GcEvent::Message(message)) = &event {
                if message.msg_type() == answer_type {
                    if let Some(answer) = answer(message)? {
                        return Ok(answer);
                    }
                    debug!("Passing on an answer of CS2's game coordinator about something else");
                }
            }
            self.handle(event)?;
        }
    }

    /// Sends hellos until the game coordinator welcomes the account, for up to [`GREET_TIMEOUT`].
    async fn greet(&mut self) -> Result<(), ClientError> {
        let deadline = Instant::now() + GREET_TIMEOUT;
        while let Some(next_hello) = self.hello_due() {
            tokio::select! {
                event = self.channel.recv() => self.handle(event)?,
                () = sleep_until(next_hello) => self.send_hello()?,
                () = sleep_until(deadline) => return Err(ClientError::GcTimeout(CLIENT_WELCOME)),
            }
        }
        Ok(())
    }

    /// When the next hello is due, unless welcomed. Starts greeting if the session was lost.
    fn hello_due(&mut self) -> Option<Instant> {
        match self.session {
            Session::Welcomed => None,
            Session::Lost => {
                let next_hello = Instant::now() + FIRST_HELLO;
                self.session = Session::Greeting {
                    next_hello,
                    interval: HELLO_INTERVAL,
                };
                Some(next_hello)
            }
            Session::Greeting { next_hello, .. } => Some(next_hello),
        }
    }

    fn send_hello(&mut self) -> Result<(), ClientError> {
        let Session::Greeting { interval, .. } = self.session else {
            return Ok(());
        };
        debug!("Greeting CS2's game coordinator, again in {:?}", interval);
        self.channel.send(CLIENT_HELLO, &hello())?;
        self.session = Session::Greeting {
            next_hello: Instant::now() + interval,
            interval: (interval * 2).min(MAX_HELLO_INTERVAL),
        };
        Ok(())
    }

    /// Reads what came since the last request, to know whether the session is still on.
    fn read_waiting(&mut self) -> Result<(), ClientError> {
        loop {
            match self.channel.try_recv() {
                Ok(event) => self.handle(Some(event))?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Stopped) => return Err(ClientError::Closed),
            }
        }
    }

    /// Follows the session, the inventory and the stats through `event`, from the channel, and keeps what's for the
    /// caller as events.
    fn handle(&mut self, event: Option<GcEvent>) -> Result<(), ClientError> {
        let message = match event.ok_or(ClientError::Closed)? {
            GcEvent::Message(message) => message,
            GcEvent::SessionLost => {
                debug!("Lost the session with CS2's game coordinator: the connection to Steam was lost");
                self.lose_session();
                // hellos sent while connecting again were dropped: start over
                self.session = Session::Lost;
                return Ok(());
            }
            // an answer may have been dropped; it may also still come
            GcEvent::Lagged(missed) => {
                warn!("Missed {} messages of CS2's game coordinator", missed);
                self.events.missed(missed);
                return Ok(());
            }
        };
        match message.msg_type() {
            CLIENT_WELCOME => self.welcomed(&message),
            CONNECTION_STATUS => match message.parse::<CMsgConnectionStatus>() {
                // the raw value, as node-globaloffensive compares it: one unknown here isn't a session
                Ok(status) => {
                    let status = status.status.map_or(0, |status| status.value());
                    if status != GCConnectionStatus::GCConnectionStatus_HAVE_SESSION as i32 {
                        debug!(
                            "CS2's game coordinator has no session for the account (status {})",
                            status
                        );
                        self.lose_session();
                    }
                }
                Err(error) => warn!("Unreadable connection status from CS2's game coordinator: {}", error),
            },
            LOGON_FATAL_ERROR => {
                self.lose_session();
                self.session = Session::Lost;
                let error = message.parse::<CMsgGCCStrike15_v2_ClientLogonFatalError>()?;
                return Err(ClientError::GcRefused {
                    code: error.errorcode(),
                    message: error.message().to_string(),
                });
            }
            MATCHMAKING_HELLO => match message.parse::<CMsgGCCStrike15_v2_MatchmakingGC2ClientHello>() {
                Ok(stats) => {
                    let stats = PlayerProfile::new(stats);
                    self.matchmaking_stats = Some(stats.clone());
                    self.events.push(Cs2Event::MatchmakingStats(stats));
                }
                Err(error) => {
                    warn!("Unreadable matchmaking stats from CS2's game coordinator: {}", error);
                    self.events.push(Cs2Event::Other(message));
                }
            },
            _ => match self.inventory.apply(&message, self.account) {
                Some(changes) => {
                    for change in changes {
                        self.events.push(match change {
                            Change::Loaded => Cs2Event::InventoryLoaded,
                            Change::Acquired(item) => Cs2Event::ItemAcquired(item),
                            Change::Changed { old, new } => Cs2Event::ItemChanged { old, new },
                            Change::Removed(item) => Cs2Event::ItemRemoved(item),
                        });
                    }
                }
                None => self.events.push(Cs2Event::Other(message)),
            },
        }
        Ok(())
    }

    /// Reads the welcome: the session is on, with the inventory it carries.
    fn welcomed(&mut self, message: &GcMessage) {
        debug!("CS2's game coordinator welcomed the account");
        self.session = Session::Welcomed;
        match message.parse::<CMsgClientWelcome>() {
            Ok(welcome) => {
                self.inventory.load(&welcome.outofdate_subscribed_caches, self.account);
            }
            Err(error) => warn!("Unreadable welcome from CS2's game coordinator: {}", error),
        }
        if mem::replace(&mut self.welcomed_before, true) {
            self.events.push(Cs2Event::Welcomed);
        }
    }

    /// The session ended, if it was on.
    fn lose_session(&mut self) {
        if self.session == Session::Welcomed {
            self.session = Session::Lost;
            self.events.push(Cs2Event::SessionLost);
        }
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

/// The account id of `player`, if an individual account in the public universe, as node-globaloffensive checks.
fn individual(player: SteamID) -> Result<u32, ClientError> {
    let account_id = player.account_id();
    if account_id == 0 || SteamID::from_account_id(account_id) != player {
        return Err(ClientError::NotIndividual(player));
    }
    Ok(account_id)
}

/// The events [`Cs2::next_event`] hasn't read.
#[derive(Debug, Default)]
struct EventQueue {
    events: VecDeque<Cs2Event>,
    /// Events dropped (or messages the channel dropped) since the last `Lagged`, which is added when there is room
    /// again.
    dropped: u64,
}

impl EventQueue {
    /// Keeps `event`, or drops it past [`EVENT_CAPACITY`], unless it's about the session. Those are never dropped,
    /// and one is enough while nothing came after it.
    fn push(&mut self, event: Cs2Event) {
        let about_session = matches!(event, Cs2Event::SessionLost | Cs2Event::Welcomed);
        if !about_session && self.events.len() >= EVENT_CAPACITY {
            if self.dropped == 0 {
                debug!("CS2's events aren't read: dropping them");
            }
            self.dropped += 1;
            return;
        }
        self.flush_dropped();
        if about_session {
            let last = self
                .events
                .iter()
                .rev()
                .find(|event| !matches!(event, Cs2Event::Lagged(_)));
            if last.map(mem::discriminant) == Some(mem::discriminant(&event)) {
                return;
            }
        }
        self.events.push_back(event);
    }

    /// Counts `missed` events, in their place.
    fn missed(&mut self, missed: u64) {
        self.dropped += missed;
    }

    fn pop(&mut self) -> Option<Cs2Event> {
        let Some(event) = self.events.pop_front() else {
            // the events dropped came after every event read
            return (self.dropped > 0).then(|| Cs2Event::Lagged(mem::take(&mut self.dropped)));
        };
        free_if_big(&mut self.events);
        Some(event)
    }

    fn flush_dropped(&mut self) {
        if self.dropped > 0 {
            self.events.push_back(Cs2Event::Lagged(mem::take(&mut self.dropped)));
        }
    }
}

#[cfg(test)]
mod tests;

//! Game coordinators: a game's own server, reached through the CM connection (`ClientToGC` / `ClientFromGC`).
//!
//! A game's coordinator (GC) only talks to an account that plays the game: set it with
//! [`SteamHandle::set_games_played`], then open a [`GcChannel`] with [`SteamHandle::game_coordinator`] and greet the GC
//! as the game does (CS2: `ClientHello` until `ClientWelcome`). Messages from the GC of that app then go to the
//! channel instead of the [`Events`](crate::handle::Events).
//!
//! Each message travels in a `CMsgGCClient` (`appid`, `msgtype`, `payload`), the CM header routed to the app
//! (`routing_appid`), as SteamKit's `SteamGameCoordinator` and node-steam-user's `sendToGC` send it. A protobuf GC
//! message (`msgtype` flagged with `0x8000_0000`) is a payload of its type with the flag (u32 LE), the length of its
//! GC header (u32 LE), that `CMsgProtoBufHeader`, then the body; any other has an 18-byte header (u16 version, u64
//! target job, u64 source job) before its body. Messages are sent with an empty GC header: its job ids then hold
//! their default, none.
//!
//! Without a task, [`SteamClient::send_to_gc`](crate::client::SteamClient::send_to_gc) sends, and
//! [`RawMessage::gc_message`] reads what [`next_event`](crate::client::SteamClient::next_event) gives.
//!
//! [`GcChannel`] shows a channel greeting CS2's game coordinator and reading what it sends. With the `cs2` feature,
//! `gc::cs2` is the worked case for one game: a session over a channel that greets the coordinator, pairs each request
//! with its answer and reads CS2's messages.
//!
//! [`SteamHandle::set_games_played`]: crate::handle::SteamHandle::set_games_played
//! [`SteamHandle::game_coordinator`]: crate::handle::SteamHandle::game_coordinator

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use bytes::Buf;
use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::MessageError;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgGCClient;
use tokio::sync::mpsc::WeakUnboundedSender;
use tokio::sync::Notify;

use crate::errors::ClientError;
use crate::events::RawMessage;
use crate::handle::Command;
use crate::handle::TryRecvError;
use crate::messages::message::ClientMessage;
use crate::protocol::free_if_big;
use crate::service;

#[cfg(feature = "cs2")]
pub mod cs2;

/// Flags a GC message type as protobuf.
const PROTO_MASK: u32 = 0x8000_0000;
/// A protobuf GC message starts with its type and the length of its header.
const PROTO_HEADER_START: usize = 8;
/// The header of a GC message that isn't protobuf: version, target job, source job.
const HEADER_LENGTH: usize = 18;
/// How many messages wait in a [`GcChannel`]; past this, newer ones are dropped and a [`GcEvent::Lagged`] counts
/// them.
pub const GC_CAPACITY: usize = 256;

/// The messages of one app's game coordinator, from a [spawned](crate::client::SteamClient::spawn) client, and the
/// way to send to it. Made by [`SteamHandle::game_coordinator`](crate::handle::SteamHandle::game_coordinator).
///
/// The task never waits for the reader: past [`GC_CAPACITY`] messages waiting, newer ones are dropped and a
/// [`GcEvent::Lagged`] counts them. Dropping the channel gives the app's messages back to the
/// [`Events`](crate::handle::Events). The channel doesn't keep the client running: it ends once the client stopped,
/// or once another channel took the app.
///
/// Any game's coordinator, here CS2's, with its protos from steam-protobuf's `csgo` feature (which the `cs2` feature
/// enables): play the game, greet the coordinator until it welcomes the account, and again after the session was
/// lost.
///
/// ```no_run
/// # #[cfg(feature = "cs2")]
/// # async fn run(handle: steam_client::handle::SteamHandle<steam_client::client::User>)
/// # -> Result<(), steam_client::errors::ClientError> {
/// use std::time::Duration;
///
/// use steam_client::gc::GcEvent;
/// use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientHello;
/// use steam_protobuf::protobufs::csgo::gcsdk_gcmessages::CMsgClientWelcome;
/// use steam_protobuf::protobufs::csgo::gcsystemmsgs::EGCBaseClientMsg;
/// use tokio::time;
/// use tokio::time::MissedTickBehavior;
///
/// const CS2: u32 = 730;
/// const HELLO: u32 = EGCBaseClientMsg::k_EMsgGCClientHello as u32;
/// const WELCOME: u32 = EGCBaseClientMsg::k_EMsgGCClientWelcome as u32;
///
/// handle.set_games_played(&[CS2])?;
/// let mut channel = handle.game_coordinator(CS2)?;
/// let mut welcomed = false;
/// let mut hellos = time::interval(Duration::from_secs(5));
/// // one hello at once after the session was lost, not one for each tick missed
/// hellos.set_missed_tick_behavior(MissedTickBehavior::Delay);
/// loop {
///     let event = tokio::select! {
///         _ = hellos.tick(), if !welcomed => {
///             channel.send(HELLO, &CMsgClientHello::new())?;
///             continue;
///         }
///         event = channel.recv() => event,
///     };
///     match event {
///         Some(GcEvent::Message(message)) if message.msg_type() == WELCOME => {
///             let welcome: CMsgClientWelcome = message.parse()?;
///             println!("Welcomed by version {}", welcome.version());
///             welcomed = true;
///         }
///         Some(GcEvent::Message(message)) => println!("Message {}", message.msg_type()),
///         // the hellos sent while the client connects again are dropped: keep sending them
///         Some(GcEvent::SessionLost) => welcomed = false,
///         Some(GcEvent::Lagged(missed)) => println!("Missed {} messages", missed),
///         Some(_) => {}
///         // the client stopped, or another channel took the app
///         None => return Ok(()),
///     }
/// }
/// # }
/// ```
pub struct GcChannel {
    app_id: u32,
    queue: Arc<GcQueue>,
    commands: WeakUnboundedSender<Command>,
}

/// What a [`GcChannel`] receives.
#[derive(Debug)]
#[non_exhaustive]
pub enum GcEvent {
    /// A message from the game coordinator.
    Message(GcMessage),
    /// The client lost its connection to Steam, and with it the session with the game coordinator. Messages sent
    /// until it logged on again are dropped; once it did (the games played are set again), greet the coordinator
    /// again as the game does. The messages after this are from the new session.
    SessionLost,
    /// This many messages were dropped here because they weren't read in time, or (counted by every channel) one from
    /// a game coordinator couldn't be read. The events after it are the newer ones.
    Lagged(u64),
}

/// A message from a game coordinator, its body not parsed yet. Holds a slice of what was read, not a copy.
#[derive(Clone)]
pub struct GcMessage {
    app_id: u32,
    /// As Steam sent it: with [`PROTO_MASK`] if the body is protobuf.
    msg_type: u32,
    body: Bytes,
}

impl GcChannel {
    /// The app whose game coordinator this is.
    pub fn app_id(&self) -> u32 {
        self.app_id
    }

    /// Sends a protobuf message of type `msg_type` (the game's own numbering, without the protobuf flag) to the game
    /// coordinator. The task sends it: this doesn't wait. While the client is connecting again, it is dropped (the
    /// channel got a [`GcEvent::SessionLost`]).
    ///
    /// Fails with [`ClientError::Closed`] once the client stopped or another channel took the app, and if `body`
    /// lacks a required field.
    pub fn send<M: Message>(&self, msg_type: u32, body: &M) -> Result<(), ClientError> {
        if self.queue.lock().stopped {
            return Err(ClientError::Closed);
        }
        let message = client_to_gc(self.app_id, msg_type, body)?;
        let commands = self.commands.upgrade().ok_or(ClientError::Closed)?;
        commands
            .send(Command::ToGc(Box::new(message)))
            .map_err(|_| ClientError::Closed)
    }

    /// The next event, or `None` once the client stopped, or another channel took the app, and every event was read.
    /// Cancel-safe: it can wait in a `select!`.
    pub async fn recv(&mut self) -> Option<GcEvent> {
        loop {
            {
                let mut queue = self.queue.lock();
                if let Some(event) = queue.pop() {
                    return Some(event);
                }
                if queue.stopped {
                    return None;
                }
            }
            self.queue.ready.notified().await;
        }
    }

    /// The next event if one waits, without waiting. [`TryRecvError::Stopped`] once [`recv`](Self::recv) would give
    /// `None`.
    pub fn try_recv(&mut self) -> Result<GcEvent, TryRecvError> {
        let mut queue = self.queue.lock();
        match queue.pop() {
            Some(event) => Ok(event),
            None if queue.stopped => Err(TryRecvError::Stopped),
            None => Err(TryRecvError::Empty),
        }
    }
}

impl Drop for GcChannel {
    fn drop(&mut self) {
        let mut queue = self.queue.lock();
        queue.reader_gone = true;
        queue.events = VecDeque::new();
        queue.messages = 0;
    }
}

impl fmt::Debug for GcChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcChannel")
            .field("app_id", &self.app_id)
            .field("waiting", &self.queue.lock().events.len())
            .finish()
    }
}

impl GcMessage {
    /// The app whose game coordinator sent it.
    pub fn app_id(&self) -> u32 {
        self.app_id
    }

    /// Its type, in the game's own numbering, without the protobuf flag.
    pub fn msg_type(&self) -> u32 {
        self.msg_type & !PROTO_MASK
    }

    /// Whether the body is protobuf. Other bodies have the game's own layout.
    pub fn is_proto(&self) -> bool {
        self.msg_type & PROTO_MASK != 0
    }

    /// The body, after the GC header.
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// Parses the body, as `M`. Its `bytes` fields share the buffer the message was read into.
    pub fn parse<M: Message>(&self) -> Result<M, ClientError> {
        M::parse_from_tokio_bytes(&self.body).map_err(|error| ClientError::Message(error.into()))
    }
}

impl fmt::Debug for GcMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcMessage")
            .field("app_id", &self.app_id)
            .field("msg_type", &self.msg_type())
            .field("is_proto", &self.is_proto())
            .field("len", &self.body.len())
            .finish()
    }
}

impl RawMessage {
    /// The game coordinator's message this carries, if it is a [`EMsg::ClientFromGC`]. Fails if its envelope or GC
    /// header can't be read.
    pub fn gc_message(&self) -> Option<Result<GcMessage, ClientError>> {
        if self.emsg() != EMsg::ClientFromGC {
            return None;
        }
        Some(self.parse().map_err(ClientError::from).and_then(|packet| {
            let envelope = ClientMessage::<CMsgGCClient>::from_proto_packet(packet)?.body;
            let (app_id, msg_type) = (envelope.appid(), envelope.msgtype());
            let body = gc_body(msg_type, envelope.payload.unwrap_or_default())?;
            Ok(GcMessage { app_id, msg_type, body })
        }))
    }
}

/// The body of a GC message of type `msg_type` (as `CMsgGCClient` has it, flagged if protobuf): `payload` past its
/// GC header, a slice of it. Fails if the payload is shorter than its header.
pub(crate) fn gc_body(msg_type: u32, mut payload: Bytes) -> Result<Bytes, MessageError> {
    let header_length = if msg_type & PROTO_MASK == 0 {
        HEADER_LENGTH
    } else {
        let too_short = MessageError::TooShort {
            needed: PROTO_HEADER_START,
            got: payload.len(),
        };
        let length = payload.get(4..PROTO_HEADER_START).ok_or(too_short)?;
        let length = u32::from_le_bytes([length[0], length[1], length[2], length[3]]);
        // u64, so that it can't overflow on 32 bits
        let needed = PROTO_HEADER_START as u64 + u64::from(length);
        if needed > payload.len() as u64 {
            return Err(MessageError::TooShort {
                needed: needed.min(usize::MAX as u64) as usize,
                got: payload.len(),
            });
        }
        needed as usize
    };
    if payload.len() < header_length {
        return Err(MessageError::TooShort {
            needed: header_length,
            got: payload.len(),
        });
    }
    payload.advance(header_length);
    Ok(payload)
}

/// The `ClientToGC` body of a protobuf message to app `app_id`'s game coordinator, with an empty GC header.
pub(crate) fn client_to_gc<M: Message>(app_id: u32, msg_type: u32, body: &M) -> Result<CMsgGCClient, ClientError> {
    service::check(body)?;
    let msg_type = msg_type | PROTO_MASK;
    let mut payload = Vec::with_capacity(PROTO_HEADER_START + body.compute_size() as usize);
    payload.extend_from_slice(&msg_type.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    body.write_to_vec(&mut payload)
        .map_err(|error| ClientError::Message(error.into()))?;

    let mut message = CMsgGCClient::new();
    message.set_appid(app_id);
    message.set_msgtype(msg_type);
    message.set_payload(payload.into());
    Ok(message)
}

/// A channel for `app_id`'s game coordinator, sending through `commands`, and the task's end of it, to add to its
/// [`GcRoutes`].
pub(crate) fn channel(app_id: u32, commands: WeakUnboundedSender<Command>) -> (GcChannel, Route) {
    let queue = Arc::new(GcQueue::default());
    let route = Route {
        app_id,
        queue: queue.clone(),
    };
    let channel = GcChannel {
        app_id,
        queue,
        commands,
    };
    (channel, route)
}

/// The task's routes: which [`GcChannel`] each app's messages go to.
#[derive(Default)]
pub(crate) struct GcRoutes(HashMap<u32, Route>);

impl GcRoutes {
    /// Sends the app's messages to `route`'s channel. It replaces the app's earlier one, which ends.
    pub(crate) fn add(&mut self, route: Route) {
        if self.0.insert(route.app_id, route).is_some() {
            debug!("A new channel took the game coordinator's messages");
        }
    }

    /// Passes a game coordinator's message to its app's channel. Gives the message back if it isn't one, or no
    /// channel takes it: no channel was opened for the app, it was dropped, or the message can't be read.
    pub(crate) fn route(&mut self, message: RawMessage) -> Option<RawMessage> {
        if self.0.is_empty() || message.emsg() != EMsg::ClientFromGC {
            return Some(message);
        }
        let gc_message = match message.gc_message() {
            Some(Ok(gc_message)) => gc_message,
            Some(Err(error)) => {
                warn!("Could not read a message from a game coordinator: {}", error);
                // whose it is can't be told: each channel misses it, as one dropped
                self.0.retain(|_, route| route.queue.missed());
                return Some(message);
            }
            None => return Some(message),
        };
        let app_id = gc_message.app_id;
        let Some(route) = self.0.get(&app_id) else {
            return Some(message);
        };
        if route.queue.push(gc_message) {
            return None;
        }
        debug!("The channel of app {}'s game coordinator was dropped", app_id);
        self.0.remove(&app_id);
        Some(message)
    }

    /// Tells every channel the session was lost, and forgets those dropped.
    pub(crate) fn session_lost(&mut self) {
        self.0.retain(|_, route| route.queue.session_lost());
    }
}

/// The task's end of a [`GcChannel`]. Dropping it ends the channel, once its events are read.
pub(crate) struct Route {
    app_id: u32,
    queue: Arc<GcQueue>,
}

impl Drop for Route {
    fn drop(&mut self) {
        self.queue.lock().stopped = true;
        self.queue.ready.notify_one();
    }
}

/// The events a [`GcChannel`] hasn't read, shared with the task, which only holds the lock to add to it.
#[derive(Default)]
struct GcQueue {
    queue: Mutex<Queue>,
    /// Woken when events are added, or the channel ends.
    ready: Notify,
}

#[derive(Default)]
struct Queue {
    events: VecDeque<GcEvent>,
    /// How many of `events` are messages.
    messages: usize,
    /// Messages dropped since the last `Lagged`, which is added when there is room again.
    dropped: u64,
    /// The task's end is gone: nothing more comes.
    stopped: bool,
    /// The channel was dropped: its messages go to the events.
    reader_gone: bool,
}

impl GcQueue {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps a message, or drops it past [`GC_CAPACITY`]. False if the channel was dropped.
    fn push(&self, message: GcMessage) -> bool {
        let mut queue = self.lock();
        if queue.reader_gone {
            return false;
        }
        if queue.messages >= GC_CAPACITY {
            if queue.dropped == 0 {
                warn!(
                    "The game coordinator of app {} sends faster than its messages are read: dropping them",
                    message.app_id
                );
            }
            queue.dropped += 1;
            return true;
        }
        queue.flush_dropped();
        queue.events.push_back(GcEvent::Message(message));
        queue.messages += 1;
        drop(queue);
        self.ready.notify_one();
        true
    }

    /// Counts a message that couldn't be read as dropped. False if the channel was dropped.
    fn missed(&self) -> bool {
        let mut queue = self.lock();
        if queue.reader_gone {
            return false;
        }
        queue.dropped += 1;
        drop(queue);
        self.ready.notify_one();
        true
    }

    /// Keeps a [`GcEvent::SessionLost`], never dropped. One is enough while no message came after the last one. False
    /// if the channel was dropped.
    fn session_lost(&self) -> bool {
        let mut queue = self.lock();
        if queue.reader_gone {
            return false;
        }
        queue.flush_dropped();
        let last = queue
            .events
            .iter()
            .rev()
            .find(|event| !matches!(event, GcEvent::Lagged(_)));
        if !matches!(last, Some(GcEvent::SessionLost)) {
            queue.events.push_back(GcEvent::SessionLost);
        }
        drop(queue);
        self.ready.notify_one();
        true
    }
}

impl Queue {
    fn pop(&mut self) -> Option<GcEvent> {
        let Some(event) = self.events.pop_front() else {
            // the messages dropped came after every message read
            return (self.dropped > 0).then(|| GcEvent::Lagged(mem::take(&mut self.dropped)));
        };
        if let GcEvent::Message(_) = event {
            self.messages -= 1;
        }
        free_if_big(&mut self.events);
        Some(event)
    }

    /// Counts the messages dropped, in their place, before what comes next.
    fn flush_dropped(&mut self) {
        if self.dropped > 0 {
            self.events.push_back(GcEvent::Lagged(mem::take(&mut self.dropped)));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

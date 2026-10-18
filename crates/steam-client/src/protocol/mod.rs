//! The CM protocol as a state machine with no IO ("sans-IO", as quinn-proto and rustls do it).
//!
//! [`CmProtocol`] takes the bytes read from the connection and the time they were read, and gives back the bytes to
//! write, what Steam sent, and when it next needs the time. It owns everything between the socket and the messages:
//! framing, the channel encryption handshake and the encryption itself, parsing, unpacking `Multi`s, matching
//! responses to calls by job id, the logon session and its heartbeat. It has no async code and reads no clock; the
//! driver, [`crate::connection::Connection`], does the IO.
//!
//! Reading messages allocates nothing, past growing the buffers and parsing protobuf bodies (see
//! `tests/zero_alloc.rs`).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::net::Ipv4Addr;
use std::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use bytes::BytesMut;
use num_traits::FromPrimitive;
use steam_crypto::SessionKey;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::generated::EMsgError;
use steam_language_gen::HasJobId;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;
use steam_language_gen::SerializableBytes;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHeartBeat;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogOff;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;
use steamid_parser::SteamID;

use crate::errors::ClientError;
use crate::events::Event;
use crate::events::RawMessage;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::protocol::encryption::answer_encrypt_request;
use crate::protocol::encryption::check_encrypt_result;
use crate::protocol::framing::Encryption;
use crate::protocol::multi::unpack_multi;
use crate::service;
use crate::service::Caller;
use crate::service::ServiceMethod;

pub(crate) mod encryption;
pub(crate) mod framing;
pub(crate) mod multi;
#[cfg(test)]
pub(crate) mod tests;

/// Clears the protobuf flag of a raw EMsg.
const EMSG_MASK: u32 = 0x7FFF_FFFF;
/// Flags an EMsg as followed by a protobuf header.
const PROTO_FLAG: u32 = 0x8000_0000;
/// How many [`Event`]s from earlier reads are kept; past this, the oldest are dropped and reported as
/// [`Event::Lagged`]. The events of one read, such as the messages of a big `Multi`, are all kept, up to
/// [`MAX_EVENTS_PER_READ`]. An event takes 48 bytes, plus the buffer its message is a slice of (see [`RawMessage`]):
/// a read's messages share theirs, while each read kept holds its own. The queue grows as events come: it takes
/// nothing until then.
pub(crate) const EVENT_QUEUE_CAPACITY: usize = 256;
/// The most events one read keeps, against a `Multi` of countless tiny messages.
pub(crate) const MAX_EVENTS_PER_READ: usize = 4096;
/// What the buffer of bytes received starts with. Most frames are smaller.
const RECEIVE_BUFFER_CAPACITY: usize = 8 * 1024;
/// The biggest frame whose buffer is kept for the next ones: after a bigger one, the buffer is replaced once empty, so
/// its memory goes. Up to this, a buffer that grew is reused, and reading allocates nothing.
const MAX_KEPT_FRAME: usize = 64 * 1024;
/// The least room the buffer of bytes received has before a read.
const MIN_READ: usize = 4 * 1024;
/// The most a queue keeps room for once empty: a bigger one, after a burst such as a big `Multi`, is freed.
const QUEUE_KEPT_CAPACITY: usize = 64;

/// How messages travel, chosen when connecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// TCP: every message in a `VT01` frame, encrypted with a session key after the channel encryption handshake.
    Tcp,
    /// WebSocket, planned (there is no driver for it yet): TLS encrypts, and every WebSocket message is one message.
    WebSocket,
}

/// The CM protocol of one connection, with no IO.
///
/// Read into [`CmProtocol::receive_buffer`] and call [`CmProtocol::handle_received`] (or pass what was read to
/// [`CmProtocol::handle_input`]), call [`CmProtocol::handle_timeout`] once [`CmProtocol::poll_timeout`] passed, and
/// write what [`CmProtocol::poll_transmit`] gives. Requests are answered through their own methods
/// ([`CmProtocol::take_response`], [`CmProtocol::logon_result`]); messages nothing waits for are kept as [`Event`]s.
pub(crate) struct CmProtocol {
    channel: Channel,
    logon: Logon,
    jobs: JobIds,
    /// Calls waiting for their response, or answered and not taken yet, by job id.
    calls: HashMap<u64, Answer>,
    /// Bytes received and not framed yet (TCP), or the message received (WebSocket).
    received: BytesMut,
    /// `received` grew for a frame bigger than [`MAX_KEPT_FRAME`]: it is replaced once empty, so the memory goes once
    /// nothing else holds it.
    received_grew: bool,
    /// Packets unpacked from a `Multi`, not handled yet.
    unpacked: VecDeque<Bytes>,
    /// Messages sent before the channel was encrypted, sent once it is.
    unencrypted: Vec<Vec<u8>>,
    /// The message being sent, before it is encrypted. Kept for its capacity.
    serialized: Vec<u8>,
    /// What to write, in order.
    outgoing: BytesMut,
    /// Over WebSocket, the length of each message in `outgoing`.
    message_lengths: VecDeque<usize>,
    /// Messages nothing waits for: at most [`EVENT_QUEUE_CAPACITY`] from earlier reads, and those of this read.
    events: VecDeque<Event>,
    /// How many events this read kept so far.
    read_events: usize,
    /// How many events were dropped since the last [`Event::Lagged`].
    lagged: u64,
}

/// The channel encryption handshake, then the encrypted channel.
#[derive(Debug)]
enum Channel {
    /// TCP: waiting for Steam's `ChannelEncryptRequest`.
    AwaitingEncryptRequest,
    /// TCP: answered with a new session key, waiting for the `ChannelEncryptResult`.
    AwaitingEncryptResult(Encryption),
    /// TCP: every frame is encrypted with the session key.
    Encrypted(Encryption),
    /// WebSocket: TLS encrypts, and every input is one message.
    WebSocket,
    /// The input was invalid, or the connection closed: it takes no more input.
    Failed,
}

/// The logon session.
#[derive(Debug)]
enum Logon {
    /// Not logged on: calls go as `ServiceMethodCallFromClientNonAuthed`.
    LoggedOut,
    /// `ClientLogon` sent, waiting for Steam's response.
    Pending,
    /// Messages carry the session on their header, and a heartbeat is due at `next_heartbeat`.
    LoggedOn { info: LogonInfo, next_heartbeat: Instant },
    /// Steam refused the logon or logged us off, or the connection failed: calls fail.
    Ended(Ended),
}

/// What Steam assigned on logon.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LogonInfo {
    pub(crate) session: Session,
    pub(crate) heartbeat_interval: Duration,
    pub(crate) cell_id: u32,
    pub(crate) public_ip: Option<Ipv4Addr>,
}

/// Why the session ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Ended {
    /// Steam refused the logon.
    Refused { result: EResult, extended: Option<EResult> },
    /// Steam logged us off.
    LoggedOff(EResult),
    /// The connection failed.
    Dropped,
}

impl From<Ended> for ClientError {
    fn from(ended: Ended) -> Self {
        match ended {
            Ended::Refused { result, extended } => Self::LogonFailed { result, extended },
            Ended::LoggedOff(result) => Self::LoggedOff(result),
            Ended::Dropped => Self::Dropped,
        }
    }
}

/// What a call got.
#[derive(Debug)]
enum Answer {
    Waiting,
    /// The response, unparsed, or why none will come.
    Answered(Result<RawMessage, Ended>),
}

/// A call of service method `M`, to take its response with [`CmProtocol::take_response`].
///
/// It belongs to the protocol that made it: job ids are counted per protocol, so another one would take the answer
/// to its own job with that id. The connection only uses a call with its own protocol, and a spawned client fails
/// its calls when it replaces the connection.
pub(crate) struct Call<M> {
    job_id: u64,
    method: PhantomData<fn() -> M>,
}

impl<M> Call<M> {
    pub(crate) fn job_id(&self) -> u64 {
        self.job_id
    }
}

impl CmProtocol {
    pub(crate) fn new(transport: Transport) -> Self {
        Self::with_channel(match transport {
            Transport::Tcp => Channel::AwaitingEncryptRequest,
            Transport::WebSocket => Channel::WebSocket,
        })
    }

    /// Over TCP, with the channel already encrypted with `key` as Steam's CMs do, for tests: they can't answer the
    /// handshake as Steam.
    pub(crate) fn encrypted(key: SessionKey) -> Self {
        Self::with_channel(Channel::Encrypted(Encryption::HmacIv(key)))
    }

    fn with_channel(channel: Channel) -> Self {
        Self {
            channel,
            logon: Logon::LoggedOut,
            jobs: JobIds::new(),
            calls: HashMap::new(),
            received: BytesMut::with_capacity(RECEIVE_BUFFER_CAPACITY),
            received_grew: false,
            unpacked: VecDeque::new(),
            unencrypted: Vec::new(),
            serialized: Vec::new(),
            outgoing: BytesMut::new(),
            message_lengths: VecDeque::new(),
            events: VecDeque::new(),
            read_events: 0,
            lagged: 0,
        }
    }

    /// Whether messages flow: over TCP, once the channel is encrypted. What is sent before waits until then.
    pub(crate) fn is_connected(&self) -> bool {
        matches!(self.channel, Channel::Encrypted(_) | Channel::WebSocket)
    }

    /// The session key, once the channel is encrypted, for tests to talk to the client.
    #[cfg(test)]
    pub(crate) fn session_key(&self) -> Option<&SessionKey> {
        match &self.channel {
            Channel::Encrypted(encryption) => Some(encryption.key()),
            _ => None,
        }
    }

    /// Where to read what the connection received, with room for a read. Then call
    /// [`CmProtocol::handle_received`].
    pub(crate) fn receive_buffer(&mut self) -> &mut BytesMut {
        self.received.reserve(MIN_READ);
        &mut self.received
    }

    /// Takes what the connection read at `now`, as [`CmProtocol::handle_received`] does, from a slice.
    pub(crate) fn handle_input(&mut self, data: &[u8], now: Instant) -> Result<(), ClientError> {
        self.received.extend_from_slice(data);
        self.handle_received(now)
    }

    /// Takes what the connection read into [`CmProtocol::receive_buffer`] at `now`: over TCP, any part of the stream;
    /// over WebSocket, one message.
    ///
    /// Fails if the connection can't be used anymore: a frame could not be read or decrypted, or the channel
    /// encryption failed. From then on, input fails with `Dropped`, and so do calls. Messages that can't be parsed
    /// are logged and skipped.
    pub(crate) fn handle_received(&mut self, now: Instant) -> Result<(), ClientError> {
        self.read_events = 0;
        loop {
            match self.next_message() {
                Ok(Some(message)) => self.receive(message, now),
                Ok(None) => break,
                Err(error) => {
                    self.fail();
                    return Err(error);
                }
            }
        }
        if self.received_grew && self.received.is_empty() {
            // frees the big buffer once the messages it holds are gone; until then, they keep it
            self.received = BytesMut::with_capacity(RECEIVE_BUFFER_CAPACITY);
            self.received_grew = false;
        }
        free_if_big(&mut self.unpacked);
        self.read_events = 0;
        Ok(())
    }

    /// The connection closed, or failed below the protocol. Everything waiting fails with `Dropped`, as will later
    /// calls, unless the session already ended otherwise.
    pub(crate) fn handle_closed(&mut self) {
        self.fail();
    }

    /// Sends a heartbeat if one is due at `now`. Calling it earlier does nothing.
    pub(crate) fn handle_timeout(&mut self, now: Instant) {
        let session = match &mut self.logon {
            Logon::LoggedOn { info, next_heartbeat } if *next_heartbeat <= now => {
                // from now rather than from when it was due, so a late heartbeat isn't followed by a burst
                *next_heartbeat = now + info.heartbeat_interval;
                info.session
            }
            _ => return,
        };
        trace!("Sending a heartbeat");
        self.send_proto(EMsg::ClientHeartBeat, &session.header(), &CMsgClientHeartBeat::new());
    }

    /// When [`CmProtocol::handle_timeout`] has something to do: the next heartbeat, once logged on.
    pub(crate) fn poll_timeout(&self) -> Option<Instant> {
        match &self.logon {
            Logon::LoggedOn { next_heartbeat, .. } => Some(*next_heartbeat),
            _ => None,
        }
    }

    /// How many bytes wait to be written.
    pub(crate) fn unsent(&self) -> usize {
        self.outgoing.len()
    }

    /// What to write next: over TCP, everything there is; over WebSocket, one message. A slice of the protocol's
    /// buffer, not a copy.
    pub(crate) fn poll_transmit(&mut self) -> Option<Bytes> {
        let length = match self.channel {
            Channel::WebSocket => self.message_lengths.pop_front()?,
            _ if self.outgoing.is_empty() => return None,
            _ => self.outgoing.len(),
        };
        Some(self.outgoing.split_to(length).freeze())
    }

    /// The next message Steam sent that nothing waited for, or how many were dropped before it.
    pub(crate) fn poll_event(&mut self) -> Option<Event> {
        if self.lagged > 0 {
            return Some(Event::Lagged(mem::take(&mut self.lagged)));
        }
        let event = self.events.pop_front();
        free_if_big(&mut self.events);
        event
    }

    /// Sends a message, with the session on its header once logged on.
    pub(crate) fn send<M: Message>(&mut self, emsg: EMsg, body: M) {
        let header = self.header();
        trace!("Sending {:?}", emsg);
        self.send_proto(emsg, &header, &body);
    }

    /// Sends a message routed to app `app_id` (`routing_appid` on the header), such as one for its game coordinator,
    /// with the session on its header once logged on.
    pub(crate) fn send_routed<M: Message>(&mut self, emsg: EMsg, app_id: u32, body: M) {
        let mut header = self.header();
        header.set_routing_appid(app_id);
        trace!("Sending {:?} to app {}", emsg, app_id);
        self.send_proto(emsg, &header, &body);
    }

    /// The header of what is sent: with the session once logged on.
    fn header(&self) -> CMsgProtoBufHeader {
        match &self.logon {
            Logon::LoggedOn { info, .. } => info.session.header(),
            _ => CMsgProtoBufHeader::new(),
        }
    }

    /// Calls a service method: before logon as `ServiceMethodCallFromClientNonAuthed`, once logged on as
    /// `ServiceMethodCallFromClient` with the session on the header. Once the session ended, the call fails at once.
    ///
    /// `request` must have its required fields set (see [`service::check`]).
    pub(crate) fn call<M: ServiceMethod>(&mut self, request: M::Request) -> Call<M> {
        let job_id = self.jobs.next();
        let answer = match self.caller() {
            Ok(caller) => {
                trace!("Calling {} as job {}", M::METHOD, job_id);
                let header = service::request_header::<M>(&caller, job_id);
                self.send_proto(caller.emsg(), &header, &request);
                Answer::Waiting
            }
            // nothing would answer
            Err(ended) => Answer::Answered(Err(ended)),
        };
        self.calls.insert(job_id, answer);
        Call {
            job_id,
            method: PhantomData,
        }
    }

    fn caller(&self) -> Result<Caller, Ended> {
        match &self.logon {
            Logon::LoggedOut | Logon::Pending => Ok(Caller::NonAuthed),
            Logon::LoggedOn { info, .. } => Ok(Caller::Authed(info.session)),
            Logon::Ended(ended) => Err(*ended),
        }
    }

    /// Takes the response to `call` once it came, parsed, or why the call failed.
    pub(crate) fn take_response<M: ServiceMethod>(
        &mut self,
        call: &Call<M>,
    ) -> Option<Result<M::Response, ClientError>> {
        Some(match self.take_answer(call.job_id)? {
            Ok(message) => message
                .parse()
                .map_err(ClientError::from)
                .and_then(service::response::<M>),
            Err(ended) => Err(ended.into()),
        })
    }

    /// Takes the answer to job `job_id` once it came: the response, unparsed, or why the call failed.
    pub(crate) fn take_answer(&mut self, job_id: u64) -> Option<Result<RawMessage, Ended>> {
        match self.calls.remove(&job_id)? {
            Answer::Waiting => {
                self.calls.insert(job_id, Answer::Waiting);
                None
            }
            Answer::Answered(answer) => Some(answer),
        }
    }

    /// Stops waiting for job `job_id`: its response, if it came and wasn't taken, or once it comes, is kept as an
    /// [`Event`].
    pub(crate) fn forget(&mut self, job_id: u64) {
        if let Some(Answer::Answered(Ok(message))) = self.calls.remove(&job_id) {
            debug!("Nothing waits for the response to job {} anymore", job_id);
            self.push_event(Event::Message(message));
        }
    }

    /// Sends `ClientLogon`, with `steam_id` on the header until Steam assigns ours. [`CmProtocol::logon_result`] says
    /// how it went. Once the session ended, the logon fails at once.
    pub(crate) fn log_on(&mut self, steam_id: SteamID, logon: CMsgClientLogon) {
        if let Logon::Ended(_) = self.logon {
            // nothing would answer
            return;
        }
        let mut header = CMsgProtoBufHeader::new();
        header.set_steamid(steam_id.to_steam64());
        header.set_client_sessionid(0);
        trace!("Sending ClientLogon");
        self.send_proto(EMsg::ClientLogon, &header, &logon);
        self.logon = Logon::Pending;
    }

    /// What Steam assigned, once it answered `ClientLogon`, or why the logon failed.
    pub(crate) fn logon_result(&self) -> Option<Result<LogonInfo, ClientError>> {
        match &self.logon {
            Logon::LoggedOut | Logon::Pending => None,
            Logon::LoggedOn { info, .. } => Some(Ok(*info)),
            Logon::Ended(ended) => Some(Err((*ended).into())),
        }
    }

    /// Sends `ClientLogOff`. [`CmProtocol::logged_off`] says when Steam confirmed.
    pub(crate) fn log_off(&mut self) {
        self.send(EMsg::ClientLogOff, CMsgClientLogOff::new());
    }

    /// Why Steam logged us off, once it did, or why the session ended otherwise.
    pub(crate) fn logged_off(&self) -> Option<Result<EResult, ClientError>> {
        match self.logon {
            Logon::Ended(Ended::LoggedOff(result)) => Some(Ok(result)),
            Logon::Ended(ended) => Some(Err(ended.into())),
            _ => None,
        }
    }

    /// The next message received, decrypted. Runs the channel encryption handshake on the way.
    fn next_message(&mut self) -> Result<Option<Bytes>, ClientError> {
        loop {
            let frame = match &self.channel {
                Channel::Failed => return Err(ClientError::Dropped),
                Channel::WebSocket if self.received.is_empty() => return Ok(None),
                Channel::WebSocket => self.received.split(),
                _ => match framing::decode(&mut self.received)? {
                    Some(frame) => frame,
                    None => return Ok(None),
                },
            };
            if frame.len() > MAX_KEPT_FRAME {
                self.received_grew = true;
            }
            match &self.channel {
                Channel::Encrypted(encryption) => return Ok(Some(framing::decrypt_frame(frame, encryption)?)),
                Channel::WebSocket => return Ok(Some(frame.freeze())),
                _ => self.handshake(frame.freeze())?,
            }
        }
    }

    /// Handles a plain frame during the channel encryption handshake. A frame that can't be parsed is an error; other
    /// messages than the one expected are skipped.
    fn handshake(&mut self, frame: Bytes) -> Result<(), ClientError> {
        let packet = PacketMessage::from_bytes(frame)?;
        match (&self.channel, packet.emsg()) {
            (Channel::AwaitingEncryptRequest, EMsg::ChannelEncryptRequest) => {
                let (encryption, response) = answer_encrypt_request(packet)?;
                framing::write_frame(&response, &mut self.outgoing);
                self.channel = Channel::AwaitingEncryptResult(encryption);
            }
            (Channel::AwaitingEncryptResult(encryption), EMsg::ChannelEncryptResult) => {
                let encryption = encryption.clone();
                check_encrypt_result(packet)?;
                self.channel = Channel::Encrypted(encryption);
                for message in mem::take(&mut self.unencrypted) {
                    self.transmit(&message);
                }
            }
            (channel, emsg) => debug!("Skipping {:?} during channel setup, in {:?}", emsg, channel),
        }
        Ok(())
    }

    /// Handles a message received, and the ones it carries if it is a `Multi`, in order.
    fn receive(&mut self, message: Bytes, now: Instant) {
        let mut next = Some(message);
        while let Some(raw) = next.take().or_else(|| self.unpacked.pop_front()) {
            // the header is parsed to route the message, which is kept whole
            if let Some(packet) = parse(raw.clone(), &mut self.unpacked) {
                self.handle_packet(RawMessage::new(packet.emsg(), raw), packet, now);
            }
        }
    }

    fn handle_packet(&mut self, message: RawMessage, packet: PacketMessage, now: Instant) {
        match packet.emsg() {
            EMsg::ServiceMethodResponse => {
                let job_id = packet.header().target();
                match self.calls.get_mut(&job_id) {
                    Some(answer) if matches!(answer, Answer::Waiting) => *answer = Answer::Answered(Ok(message)),
                    _ => {
                        debug!("Nothing waits for job {}", job_id);
                        self.push_event(Event::Message(message));
                    }
                }
            }
            EMsg::ClientLogOnResponse if matches!(self.logon, Logon::Pending) => match logon_result(packet) {
                Ok(info) => {
                    debug!(
                        "Logged on as {}, heartbeat every {:?}",
                        info.session.steam_id, info.heartbeat_interval
                    );
                    self.logon = Logon::LoggedOn {
                        info,
                        next_heartbeat: now + info.heartbeat_interval,
                    };
                }
                Err(refused) => self.end(refused),
            },
            EMsg::ClientLoggedOff => {
                let result = logged_off_result(packet);
                debug!("Steam logged us off: {:?}", result);
                self.end(Ended::LoggedOff(result));
                self.push_event(Event::LoggedOff(result));
            }
            _ => self.push_event(Event::Message(message)),
        }
    }

    /// Keeps an event. Past [`EVENT_QUEUE_CAPACITY`] from earlier reads, or [`MAX_EVENTS_PER_READ`] from this one,
    /// the oldest goes.
    fn push_event(&mut self, event: Event) {
        self.read_events += 1;
        let capacity = self.read_events.clamp(EVENT_QUEUE_CAPACITY, MAX_EVENTS_PER_READ);
        if self.events.len() >= capacity {
            self.events.pop_front();
            self.lagged += 1;
            debug!("The event queue is full, dropped the oldest ({} so far)", self.lagged);
        }
        self.events.push_back(event);
    }

    /// Ends the session: the calls waiting fail as `ended` says, and so will later ones. Answered calls keep their
    /// answer.
    fn end(&mut self, ended: Ended) {
        self.logon = Logon::Ended(ended);
        for answer in self.calls.values_mut() {
            if let Answer::Waiting = answer {
                *answer = Answer::Answered(Err(ended));
            }
        }
    }

    /// Takes no more input, and ends the session as `Dropped` unless it already ended.
    fn fail(&mut self) {
        self.channel = Channel::Failed;
        self.received = BytesMut::new();
        self.outgoing = BytesMut::new();
        self.message_lengths.clear();
        if !matches!(self.logon, Logon::Ended(_)) {
            self.end(Ended::Dropped);
        }
    }

    /// Serializes a message with a protobuf header, and sends it. Reuses the protocol's buffers: past the encryption,
    /// it allocates nothing once they have grown.
    fn send_proto<M: Message>(&mut self, emsg: EMsg, header: &CMsgProtoBufHeader, body: &M) {
        let mut serialized = mem::take(&mut self.serialized);
        serialized.clear();
        write_proto(emsg, header, body, &mut serialized);
        self.transmit(&serialized);
        self.serialized = serialized;
    }

    /// Frames and encrypts a serialized message for the channel, or keeps it until the channel is encrypted.
    fn transmit(&mut self, message: &[u8]) {
        match &self.channel {
            Channel::Encrypted(encryption) => framing::write_encrypted(message, encryption, &mut self.outgoing),
            Channel::WebSocket => {
                self.outgoing.extend_from_slice(message);
                self.message_lengths.push_back(message.len());
            }
            Channel::AwaitingEncryptRequest | Channel::AwaitingEncryptResult(_) => {
                self.unencrypted.push(message.to_vec())
            }
            Channel::Failed => debug!("Not sending a message, the connection failed"),
        }
    }
}

impl fmt::Debug for CmProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CmProtocol")
            .field("channel", &self.channel)
            .field("logon", &self.logon)
            .field("calls", &self.calls.len())
            .field("outgoing", &self.outgoing.len())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

/// Frees an empty queue that grew past [`QUEUE_KEPT_CAPACITY`].
pub(crate) fn free_if_big<T>(queue: &mut VecDeque<T>) {
    if queue.is_empty() && queue.capacity() > QUEUE_KEPT_CAPACITY {
        *queue = VecDeque::new();
    }
}

/// Hands out the job ids requests are matched to their responses with.
#[derive(Debug)]
struct JobIds(u64);

impl JobIds {
    fn new() -> Self {
        // `u64::MAX` means "no job" on the header
        Self(0)
    }

    fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}

/// Identifies the session on the header of the messages we send, once logged on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Session {
    pub(crate) steam_id: SteamID,
    pub(crate) session_id: i32,
}

impl Session {
    pub(crate) fn header(&self) -> CMsgProtoBufHeader {
        let mut header = CMsgProtoBufHeader::new();
        header.set_steamid(self.steam_id.to_steam64());
        header.set_client_sessionid(self.session_id);
        header
    }
}

/// Parses a raw message. A `Multi` is unpacked into `pending`, and messages that can't be parsed are logged.
///
/// Returns the message, unless it was one of those.
pub(crate) fn parse(raw: Bytes, pending: &mut VecDeque<Bytes>) -> Option<PacketMessage> {
    // for the log, as parsing consumes `raw`
    let raw_len = raw.len();
    let raw_emsg = raw
        .get(..4)
        .map(|emsg| u32::from_le_bytes([emsg[0], emsg[1], emsg[2], emsg[3]]));

    let packet = match PacketMessage::from_bytes(raw) {
        Ok(packet) => packet,
        Err(MessageError::EMsg(EMsgError::ValueNotFound(_))) => {
            let emsg = raw_emsg.unwrap_or_default() & EMSG_MASK;
            debug!("Skipping a message with unknown EMsg {} ({} bytes)", emsg, raw_len);
            return None;
        }
        Err(error) => {
            warn!("Skipping a message that could not be parsed: {}", error);
            return None;
        }
    };

    if packet.emsg() != EMsg::Multi {
        trace!("Received {:?}", packet.emsg());
        return Some(packet);
    }
    if !matches!(packet.header(), MessageHeaderWrapper::Proto(_)) {
        warn!("Skipping a Multi without a protobuf header");
        return None;
    }

    let multi = match ClientMessage::<CMsgMulti>::from_proto_packet(packet) {
        Ok(multi) => multi.body,
        Err(error) => {
            warn!("Skipping a Multi that could not be parsed: {}", error);
            return None;
        }
    };
    match unpack_multi(multi, pending) {
        Ok(unpacked) => trace!("Unpacked {} messages from a Multi", unpacked),
        Err(error) => warn!("Skipping a Multi that could not be unpacked: {}", error),
    }
    None
}

/// Serializes a message with a protobuf header.
pub(crate) fn proto_message<M: Message>(emsg: EMsg, header: CMsgProtoBufHeader, body: M) -> Vec<u8> {
    let mut message = ClientMessage::<M>::new_proto(emsg);
    message.wrapped_header = MessageHeaderWrapper::Proto(header);
    message.body = body;
    message.to_bytes()
}

/// Writes a message with a protobuf header to `out`, as [`proto_message`] serializes it, without a buffer of its own.
fn write_proto<M: Message>(emsg: EMsg, header: &CMsgProtoBufHeader, body: &M, out: &mut Vec<u8>) {
    out.extend_from_slice(&(emsg as u32 | PROTO_FLAG).to_le_bytes());
    out.extend_from_slice(&(header.compute_size() as u32).to_le_bytes());
    // only fail on missing required fields: the header has none, and requests are checked before they're sent
    header
        .write_to_vec(out)
        .expect("CMsgProtoBufHeader has no required fields");
    body.write_to_vec(out)
        .expect("messages sent have their required fields");
}

/// Checks a `ClientLogOnResponse`. Returns what Steam assigned, or why it refused.
fn logon_result(packet: PacketMessage) -> Result<LogonInfo, Ended> {
    let invalid = Ended::Refused {
        result: EResult::Invalid,
        extended: None,
    };
    let session = match packet.header() {
        MessageHeaderWrapper::Proto(header) => Session {
            steam_id: SteamID::from_steam64(header.steamid()),
            session_id: header.client_sessionid(),
        },
        // Steam sends it without a protobuf header when it got no `ClientLogon`
        _ => return Err(invalid),
    };
    let response = match ClientMessage::<CMsgClientLogonResponse>::from_proto_packet(packet) {
        Ok(response) => response.body,
        Err(error) => {
            warn!("Could not parse the logon response: {}", error);
            return Err(invalid);
        }
    };

    let result = eresult(response.eresult());
    if result != EResult::OK {
        return Err(Ended::Refused {
            result,
            extended: response.eresult_extended.map(eresult),
        });
    }
    Ok(LogonInfo {
        session,
        heartbeat_interval: heartbeat_interval(&response),
        cell_id: response.cell_id(),
        public_ip: response
            .public_ip
            .has_v4()
            .then(|| Ipv4Addr::from(response.public_ip.v4())),
    })
}

fn heartbeat_interval(response: &CMsgClientLogonResponse) -> Duration {
    // SteamKit uses the out of game interval
    let seconds = match response.legacy_out_of_game_heartbeat_seconds() {
        seconds if seconds > 0 => seconds,
        _ => response.heartbeat_seconds(),
    };
    Duration::from_secs(seconds.max(1) as u64)
}

fn logged_off_result(packet: PacketMessage) -> EResult {
    match ClientMessage::<CMsgClientLoggedOff>::from_proto_packet(packet) {
        Ok(message) => eresult(message.body.eresult()),
        Err(_) => EResult::Invalid,
    }
}

pub(crate) fn eresult(value: i32) -> EResult {
    EResult::from_i32(value).unwrap_or_else(|| {
        warn!("Unknown EResult {}", value);
        EResult::Invalid
    })
}

//! Not part of the API: the receive path, for `tests/zero_alloc.rs` to drive without a socket.

use std::collections::VecDeque;
use std::time::Instant;

use bytes::Bytes;
use bytes::BytesMut;
use steam_cryptography::SessionKey;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::messages::HasEMsg;
use steam_language_gen::MessageBodyExt;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;
use steam_language_gen::SerializableBytes;
use steam_protobuf::protobuf::Message;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

use crate::client::account::Logon;
use crate::client::Anonymous;
use crate::connection::Connection;
use crate::errors::ClientError;
use crate::errors::PacketError;
use crate::events::Event;
use crate::gc::GcMessage;
use crate::handle;
use crate::handle::Events;
use crate::handle::SteamHandle;
use crate::handle::SteamTask;
use crate::job::JobRequest;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::protocol;
use crate::protocol::framing;
use crate::protocol::CmProtocol;
use crate::protocol::LogonInfo;
use crate::protocol::Transport;
use crate::service::ServiceRequest;

/// Decodes a plain frame, as during the encryption handshake.
pub fn decode_plain(src: &mut BytesMut) -> Result<Option<Bytes>, PacketError> {
    Ok(framing::decode(src)?.map(BytesMut::freeze))
}

/// Decodes encrypted frames, as the connection does once the channel is encrypted.
#[derive(Debug)]
pub struct EncryptedCodec(SessionKey);

impl EncryptedCodec {
    pub fn new(key: SessionKey) -> Self {
        Self(key)
    }

    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Bytes>, PacketError> {
        framing::decode(src)?
            .map(|frame| framing::decrypt(frame, &self.0))
            .transpose()
    }
}

/// The CM protocol, which the connection feeds what it reads.
#[derive(Debug)]
pub struct Protocol(CmProtocol);

impl Protocol {
    /// Over TCP, with the channel already encrypted with `key`.
    pub fn encrypted(key: SessionKey) -> Self {
        Self(CmProtocol::encrypted(key))
    }

    /// Over WebSocket: every input is one message, neither framed nor encrypted.
    pub fn websocket() -> Self {
        Self(CmProtocol::new(Transport::WebSocket))
    }

    pub fn handle_input(&mut self, data: &[u8], now: Instant) -> Result<(), ClientError> {
        self.0.handle_input(data, now)
    }

    /// Calls a service method. Returns its job id.
    pub fn call<R: ServiceRequest>(&mut self, request: R) -> u64 {
        self.0.call::<R::Method>(request).job_id()
    }

    /// Sends a job message. Returns its job id.
    pub fn call_job<R: JobRequest>(&mut self, request: R) -> u64 {
        self.0.call_job::<R::Job>(request).job_id()
    }

    /// The response to job `job_id`, unparsed, once it came.
    pub fn take_response(&mut self, job_id: u64) -> Option<Result<Packet, ClientError>> {
        self.0.take_answer(job_id).map(|answer| {
            answer
                .map_err(ClientError::from)
                .and_then(|message| Ok(Packet(message.parse()?)))
        })
    }

    /// Sends a message, as the client does.
    pub fn send<M: Message>(&mut self, emsg: EMsg, body: M) {
        self.0.send(emsg, body)
    }

    /// Takes what there is to write.
    pub fn take_transmits(&mut self) -> usize {
        std::iter::from_fn(|| self.0.poll_transmit())
            .map(|transmit| transmit.len())
            .sum()
    }

    /// The next game coordinator's message, read as a spawned client reads it to route it, skipping other events
    /// and messages.
    pub fn poll_gc_message(&mut self) -> Option<Result<GcMessage, ClientError>> {
        loop {
            if let Event::Message(message) = self.0.poll_event()? {
                if let Some(gc_message) = message.gc_message() {
                    return Some(gc_message);
                }
            }
        }
    }

    /// The next message nothing waited for, skipping other events.
    pub fn poll_message(&mut self) -> Option<Packet> {
        loop {
            if let Event::Message(message) = self.0.poll_event()? {
                return message.parse().ok().map(Packet);
            }
        }
    }
}

/// A packet: its `EMsg` and header parsed, its body not yet.
#[derive(Debug)]
pub struct Packet(PacketMessage);

impl Packet {
    /// Parses a packet, as during the encryption handshake.
    pub fn from_bytes(raw: Bytes) -> Result<Self, MessageError> {
        PacketMessage::from_bytes(raw).map(Self)
    }

    /// Parses a packet as the encrypted connection does: a `Multi` is unpacked into `pending`, and messages that
    /// can't be parsed are skipped.
    pub fn parse(raw: Bytes, pending: &mut VecDeque<Bytes>) -> Option<Self> {
        protocol::parse(raw, pending).map(Self)
    }

    pub fn emsg(&self) -> EMsg {
        self.0.emsg()
    }

    pub fn header(&self) -> &MessageHeaderWrapper {
        self.0.header()
    }

    /// Parses a body with a fixed layout. Returns it with the payload that follows it.
    pub fn parse_body<T: MessageBodyExt + HasEMsg + SerializableBytes>(self) -> Result<(T, Bytes), MessageError> {
        let message = ClientMessage::<T>::from_packet_message(self.0)?;
        Ok((message.body, message.payload))
    }

    /// Parses a protobuf body.
    pub fn parse_proto<M: Message>(self) -> Result<M, MessageError> {
        ClientMessage::<M>::from_proto_packet(self.0).map(|message| message.body)
    }
}

/// A connection over `stream`, its channel already encrypted with `key`, as the client sends through it.
#[derive(Debug)]
pub struct EncryptedConnection<T>(Connection<T>);

impl<T: AsyncRead + AsyncWrite + Unpin> EncryptedConnection<T> {
    pub fn new(stream: T, key: SessionKey) -> Self {
        Self(Connection::new(stream, CmProtocol::encrypted(key)))
    }

    /// Sends a message, and writes it.
    pub async fn send<M: Message>(&mut self, emsg: EMsg, body: M) -> Result<(), ClientError> {
        self.0.send(emsg, body).await
    }
}

/// A client logged on anonymously over `stream`, its channel already encrypted with `key`, used without a task, as
/// `SteamClient<LoggedOn<Anonymous>>` is.
#[derive(Debug)]
pub struct LoggedOnConnection<T> {
    connection: Connection<T>,
    logon: LogonInfo,
}

impl<T: AsyncRead + AsyncWrite + Unpin> LoggedOnConnection<T> {
    pub async fn log_on(stream: T, key: SessionKey) -> Result<Self, ClientError> {
        let mut connection = Connection::new(stream, CmProtocol::encrypted(key));
        let (steam_id, logon) = Anonymous.logon();
        let logon = connection.log_on(steam_id, logon).await?;
        Ok(Self { connection, logon })
    }

    /// Spawns it, as `SteamClient::spawn` does. Connecting again fails.
    pub fn spawn(self) -> (SteamHandle<Anonymous>, Events, SteamTask)
    where
        T: Send + 'static,
    {
        handle::spawn(
            self.connection,
            String::new(),
            self.logon,
            Anonymous,
            handle::connector(|_| Box::pin(async { Err(ClientError::NoServerAvailable) })),
        )
    }
}

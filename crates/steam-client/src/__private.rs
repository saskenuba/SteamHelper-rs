//! Not part of the API: the receive path, for `tests/zero_alloc.rs` to drive without a socket.

use std::collections::VecDeque;

use bytes::Bytes;
use bytes::BytesMut;
use steam_crypto::SessionKey;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::messages::HasEMsg;
use steam_language_gen::MessageBodyExt;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;
use steam_language_gen::SerializableBytes;
use steam_protobuf::protobuf::Message;
use tokio_util::codec::Decoder;

use crate::connection;
use crate::errors::PacketError;
use crate::messages::codec;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;

/// Decodes a plain frame, as during the encryption handshake.
pub fn decode_plain(src: &mut BytesMut) -> Result<Option<Bytes>, PacketError> {
    Ok(codec::FrameCodec.decode(src)?.map(BytesMut::freeze))
}

/// Decodes encrypted frames, as the connection does once the channel is encrypted.
#[derive(Debug)]
pub struct EncryptedCodec(codec::EncryptedCodec);

impl EncryptedCodec {
    pub fn new(key: SessionKey) -> Self {
        Self(codec::EncryptedCodec::new(key))
    }

    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Bytes>, PacketError> {
        self.0.decode(src)
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
        connection::parse(raw, pending).map(Self)
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

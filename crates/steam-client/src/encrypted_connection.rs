use std::fmt::Formatter;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::Serialize;

use steam_crypto::{generate_encrypt_request_handshake, generate_session_key};
use steam_language_gen::{
    generated::{
        enums::EMsg,
        headers::{ExtendedMessageHeader, MessageHeaders, StandardMessageHeader},
        messages::{
            HasEMsg, MsgChannelEncryptRequest, MsgChannelEncryptResponse, MsgClientChatAction,
            MsgClientNewLoginKeyAccepted,
        },
    },
    DeserializableBytes, MessageBodyExt, MessageHeader, MessageHeaderExt, MessageHeaderWrapper, SerializableBytes,
};

use crate::messages::packetmessage::PacketMessage;

pub(crate) fn handle_encrypt_request(message: PacketMessage) -> Vec<u8> {
    // let msg = MsgChannelEncryptRequest::from_bytes(message_contents);
    // let (_, payload) = MsgChannelEncryptRequest::split_from_bytes(message_contents);
    let incoming_message: Msg<MsgChannelEncryptRequest> = Msg::from_packet_message(message);

    // this would be used for selecting the appropriate public key.. but are using only the key for
    // the Public Universe
    let connected_universe = incoming_message.msg_type.universe;
    let protocol_version = incoming_message.msg_type.protocol_version;

    println!(
        "Got encryption request. Universe: {:?} Protocol Version {:?}",
        connected_universe, protocol_version
    );

    let mut random_challenge = BytesMut::with_capacity(1024);

    let payload = incoming_message.payload();
    if incoming_message.payload.len() >= 16 {
        random_challenge.put(payload);
    }

    let (session_keys, encrypted_payload) = generate_encrypt_request_handshake(&*random_challenge);

    // last message source is now our target.. dunno yet about our source, maybe last message target?
    let target = incoming_message.header.target();
    let source = incoming_message.header.source();

    let reply_message: Msg<MsgChannelEncryptResponse> =
        Msg::new().set_target(target).set_payload(encrypted_payload.as_ref());

    println!("Incoming message: {}.", incoming_message);
    println!(
        "Answering with: {} Payload {:?}.",
        reply_message,
        &reply_message.payload()[..128]
    );
    reply_message.to_bytes()
}

#[derive(Clone, Debug, Serialize)]
struct Msg<M: SerializableBytes + HasEMsg> {
    emsg: EMsg,
    header: MessageHeaderWrapper,
    msg_type: M,
    payload: Vec<u8>,
}

impl<M: std::fmt::Debug + SerializableBytes + HasEMsg> std::fmt::Display for Msg<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Message: {:?}, Target ID: {:?} Source ID: {:?} Payload size {} bytes.",
            self.msg_type,
            self.header.target(),
            self.header.source(),
            self.payload.len()
        )
    }
}

impl<M: HasEMsg + SerializableBytes> SerializableBytes for Msg<M> {
    fn to_bytes(&self) -> Vec<u8> {
        let mut output_buffer = BytesMut::with_capacity(1024);
        let emsg = self.emsg as u32;

        output_buffer.extend(&emsg.to_le_bytes());
        output_buffer.extend(self.header.to_bytes());
        output_buffer.extend(self.msg_type.to_bytes());
        output_buffer.extend(self.payload.as_slice());
        output_buffer.freeze().to_vec()
    }
}

const DEFAULT_MESSAGE_MAX_SIZE: usize = 1024;

impl<T: MessageBodyExt + HasEMsg + SerializableBytes + DeserializableBytes> Msg<T> {
    /// Used to decode incoming messages
    fn from_packet_message(msg: PacketMessage) -> Self {
        let (message_bytes, incoming_message_payload) = T::split_from_bytes(msg.payload());
        let incoming_message = T::from_bytes(message_bytes);

        // since we already have the correct header from the packet message, we dont need to look up
        // for it again..
        let header_kind = MessageHeaders::header_from_emsg(T::emsg()).unwrap();
        let incoming_message_header = msg.header();
        match header_kind {
            MessageHeaders::Standard => Self {
                emsg: T::emsg(),
                header: incoming_message_header,
                msg_type: incoming_message,
                payload: incoming_message_payload.to_vec(),
            },
            MessageHeaders::Extended => Self {
                emsg: T::emsg(),
                header: incoming_message_header,
                msg_type: incoming_message,
                payload: incoming_message_payload.to_vec(),
            },
        }
    }

    /// Used to build replies to Steam3
    fn new() -> Self {
        let header_kind = MessageHeaders::header_from_emsg(T::emsg()).unwrap();

        match header_kind {
            MessageHeaders::Standard => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Std(StandardMessageHeader::create()),
                msg_type: T::create(),
                payload: Vec::with_capacity(DEFAULT_MESSAGE_MAX_SIZE),
            },
            MessageHeaders::Extended => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Ext(ExtendedMessageHeader::create()),
                msg_type: T::create(),
                payload: Vec::with_capacity(DEFAULT_MESSAGE_MAX_SIZE),
            },
        }
    }

    fn set_target(mut self, target: u64) -> Self {
        self.header.set_target(target);
        self
    }
    fn set_payload(mut self, payload: &[u8]) -> Self {
        self.payload = payload.to_vec();
        self
    }
}

impl<C: HasEMsg + SerializableBytes> MessageKind for Msg<C> {
    fn payload(&self) -> &[u8] {
        &self.payload
    }
}

pub(crate) trait MessageKind {
    fn payload(&self) -> &[u8];
}

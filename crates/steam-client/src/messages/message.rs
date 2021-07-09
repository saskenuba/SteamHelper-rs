//! Message Module
//!
//!
//!
//!
//!
//! Check link below for more info:
//! https://github.com/ValvePython/steam/blob/09f4f51a287ee7aec1f159c7e8098add5f14bed3/steam/core/msg/headers.py

use std::fmt::Formatter;

use bytes::BytesMut;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::headers::{ExtendedMessageHeader, MessageHeaders, StandardMessageHeader};
use steam_language_gen::generated::messages::HasEMsg;
use steam_language_gen::{
    DeserializableBytes, HasJobId, MessageBodyExt, MessageHeaderExt, MessageHeaderWrapper, SerializableBytes,
};
use steam_protobuf::steam::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::Message;

use crate::messages::packet::PacketMessage;
use crate::messages::MessageKind;

//  if message is proto: emsg_enum, raw_data from packet
// new MessageHeaderProtobuf
// steammessages_base_pb2. CMSGProtobufHeader

//  if not proto: emsg_enum, raw_data from packet -> extended
// novo  ExtendedMessageHeader

#[derive(Clone, Debug)]
/// A unified interface for interacting with Steam Client.
///
/// This type wraps:
/// Protobuf Header and protobuf message body;
/// Standard Header and body for encryption purposes;
/// Extended Header, body and payload;
pub struct ClientMessage<M> {
    /// The vinculated `EMsg` of this `ClientMessage`.
    // FIXME: Should be HasEMsg trait
    pub emsg: EMsg,
    /// A client message header wrapped in `MessageHeaderWrapper`.
    pub header: MessageHeaderWrapper,

    /// The body of a message.
    /// Can be Protobuf backed message, or a simple body.
    pub body: M,

    /// Apparently, Protobuf messages don't have additional payload.
    /// This is only applicable if we are dealing with messages with Standard and Extended Headers.
    payload: Vec<u8>,
}

impl<M> ClientMessage<M> {
    pub fn proto_header(&self) -> Option<&CMsgProtoBufHeader> {
        self.header.proto_header()
    }
}

impl<M> ClientMessage<M>
where
    M: Message,
{
    pub fn new_proto(emsg: EMsg) -> Self {
        Self {
            emsg,
            header: MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()),
            body: M::new(),
            payload: vec![],
        }
    }
}

impl<M: std::fmt::Debug + HasEMsg> std::fmt::Display for ClientMessage<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Message: {:?}, Target ID: {:?} Source ID: {:?} Payload size {} bytes.",
            self.body,
            self.header.target(),
            self.header.source(),
            self.payload.len()
        )
    }
}

impl<M> SerializableBytes for ClientMessage<M>
where
    M: SerializableBytes,
{
    fn to_bytes(&self) -> Vec<u8> {
        let mut output_buffer = BytesMut::with_capacity(1024);
        let emsg = self.emsg as u32;

        output_buffer.extend(&emsg.to_le_bytes());
        output_buffer.extend(self.header.to_bytes());
        output_buffer.extend(self.body.to_bytes());
        output_buffer.extend(self.payload.as_slice());
        output_buffer.freeze().to_vec()
    }
}

const DEFAULT_MESSAGE_MAX_SIZE: usize = 1024;

impl<T: MessageBodyExt + DeserializableBytes> ClientMessage<T> {
    /// Used to decode incoming messages
    pub(crate) fn from_packet_message(msg: PacketMessage) -> Self {
        let header = msg.header();
        let emsg = msg.emsg();
        let message = T::from_bytes(msg.payload());

        let (_, message_payload_bytes) = T::split_from_bytes(msg.payload());

        Self {
            emsg,
            header,
            body: message,
            payload: message_payload_bytes.to_vec(),
        }
    }
}

impl<T: HasEMsg> ClientMessage<T> {
    /// Used to build replies to Steam3
    pub(crate) fn new() -> Self {
        let header_kind = MessageHeaders::header_from_emsg(T::emsg()).unwrap();

        match header_kind {
            MessageHeaders::Standard => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Std(StandardMessageHeader::create()),
                body: T::create(),
                payload: Vec::with_capacity(DEFAULT_MESSAGE_MAX_SIZE),
            },
            MessageHeaders::Extended => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Ext(ExtendedMessageHeader::create()),
                body: T::create(),
                payload: Vec::with_capacity(DEFAULT_MESSAGE_MAX_SIZE),
            },
        }
    }
}

impl<C> MessageKind for ClientMessage<C> {
    fn set_payload(&mut self, payload: &[u8]) {
        self.payload = payload.to_vec()
    }

    fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl<C: 'static> HasJobId for ClientMessage<C> {
    fn set_target(&mut self, new_target: u64) {
        self.header.set_target(new_target);
    }

    fn set_source(&mut self, new_source: u64) {
        self.header.set_source(new_source);
    }

    fn source(&self) -> u64 {
        self.header.source()
    }

    fn target(&self) -> u64 {
        self.header.target()
    }
}

#[cfg(test)]
mod tests {
    use steam_language_gen::generated::enums::{EMsg, EUniverse};
    use steam_language_gen::generated::headers::{ExtendedMessageHeader, StandardMessageHeader};
    use steam_language_gen::generated::messages::{MsgChannelEncryptRequest, MsgClientChatEnter};
    use steam_language_gen::{DeserializableBytes, MessageHeaderExt, SerializableBytes};

    /// ChannelEncryptRequest
    /// This has standard header
    fn get_channel_encrypt_request() -> Vec<u8> {
        let on_connection_packet = vec![
            23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1,
            0, 0, 0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
        ];
        on_connection_packet
    }

    /// ClientChatEnter, EMsg(807)
    fn get_client_chat_enter() -> Vec<u8> {
        let struct_msg_data = vec![
            0x27, 0x03, 0x00, 0x00, 0x24, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xEF, 0xAC, 0x15, 0x89, 0x00, 0x01, 0x00, 0x10, 0x01, 0x8E, 0x56, 0x11, 0x00,
            0xBC, 0x4E, 0x2A, 0x00, 0x00, 0x00, 0x88, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0xBC, 0x4E, 0x2A, 0x00, 0x00, 0x00, 0x70, 0x01, 0xBC, 0x4E, 0x2A, 0x00, 0x00, 0x00, 0x70, 0x01,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x53, 0x61, 0x78, 0x74, 0x6F, 0x6E, 0x20, 0x48, 0x65,
            0x6C, 0x6C, 0x00, 0x00, 0x4D, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00,
            0x07, 0x73, 0x74, 0x65, 0x61, 0x6D, 0x69, 0x64, 0x00, 0xAC, 0x15, 0x89, 0x00, 0x01, 0x00, 0x10, 0x01, 0x02,
            0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00, 0x7B, 0x03, 0x00, 0x00, 0x02, 0x44,
            0x65, 0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x4D, 0x65, 0x73, 0x73,
            0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D, 0x69, 0x64,
            0x00, 0x00, 0x28, 0x90, 0x00, 0x01, 0x00, 0x10, 0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73, 0x69,
            0x6F, 0x6E, 0x73, 0x00, 0x08, 0x00, 0x00, 0x00, 0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x04,
            0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x4D, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63,
            0x74, 0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D, 0x69, 0x64, 0x00, 0xB0, 0xDC, 0x5B, 0x04, 0x01, 0x00, 0x10,
            0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00, 0x08, 0x00, 0x00, 0x00,
            0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x4D, 0x65,
            0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D,
            0x69, 0x64, 0x00, 0x39, 0xCB, 0x77, 0x05, 0x01, 0x00, 0x10, 0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69, 0x73,
            0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00, 0x1A, 0x03, 0x00, 0x00, 0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73,
            0x00, 0x02, 0x00, 0x00, 0x00, 0x08, 0x08, 0xE8, 0x03, 0x00, 0x00,
        ];
        struct_msg_data
    }

    #[test]
    fn deserialize_client_chat_enter() {
        let message = get_client_chat_enter();

        let emsg = EMsg::from_raw_message(&message).unwrap();
        let message_complete = EMsg::strip_message(&message);
        let (header, message): (&[u8], &[u8]) = ExtendedMessageHeader::split_from_bytes(message_complete);

        assert_eq!(EMsg::ClientChatEnter, emsg);

        let msg = MsgClientChatEnter::from_bytes(message);
        println!(": {:#?}", msg);
    }

    #[test]
    fn deserialize_msg_encrypt_request() {
        let message = b"\x17\x05\x00\x00\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\
        \xff\xff\xff\xff\x01\x00\x00\x00\x01\x00\x00\x00"
            .to_vec();

        let emsg = EMsg::from_raw_message(&message).unwrap();
        let message_complete = EMsg::strip_message(&message);
        let (header, message): (&[u8], &[u8]) = StandardMessageHeader::split_from_bytes(message_complete);
        let msgheader_default: StandardMessageHeader = StandardMessageHeader::new();

        assert_eq!(EMsg::ChannelEncryptRequest, emsg);
        assert_eq!(msgheader_default.to_bytes(), header);
        assert_eq!(StandardMessageHeader::from_bytes(header), msgheader_default);

        let msg = MsgChannelEncryptRequest {
            protocol_version: 1,
            universe: EUniverse::Public,
        };
        assert_eq!(MsgChannelEncryptRequest::from_bytes(message), msg);
    }
}

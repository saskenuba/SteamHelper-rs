use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::Serialize;

use steam_crypto::generate_session_key;
use steam_language_gen::{DeserializableBytes, generated::{
    enums::EMsg,
    headers::{
        ExtendedMessageHeader,
        MessageHeaders,
        ProtobufMessageHeader,
        StandardMessageHeader,
    },
    messages::{
        HasEMsg,
        MsgChannelEncryptRequest,
        MsgChannelEncryptResponse,
        MsgClientChatAction,
        MsgClientNewLoginKeyAccepted,
    },
}, MessageBodyExt, MessageHeader, MessageHeaderExt, MessageHeaderWrapper, SerializableBytes};

use crate::messages::packetmessage::PacketMessage;

pub(crate) fn handle_encrypt_request(message: PacketMessage) {
    let message_contents = message.data();

    // dar um jeito nesse split, não é preocupação ficar fazendo isso.. tem que ser encapsulado os detalhes
    // pode-se trabalhar na interface Msg para realizar esta abstracao
    let msg: MsgChannelEncryptRequest = MsgChannelEncryptRequest::from_bytes(message_contents);
    let (_, payload) = MsgChannelEncryptRequest::split_from_bytes(message_contents);

    let connected_universe = msg.clone().universe;
    let protocol_version = msg.protocol_version;

    debug!("Got encryption request. Universe: {:?} Protocol Version {:?}",
           connected_universe, protocol_version);

    let mut random_challenge = BytesMut::with_capacity(1024);

    // if message is larger than 16 bytes, we
    if payload.len() >= 16 {
        random_challenge.put(payload);
    }

    let temp_session_key = generate_session_key(random_challenge.bytes());
    trace!("Plain key: {:?}", temp_session_key.plain_text);
    trace!("Encrypted key: {:?}", temp_session_key.encrypted);

    // last message source is now our target.. dunno yet about our source, maybe last message target?
    let target = *message.jobs_ids().1;
    let source = *message.jobs_ids().0;

    let reply_message: Msg<MsgChannelEncryptResponse> = Msg::new_with_target(
        message,
        target,
    );
    debug!("Incoming message: {:?} Target: {} Source: {} Payload {} bytes.", msg, target, source, payload.len());
    debug!("Answering with: {:?} Header {:?} bytes: {:?}", reply_message.msg_type, reply_message.header, reply_message.to_bytes());
}

#[derive(Clone, Debug, Serialize)]
struct Msg<M: SerializableBytes + HasEMsg> {
    emsg: EMsg,
    header: MessageHeaderWrapper,
    msg_type: M,
    payload: Vec<u8>,
}

impl<M: HasEMsg + SerializableBytes> SerializableBytes for Msg<M> {
    fn to_bytes(&self) -> Vec<u8> {
        let mut output_buffer = BytesMut::with_capacity(1024);
        let emsg = self.emsg.clone() as u32;

        output_buffer.extend(&emsg.to_le_bytes());
        output_buffer.extend(self.header.to_bytes());
        output_buffer.extend(self.msg_type.to_bytes());
        output_buffer.extend(self.payload.as_slice());
        output_buffer.freeze().to_vec()
    }
}

impl<T: MessageBodyExt + HasEMsg + SerializableBytes> Msg<T> {
    fn new(msg: PacketMessage) -> Self {
        let header_kind = MessageHeaders::header_from_emsg(T::emsg()).unwrap();
        let (_, payload) = T::split_from_bytes(msg.data());

        match header_kind {
            MessageHeaders::Standard => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Std(StandardMessageHeader::create()),
                msg_type: T::create(),
                payload: payload.to_vec(),
            },
            MessageHeaders::Extended => Self {
                emsg: T::emsg(),
                header: MessageHeaderWrapper::Ext(ExtendedMessageHeader::create()),
                msg_type: T::create(),
                payload: payload.to_vec(),
            },
        }
    }
    fn new_with_target(msg: PacketMessage, target: u64) -> Self {
        let mut msg = Self::new(msg);
        msg.header.set_target(target);
        msg
    }
}



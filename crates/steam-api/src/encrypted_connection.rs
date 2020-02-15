use bytes::{Buf, BufMut, BytesMut};

use steam_crypto::generate_session_key;
use steam_language_gen::{
    generated::enums::EMsg,
    generated::headers::{SerializableMessageHeader, StandardMessageHeader},
    generated::messages::MsgChannelEncryptRequest,
    generated::messages::MsgChannelEncryptResponse,
    generated::messages::SerializableMessageBody,
};

use crate::messages::packetmessage::PacketMessage;

pub(crate) fn handle_encrypt_request(message: PacketMessage) {
    let message_contents = message.data();
    let msg: MsgChannelEncryptRequest = MsgChannelEncryptRequest::from_bytes(message_contents);

    let connected_universe = msg.universe;
    let protocol_version = msg.protocol_version;

    debug!("Got encryption request. Universe: {:?} Protocol Version {:?}",
           connected_universe, protocol_version);

    let mut random_challenge = BytesMut::with_capacity(1024);

    // if message is larger than 16 bytes, we
    if message_contents.len() >= 16 {
        random_challenge.put(message_contents);
    }

    let temp_session_key = generate_session_key(random_challenge.bytes());
    trace!("Plain key: {:?}", temp_session_key.plain_text);
    trace!("Encrypted key: {:?}", temp_session_key.encrypted);

    let response = MsgChannelEncryptResponse::new();
    debug!("{:?}", response);


    // last message source is now our target.. dunno yet about our source, maybe last message target?
    Msg::new(StandardMessageHeader::new(), response);
}

struct Msg<H, M> {
    emsg: EMsg,
    header: H,
    msg_type: M,
}

impl<T, C> Msg<T, C> {
    fn new(header: T, msg_type: C) -> Self
        where T: SerializableMessageHeader, C: SerializableMessageBody
    {
        Self {
            emsg: EMsg::Invalid,
            header,
            msg_type,
        }
    }
}
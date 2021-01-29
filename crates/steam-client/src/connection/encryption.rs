use bytes::{BufMut, BytesMut};

use steam_crypto::generate_encrypt_request_handshake;
use steam_language_gen::generated::messages::{MsgChannelEncryptRequest, MsgChannelEncryptResponse};
use steam_language_gen::{MessageHeader, MessageHeaderExt, SerializableBytes};

use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::messages::MessageKind;

pub(crate) fn handle_encrypt_request(message: PacketMessage) -> Vec<u8> {
    let incoming_message: ClientMessage<MsgChannelEncryptRequest> = ClientMessage::from_packet_message(message);

    let connected_universe = incoming_message.body.universe;
    let protocol_version = incoming_message.body.protocol_version;

    println!(
        "Got encryption request. Universe: {:?} Protocol Version {:?}",
        connected_universe, protocol_version
    );

    let mut random_challenge = BytesMut::with_capacity(1024);

    let payload = incoming_message.payload();
    if incoming_message.payload().len() >= 16 {
        random_challenge.put(payload);
    }

    let (session_keys, encrypted_payload) = generate_encrypt_request_handshake(&*random_challenge);

    // last message source is now our target.. dunno yet about our source, maybe last message target?
    let target = incoming_message.wrapped_header.target();
    let source = incoming_message.wrapped_header.source();

    let reply_message: ClientMessage<MsgChannelEncryptResponse> =
        ClientMessage::new().set_target(target).set_payload(encrypted_payload.as_ref());

    println!("Incoming message: {}.", incoming_message);
    println!(
        "Answering with: {} Payload {:?}.",
        reply_message,
        &reply_message.payload()[..128]
    );
    reply_message.to_bytes()
}

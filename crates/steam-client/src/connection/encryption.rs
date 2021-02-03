use atomic::{Atomic, Ordering};
use bytes::{BufMut, BytesMut};
use steam_crypto::generate_encrypt_request_handshake;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::messages::{
    MsgChannelEncryptRequest, MsgChannelEncryptResponse, MsgChannelEncryptResult,
};
use steam_language_gen::{HasJobId, SerializableBytes};

use crate::connection::{BytesTx, EncryptionState};
use crate::errors::PacketError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::messages::MessageKind;

pub(crate) fn handle_encryption_negotiation(
    tx: BytesTx,
    conn_encryption_state: &mut Atomic<EncryptionState>,
    message: PacketMessage,
) -> anyhow::Result<()> {
    // asddassdj
    match message.emsg() {
        EMsg::ChannelEncryptRequest => {
            let encrypt_response = handle_encrypt_request(message).to_bytes();

            println!("req current state: {:?}", conn_encryption_state);
            conn_encryption_state.swap(EncryptionState::Challenged, Ordering::SeqCst);
            tx.send(encrypt_response)
                .map_err::<PacketError, _>(|_| PacketError::Malformed)?;
        }
        EMsg::ChannelEncryptResult => {
            println!("result matched current state: {:?}", conn_encryption_state);
            conn_encryption_state.swap(EncryptionState::Encrypted, Ordering::SeqCst);
            handle_encrypt_result(message).unwrap();
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn handle_encrypt_result(message: PacketMessage) -> anyhow::Result<()> {
    let incoming_message: ClientMessage<MsgChannelEncryptResult> = ClientMessage::from_packet_message(message);
    println!("{:?}", incoming_message.body.result);

    Ok(())
}

pub(crate) fn handle_encrypt_request(message: PacketMessage) -> ClientMessage<MsgChannelEncryptResponse> {
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

    let mut reply_message: ClientMessage<MsgChannelEncryptResponse> = ClientMessage::new();
    reply_message.set_target(target);
    reply_message.set_payload(encrypted_payload.as_ref());

    println!("Incoming message: {}.", incoming_message);
    println!(
        "Answering with: {} Payload {:?}.",
        reply_message,
        &reply_message.payload()[..128]
    );
    reply_message
}

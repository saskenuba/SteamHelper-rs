//! Channel encryption handshake.
//!
//! Steam sends a `ChannelEncryptRequest` with a random nonce. We answer with a `ChannelEncryptResponse` carrying a new
//! session key, encrypted with the universe's public key. Steam confirms with a `ChannelEncryptResult`, and every
//! later message is encrypted with the session key.
//!
//! Reference: https://github.com/SteamRE/SteamKit/blob/master/SteamKit2/SteamKit2/Networking/Steam3/EnvelopeEncryptedConnection.cs

use steam_cryptography::generate_encrypt_request_handshake;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::generated::messages::MsgChannelEncryptRequest;
use steam_language_gen::generated::messages::MsgChannelEncryptResponse;
use steam_language_gen::generated::messages::MsgChannelEncryptResult;
use steam_language_gen::SerializableBytes;

use crate::errors::ClientError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::messages::MessageKind;
use crate::protocol::framing::Encryption;

/// Size of the nonce Steam sends, which selects the HMAC variant of the session encryption.
const NONCE_SIZE: usize = 16;

/// Answers a `ChannelEncryptRequest`. Returns how the channel will be encrypted, and the `ChannelEncryptResponse` to
/// send.
///
/// As SteamKit: with a nonce of 16 bytes or more, the session key goes with the nonce, and messages are encrypted with
/// HMAC IVs; with a shorter one, the key goes alone, and messages are encrypted plainly.
pub(crate) fn answer_encrypt_request(message: PacketMessage) -> Result<(Encryption, Vec<u8>), ClientError> {
    let request: ClientMessage<MsgChannelEncryptRequest> = ClientMessage::from_packet_message(message)?;
    // `body` is a packed struct, so the fields are copied out instead of borrowed.
    let universe = request.body.universe;
    let protocol_version = request.body.protocol_version;
    debug!(
        "Got encryption request. Universe: {:?} Protocol version: {}",
        universe, protocol_version
    );

    // steam-cryptography only has the public universe's key
    if universe != EUniverse::Public {
        return Err(ClientError::UnsupportedUniverse(universe));
    }
    let nonce = request.payload();
    let (encryption, encrypted_payload) = if nonce.len() >= NONCE_SIZE {
        let (key, payload) = generate_encrypt_request_handshake(nonce)?;
        (Encryption::HmacIv(key), payload)
    } else {
        warn!(
            "Encryption request has a nonce of {} bytes, expected {}: encrypting without HMAC.",
            nonce.len(),
            NONCE_SIZE
        );
        let (key, payload) = generate_encrypt_request_handshake(&[])?;
        (Encryption::Plain(key), payload)
    };
    let response: ClientMessage<MsgChannelEncryptResponse> = ClientMessage::new().set_payload(&encrypted_payload);
    Ok((encryption, response.to_bytes()))
}

/// Checks the `ChannelEncryptResult`: from then on the channel is encrypted.
pub(crate) fn check_encrypt_result(message: PacketMessage) -> Result<(), ClientError> {
    let message: ClientMessage<MsgChannelEncryptResult> = ClientMessage::from_packet_message(message)?;
    let result = message.body.result;
    debug!("Encryption result: {:?}", result);

    match result {
        EResult::OK => Ok(()),
        result => Err(ClientError::EncryptionFailed(result)),
    }
}

#[cfg(test)]
mod tests {
    use steam_language_gen::generated::enums::EMsg;

    use super::*;

    /// ChannelEncryptRequest captured from a CM.
    const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
        23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0,
        0, 0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
    ];

    fn encrypt_result(result: EResult) -> PacketMessage {
        let mut raw = (EMsg::ChannelEncryptResult as u32).to_le_bytes().to_vec();
        raw.extend_from_slice(&[0xFF; 16]);
        raw.extend_from_slice(&(result as u32).to_le_bytes());
        PacketMessage::from_raw_bytes(&raw).unwrap()
    }

    #[test]
    fn answers_the_encrypt_request() {
        let request = PacketMessage::from_raw_bytes(&CHANNEL_ENCRYPT_REQUEST).unwrap();
        let (encryption, response) = answer_encrypt_request(request).unwrap();
        assert!(matches!(encryption, Encryption::HmacIv(_)));

        let response = PacketMessage::from_raw_bytes(&response).unwrap();
        assert_eq!(response.emsg(), EMsg::ChannelEncryptResponse);
        // protocol version and key size, then the encrypted key, its CRC32 and four zero bytes
        assert_eq!(response.payload()[..8], [1, 0, 0, 0, 128, 0, 0, 0]);
        assert_eq!(response.payload().len(), 8 + 128 + 4 + 4);
        assert_eq!(response.payload()[8 + 128 + 4..], [0; 4]);
    }

    #[test]
    fn answers_a_short_nonce_with_a_warning() {
        crate::protocol::tests::logs::capture();
        let request = PacketMessage::from_raw_bytes(&CHANNEL_ENCRYPT_REQUEST[..36]).unwrap();
        let (encryption, response) = answer_encrypt_request(request).unwrap();
        // as SteamKit: no HMAC, and the key alone (RSA hides whether the nonce went with it; the size is the same)
        assert!(matches!(encryption, Encryption::Plain(_)));
        let response = PacketMessage::from_raw_bytes(&response).unwrap();
        assert_eq!(response.payload().len(), 8 + 128 + 4 + 4);
        assert!(crate::protocol::tests::logs::take()
            .iter()
            .any(|line| line.contains("nonce of 8 bytes")));
    }

    #[test]
    fn rejects_other_universes() {
        let mut request = CHANNEL_ENCRYPT_REQUEST;
        request[24] = EUniverse::Beta as u8;
        let request = PacketMessage::from_raw_bytes(&request).unwrap();
        assert!(matches!(
            answer_encrypt_request(request),
            Err(ClientError::UnsupportedUniverse(EUniverse::Beta))
        ));
    }

    #[test]
    fn truncated_handshake_messages_are_errors() {
        let request = PacketMessage::from_raw_bytes(&CHANNEL_ENCRYPT_REQUEST[..20]).unwrap();
        assert!(matches!(answer_encrypt_request(request), Err(ClientError::Message(_))));

        let mut result = (EMsg::ChannelEncryptResult as u32).to_le_bytes().to_vec();
        result.extend_from_slice(&[0xFF; 16]);
        let result = PacketMessage::from_raw_bytes(&result).unwrap();
        assert!(matches!(check_encrypt_result(result), Err(ClientError::Message(_))));
    }

    #[test]
    fn checks_the_encrypt_result() {
        assert!(check_encrypt_result(encrypt_result(EResult::OK)).is_ok());
        assert!(matches!(
            check_encrypt_result(encrypt_result(EResult::Fail)),
            Err(ClientError::EncryptionFailed(EResult::Fail))
        ));
    }
}

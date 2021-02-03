use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::connection::EncryptionState;
use crate::errors::PacketError;
use crate::messages::packet::PacketMessage;

const PACKET_MAGIC_BYTES: &[u8] = br#"VT01"#;
const PACKET_MAGIC_SIZE: usize = 4;

/// Used to encode and decode messages coming directly from the socket.
///
/// Should be able to automatically decrypt and encrypt `PacketMessages` based
/// on the state of the connection.
///
/// The sole responsibility of the codec, is to ensure that when the connection is encrypted,
/// it encrypts outgoing and decrypts incoming messages correctly.
///
/// It does not hold state by itself, and it doesn't know anything outside of encrypting and wrapping messages
/// with Steam magic bytes.
///
/// [SteamConnection] should know how to react to changes on the connection.
#[derive(Debug)]
pub(crate) struct PacketMessageCodec {
    remaining_msg_bytes: usize,
    encryption_state: EncryptionState,
}

impl Default for PacketMessageCodec {
    fn default() -> Self {
        Self {
            remaining_msg_bytes: 0,
            encryption_state: EncryptionState::Disconnected,
        }
    }
}

impl Encoder<Vec<u8>> for PacketMessageCodec {
    type Error = PacketError;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(1024);

        println!("This is writing something");
        println!("{:?}", item);

        let message_size = item.len() as u32;
        dst.extend_from_slice(&(message_size).to_le_bytes());
        dst.extend_from_slice(PACKET_MAGIC_BYTES);
        dst.extend_from_slice(&item);

        Ok(())
    }
}

impl Decoder for PacketMessageCodec {
    type Item = PacketMessage;
    type Error = PacketError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let encryption_state = self.encryption_state;

        println!("ehlo!");

        if self.remaining_msg_bytes > src.len() || src.is_empty() {
            return Ok(None);
        }

        let data_len = src.get_u32_le();
        let magic_bytes = src.copy_to_bytes(PACKET_MAGIC_SIZE);

        if magic_bytes != PACKET_MAGIC_BYTES {
            return Err(PacketError::Malformed);
        }

        let message_bytes = src.copy_to_bytes(data_len as usize);
        let packet_message = PacketMessage::from_raw_bytes(&message_bytes);
        src.reserve(data_len as usize);

        Ok(Some(packet_message))
    }
}

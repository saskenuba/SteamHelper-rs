use bytes::Buf;
use bytes::Bytes;
use bytes::BytesMut;
use steam_crypto::symmetric_decrypt_hmac_iv_in_place;
use steam_crypto::symmetric_encrypt_hmac_iv;
use steam_crypto::SessionKey;
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

use crate::errors::PacketError;

const PACKET_MAGIC_BYTES: &[u8] = br#"VT01"#;
const PACKET_MAGIC_SIZE: usize = 4;
/// Message length (u32) and magic bytes, before each message.
const PACKET_HEADER_SIZE: usize = 4 + PACKET_MAGIC_SIZE;

/// Frames messages on the socket: each one is prefixed by its length (u32) and the `VT01` magic.
///
/// Frames are not parsed into messages here, so a message we can't parse doesn't end the stream. Used as is during
/// the encryption handshake, then wrapped by [`EncryptedCodec`].
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FrameCodec;

impl Encoder<Vec<u8>> for FrameCodec {
    type Error = PacketError;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(PACKET_HEADER_SIZE + item.len());
        dst.extend_from_slice(&(item.len() as u32).to_le_bytes());
        dst.extend_from_slice(PACKET_MAGIC_BYTES);
        dst.extend_from_slice(&item);
        Ok(())
    }
}

impl Decoder for FrameCodec {
    type Item = BytesMut;
    type Error = PacketError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < PACKET_HEADER_SIZE {
            return Ok(None);
        }

        let data_len = u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
        if &src[4..PACKET_HEADER_SIZE] != PACKET_MAGIC_BYTES {
            return Err(PacketError::Malformed);
        }

        // wait until the whole message arrived
        if src.len() < PACKET_HEADER_SIZE + data_len {
            src.reserve(PACKET_HEADER_SIZE + data_len - src.len());
            return Ok(None);
        }

        src.advance(PACKET_HEADER_SIZE);
        Ok(Some(src.split_to(data_len)))
    }
}

/// Frames messages like [`FrameCodec`], once the channel is encrypted: the frame holds the encrypted message, and its
/// length is the length of the encrypted data.
///
/// Frames are decrypted in place, so a decoded message is a slice of the read buffer.
///
/// A frame that fails to decrypt is an error, which ends the stream: the connection can't be trusted anymore.
#[derive(Debug)]
pub(crate) struct EncryptedCodec {
    key: SessionKey,
}

impl EncryptedCodec {
    pub(crate) fn new(key: SessionKey) -> Self {
        Self { key }
    }
}

impl Encoder<Vec<u8>> for EncryptedCodec {
    type Error = PacketError;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        FrameCodec.encode(symmetric_encrypt_hmac_iv(&item, &self.key), dst)
    }
}

impl Decoder for EncryptedCodec {
    type Item = Bytes;
    type Error = PacketError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match FrameCodec.decode(src)? {
            Some(mut frame) => {
                let message = symmetric_decrypt_hmac_iv_in_place(&mut frame, &self.key)?;
                frame.truncate(message.end);
                frame.advance(message.start);
                Ok(Some(frame.freeze()))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use steam_crypto::CryptoError;

    use super::*;

    /// ChannelEncryptRequest, as it comes from the socket.
    const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
        23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0,
        0, 0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
    ];

    fn framed(message: &[u8]) -> Vec<u8> {
        let mut framed = (message.len() as u32).to_le_bytes().to_vec();
        framed.extend_from_slice(PACKET_MAGIC_BYTES);
        framed.extend_from_slice(message);
        framed
    }

    fn key() -> SessionKey {
        SessionKey::from([7; 32])
    }

    #[test]
    fn decodes_a_message_only_once_it_arrived_whole() {
        let framed = framed(&CHANNEL_ENCRYPT_REQUEST);
        let mut src = BytesMut::new();

        for byte in &framed[..framed.len() - 1] {
            src.extend_from_slice(&[*byte]);
            assert!(FrameCodec.decode(&mut src).unwrap().is_none());
        }
        src.extend_from_slice(&framed[framed.len() - 1..]);

        assert_eq!(
            FrameCodec.decode(&mut src).unwrap().unwrap(),
            CHANNEL_ENCRYPT_REQUEST[..]
        );
        assert!(src.is_empty());
    }

    #[test]
    fn decodes_back_to_back_messages() {
        let framed = framed(&CHANNEL_ENCRYPT_REQUEST);
        let mut src = BytesMut::from(&[framed.as_slice(), framed.as_slice()].concat()[..]);

        assert!(FrameCodec.decode(&mut src).unwrap().is_some());
        assert!(FrameCodec.decode(&mut src).unwrap().is_some());
        assert!(FrameCodec.decode(&mut src).unwrap().is_none());
    }

    #[test]
    fn rejects_wrong_magic_bytes() {
        let mut framed = framed(&CHANNEL_ENCRYPT_REQUEST);
        framed[4] = b'X';
        assert!(matches!(
            FrameCodec.decode(&mut BytesMut::from(&framed[..])),
            Err(PacketError::Malformed)
        ));
    }

    #[test]
    fn encodes_the_length_and_magic() {
        let mut dst = BytesMut::new();
        FrameCodec.encode(CHANNEL_ENCRYPT_REQUEST.to_vec(), &mut dst).unwrap();
        assert_eq!(dst, framed(&CHANNEL_ENCRYPT_REQUEST));
    }

    #[test]
    fn encrypted_frames_round_trip() {
        let mut dst = BytesMut::new();
        EncryptedCodec::new(key())
            .encode(CHANNEL_ENCRYPT_REQUEST.to_vec(), &mut dst)
            .unwrap();

        // the frame length is the length of the encrypted data: encrypted IV, then the padded message
        let data_len = u32::from_le_bytes([dst[0], dst[1], dst[2], dst[3]]) as usize;
        assert_eq!(data_len, 16 + 48);
        assert_eq!(dst.len(), PACKET_HEADER_SIZE + data_len);
        assert_eq!(&dst[4..8], PACKET_MAGIC_BYTES);
        assert_ne!(&dst[PACKET_HEADER_SIZE..], &CHANNEL_ENCRYPT_REQUEST[..]);

        // decoded in two parts, as it may arrive from the socket
        let mut src = BytesMut::from(&dst[..20]);
        let mut codec = EncryptedCodec::new(key());
        assert!(codec.decode(&mut src).unwrap().is_none());
        src.extend_from_slice(&dst[20..]);
        assert_eq!(codec.decode(&mut src).unwrap().unwrap(), CHANNEL_ENCRYPT_REQUEST[..]);
        assert!(src.is_empty());
    }

    #[test]
    fn encrypted_frames_with_another_key_are_errors() {
        let mut dst = BytesMut::new();
        EncryptedCodec::new(key())
            .encode(CHANNEL_ENCRYPT_REQUEST.to_vec(), &mut dst)
            .unwrap();

        let result = EncryptedCodec::new(SessionKey::from([8; 32])).decode(&mut dst);
        assert!(matches!(
            result,
            Err(PacketError::Decrypt(
                CryptoError::InvalidPadding | CryptoError::InvalidHmac
            ))
        ));

        // a plain frame is not an encrypted one
        let result = EncryptedCodec::new(key()).decode(&mut BytesMut::from(&framed(&CHANNEL_ENCRYPT_REQUEST)[..]));
        assert!(matches!(
            result,
            Err(PacketError::Decrypt(CryptoError::InvalidLength(44)))
        ));
    }
}

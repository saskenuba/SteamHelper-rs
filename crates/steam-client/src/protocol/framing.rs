//! Frames on a TCP connection: each message is prefixed by its length (u32) and the `VT01` magic. Once the channel is
//! encrypted, the frame holds the encrypted message, and its length is the length of the encrypted data.
//!
//! Frames are not parsed into messages here, so a message we can't parse doesn't end the connection.

use std::ops::Range;

use bytes::Buf;
use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use steam_crypto::symmetric_decrypt_hmac_iv_in_place;
use steam_crypto::symmetric_decrypt_in_place;
use steam_crypto::symmetric_encrypt;
use steam_crypto::symmetric_encrypt_hmac_iv;
use steam_crypto::CryptoError;
use steam_crypto::SessionKey;

use crate::errors::PacketError;

const PACKET_MAGIC_BYTES: &[u8] = br#"VT01"#;
const PACKET_MAGIC_SIZE: usize = 4;
/// Message length (u32) and magic bytes, before each message.
const PACKET_HEADER_SIZE: usize = 4 + PACKET_MAGIC_SIZE;
/// The longest frame taken: a longer one is `Malformed`. Steam's frames are far smaller (a logon `Multi` is a few
/// hundred KB at most); without a bound, four bytes would make the client reserve 4 GiB.
pub(crate) const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;
/// The most reserved at once for a frame still arriving, so that memory grows with what arrived, not with what the
/// length promises.
const RESERVE_CHUNK: usize = 64 * 1024;

/// How the channel is encrypted, as SteamKit decides: with an IV that authenticates the message when Steam's nonce is
/// 16 bytes or more, plainly otherwise.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Encryption {
    HmacIv(SessionKey),
    Plain(SessionKey),
}

impl Encryption {
    pub(crate) fn key(&self) -> &SessionKey {
        match self {
            Self::HmacIv(key) | Self::Plain(key) => key,
        }
    }

    fn encrypt(&self, message: &[u8]) -> Vec<u8> {
        match self {
            Self::HmacIv(key) => symmetric_encrypt_hmac_iv(message, key),
            Self::Plain(key) => symmetric_encrypt(message, key),
        }
    }

    fn decrypt_in_place(&self, buffer: &mut [u8]) -> Result<Range<usize>, CryptoError> {
        match self {
            Self::HmacIv(key) => symmetric_decrypt_hmac_iv_in_place(buffer, key),
            Self::Plain(key) => symmetric_decrypt_in_place(buffer, key),
        }
    }
}

/// Writes a frame to `out`: the message's length, the magic, then the message.
pub(crate) fn write_frame(message: &[u8], out: &mut BytesMut) {
    out.reserve(PACKET_HEADER_SIZE + message.len());
    out.put_u32_le(message.len() as u32);
    out.put_slice(PACKET_MAGIC_BYTES);
    out.put_slice(message);
}

/// Encrypts a message, and writes its frame to `out`.
pub(crate) fn write_encrypted(message: &[u8], encryption: &Encryption, out: &mut BytesMut) {
    write_frame(&encryption.encrypt(message), out);
}

/// Frames a message.
pub(crate) fn frame(message: &[u8]) -> Vec<u8> {
    let mut frame = BytesMut::new();
    write_frame(message, &mut frame);
    frame.to_vec()
}

/// Encrypts a message with the session key as Steam's CMs do (with HMAC IVs), and frames it.
pub(crate) fn encrypt(message: &[u8], key: &SessionKey) -> Vec<u8> {
    frame(&symmetric_encrypt_hmac_iv(message, key))
}

/// Takes the next frame off the front of `src`, without its length and magic: a slice of `src`, not a copy. `None`
/// until the whole frame arrived. A frame longer than [`MAX_FRAME_SIZE`] is `Malformed`.
pub(crate) fn decode(src: &mut BytesMut) -> Result<Option<BytesMut>, PacketError> {
    if src.len() < PACKET_HEADER_SIZE {
        return Ok(None);
    }

    let data_len = u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
    if &src[4..PACKET_HEADER_SIZE] != PACKET_MAGIC_BYTES || data_len > MAX_FRAME_SIZE {
        return Err(PacketError::Malformed);
    }

    // wait until the whole message arrived; bounded by `MAX_FRAME_SIZE`, so this can't overflow, even on 32 bits
    let frame_len = data_len + PACKET_HEADER_SIZE;
    if src.len() < frame_len {
        src.reserve((frame_len - src.len()).min(RESERVE_CHUNK));
        return Ok(None);
    }

    src.advance(PACKET_HEADER_SIZE);
    Ok(Some(src.split_to(data_len)))
}

/// Decrypts an encrypted frame in place: the message is a slice of the frame.
///
/// A frame that fails to decrypt is an error: the connection can't be trusted anymore.
pub(crate) fn decrypt_frame(mut frame: BytesMut, encryption: &Encryption) -> Result<Bytes, PacketError> {
    let message = encryption.decrypt_in_place(&mut frame)?;
    frame.truncate(message.end);
    frame.advance(message.start);
    Ok(frame.freeze())
}

/// Decrypts a frame encrypted as Steam's CMs do (with HMAC IVs).
pub(crate) fn decrypt(frame: BytesMut, key: &SessionKey) -> Result<Bytes, PacketError> {
    decrypt_frame(frame, &Encryption::HmacIv(key.clone()))
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

    fn key() -> SessionKey {
        SessionKey::from([7; 32])
    }

    #[test]
    fn decodes_a_message_only_once_it_arrived_whole() {
        let framed = frame(&CHANNEL_ENCRYPT_REQUEST);
        let mut src = BytesMut::new();

        for byte in &framed[..framed.len() - 1] {
            src.extend_from_slice(&[*byte]);
            assert!(decode(&mut src).unwrap().is_none());
        }
        src.extend_from_slice(&framed[framed.len() - 1..]);

        assert_eq!(decode(&mut src).unwrap().unwrap(), CHANNEL_ENCRYPT_REQUEST[..]);
        assert!(src.is_empty());
    }

    #[test]
    fn decodes_back_to_back_messages() {
        let framed = frame(&CHANNEL_ENCRYPT_REQUEST);
        let mut src = BytesMut::from(&[framed.as_slice(), framed.as_slice()].concat()[..]);

        assert!(decode(&mut src).unwrap().is_some());
        assert!(decode(&mut src).unwrap().is_some());
        assert!(decode(&mut src).unwrap().is_none());
    }

    #[test]
    fn rejects_wrong_magic_bytes() {
        let mut framed = frame(&CHANNEL_ENCRYPT_REQUEST);
        framed[4] = b'X';
        assert!(matches!(
            decode(&mut BytesMut::from(&framed[..])),
            Err(PacketError::Malformed)
        ));
    }

    #[test]
    fn rejects_frames_over_the_limit() {
        let header = |len: u32| [&len.to_le_bytes()[..], PACKET_MAGIC_BYTES].concat();

        // at the limit, it waits for the rest, reserving a bounded chunk
        let mut src = BytesMut::from(&header(MAX_FRAME_SIZE as u32)[..]);
        assert!(decode(&mut src).unwrap().is_none());
        assert!(src.capacity() <= PACKET_HEADER_SIZE + RESERVE_CHUNK);

        for len in [MAX_FRAME_SIZE as u32 + 1, u32::MAX] {
            assert!(matches!(
                decode(&mut BytesMut::from(&header(len)[..])),
                Err(PacketError::Malformed)
            ));
        }
    }

    #[test]
    fn a_frame_still_arriving_reserves_room_for_the_rest() {
        let header = |len: u32| [&len.to_le_bytes()[..], PACKET_MAGIC_BYTES].concat();
        let mut src = BytesMut::from(&header(40_000)[..]);
        assert!(decode(&mut src).unwrap().is_none());
        assert!(src.capacity() >= PACKET_HEADER_SIZE + 40_000);
    }

    #[test]
    fn reads_a_frame_at_the_limit() {
        let message = vec![7; MAX_FRAME_SIZE];
        let mut src = BytesMut::from(&frame(&message)[..]);
        assert_eq!(decode(&mut src).unwrap().unwrap().len(), MAX_FRAME_SIZE);
    }

    #[test]
    fn plain_encryption_round_trips() {
        let encryption = Encryption::Plain(key());
        let mut out = BytesMut::new();
        write_encrypted(&CHANNEL_ENCRYPT_REQUEST, &encryption, &mut out);

        let frame = decode(&mut out).unwrap().unwrap();
        assert_eq!(
            decrypt_frame(frame.clone(), &encryption).unwrap(),
            CHANNEL_ENCRYPT_REQUEST[..]
        );
        // not with an HMAC IV
        assert!(decrypt(frame, &key()).is_err());
        assert_eq!(encryption.key(), &key());
    }

    #[test]
    fn frames_with_the_length_and_magic() {
        let framed = frame(&CHANNEL_ENCRYPT_REQUEST);
        assert_eq!(framed[..4], 44u32.to_le_bytes());
        assert_eq!(&framed[4..8], PACKET_MAGIC_BYTES);
        assert_eq!(framed[8..], CHANNEL_ENCRYPT_REQUEST);
    }

    #[test]
    fn encrypted_frames_round_trip() {
        let encrypted = encrypt(&CHANNEL_ENCRYPT_REQUEST, &key());

        // the frame length is the length of the encrypted data: encrypted IV, then the padded message
        let data_len = u32::from_le_bytes([encrypted[0], encrypted[1], encrypted[2], encrypted[3]]) as usize;
        assert_eq!(data_len, 16 + 48);
        assert_eq!(encrypted.len(), PACKET_HEADER_SIZE + data_len);
        assert_eq!(&encrypted[4..8], PACKET_MAGIC_BYTES);
        assert_ne!(&encrypted[PACKET_HEADER_SIZE..], &CHANNEL_ENCRYPT_REQUEST[..]);

        // decoded in two parts, as it may arrive from the socket
        let mut src = BytesMut::from(&encrypted[..20]);
        assert!(decode(&mut src).unwrap().is_none());
        src.extend_from_slice(&encrypted[20..]);
        let frame = decode(&mut src).unwrap().unwrap();
        assert_eq!(decrypt(frame, &key()).unwrap(), CHANNEL_ENCRYPT_REQUEST[..]);
        assert!(src.is_empty());
    }

    #[test]
    fn encrypted_frames_with_another_key_are_errors() {
        let mut src = BytesMut::from(&encrypt(&CHANNEL_ENCRYPT_REQUEST, &key())[..]);
        let encrypted = decode(&mut src).unwrap().unwrap();
        assert!(matches!(
            decrypt(encrypted, &SessionKey::from([8; 32])),
            Err(PacketError::Decrypt(
                CryptoError::InvalidPadding | CryptoError::InvalidHmac
            ))
        ));

        // a plain frame is not an encrypted one
        let mut src = BytesMut::from(&frame(&CHANNEL_ENCRYPT_REQUEST)[..]);
        let plain = decode(&mut src).unwrap().unwrap();
        assert!(matches!(
            decrypt(plain, &key()),
            Err(PacketError::Decrypt(CryptoError::InvalidLength(44)))
        ));
    }
}

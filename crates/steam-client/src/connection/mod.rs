//! This module handles connections to Content Manager Server
//! First you connect into the ip using a tcp socket
//! Then reads/writes into it
//!
//! Packets are sent at the following format: packet_len + packet_magic + data
//! packet length: u32
//! packet magic: VT01
//!
//! Apparently, bytes received are in little endian

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

use bytes::Bytes;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures::SinkExt;
use futures::StreamExt;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::EMsgError;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;
use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::codec::Framed;

use crate::connection::encryption::answer_encrypt_request;
use crate::connection::encryption::check_encrypt_result;
use crate::errors::ClientError;
use crate::messages::codec::EncryptedCodec;
use crate::messages::codec::FrameCodec;
use crate::messages::message::ClientMessage;
use crate::messages::multi::unpack_multi;
use crate::messages::packet::PacketMessage;

pub(crate) mod encryption;

/// Clears the protobuf flag of a raw EMsg.
const EMSG_MASK: u32 = 0x7FFF_FFFF;

/// The writing half of an encrypted connection. Shared, so the heartbeat can write while the client reads.
pub(crate) type Sink<T> = Arc<Mutex<SplitSink<Framed<T, EncryptedCodec>, Vec<u8>>>>;

/// Encrypted connection to a CM server.
///
/// Reads whole messages, and skips the ones it can't parse, such as an `EMsg` we don't know, instead of failing.
/// `Multi` messages are unpacked into the messages they carry.
///
/// Reading doesn't allocate, past growing the buffers: messages are slices of the frames read, decrypted in place.
/// Unzipping a `Multi` allocates its buffer.
pub(crate) struct Connection<T = TcpStream> {
    sink: Sink<T>,
    stream: SplitStream<Framed<T, EncryptedCodec>>,
    /// Packets unpacked from a `Multi`, not read yet.
    pending: VecDeque<Bytes>,
}

impl Connection<TcpStream> {
    /// Connects over TCP to `endpoint` (`ip:port`), and encrypts the channel.
    pub(crate) async fn connect(endpoint: &str) -> Result<Self, ClientError> {
        debug!("Connecting to {}", endpoint);
        let stream = TcpStream::connect(endpoint).await?;
        Self::encrypt(Framed::new(stream, FrameCodec)).await
    }
}

impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    /// Runs the encryption handshake on a connection that was just opened.
    async fn encrypt(mut framed: Framed<T, FrameCodec>) -> Result<Self, ClientError> {
        let request = read_plain(&mut framed, EMsg::ChannelEncryptRequest).await?;
        let (session_key, response) = answer_encrypt_request(request)?;
        framed.send(response).await?;

        let result = read_plain(&mut framed, EMsg::ChannelEncryptResult).await?;
        check_encrypt_result(result)?;

        // anything already read after the result stays buffered, and is decrypted with the new codec
        Ok(Self::from_encrypted(
            framed.map_codec(|_| EncryptedCodec::new(session_key)),
        ))
    }

    fn from_encrypted(framed: Framed<T, EncryptedCodec>) -> Self {
        let (sink, stream) = framed.split();
        Self {
            sink: Arc::new(Mutex::new(sink)),
            stream,
            pending: VecDeque::new(),
        }
    }

    /// The writing half, to write from another task.
    pub(crate) fn sink(&self) -> Sink<T> {
        self.sink.clone()
    }

    /// Encrypts and sends a serialized message.
    pub(crate) async fn send(&self, message: Vec<u8>) -> Result<(), ClientError> {
        send(&self.sink, message).await
    }

    /// Reads the next message we can parse.
    ///
    /// Fails if the connection was closed, or a frame could not be read or decrypted. Messages that can't be parsed
    /// are logged and skipped.
    pub(crate) async fn recv(&mut self) -> Result<PacketMessage, ClientError> {
        loop {
            let raw = match self.pending.pop_front() {
                Some(raw) => raw,
                None => match self.stream.next().await {
                    Some(frame) => frame?,
                    None => return Err(ClientError::Dropped),
                },
            };

            if let Some(packet) = parse(raw, &mut self.pending) {
                return Ok(packet);
            }
        }
    }
}

impl<T> fmt::Debug for Connection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

/// Encrypts and sends a serialized message.
pub(crate) async fn send<T>(sink: &Sink<T>, message: Vec<u8>) -> Result<(), ClientError>
where
    T: AsyncWrite + Unpin,
{
    sink.lock().await.send(message).await?;
    Ok(())
}

/// Reads plain messages until `expected`, during the encryption handshake.
async fn read_plain<T>(framed: &mut Framed<T, FrameCodec>, expected: EMsg) -> Result<PacketMessage, ClientError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let frame = framed.next().await.ok_or(ClientError::Dropped)??;
        let packet = PacketMessage::from_bytes(frame.freeze())?;
        if packet.emsg() == expected {
            return Ok(packet);
        }
        debug!(
            "Skipping {:?} during channel setup, waiting for {:?}",
            packet.emsg(),
            expected
        );
    }
}

/// Parses a raw message. A `Multi` is unpacked into `pending`, and messages that can't be parsed are logged.
///
/// Returns the message, unless it was one of those.
pub(crate) fn parse(raw: Bytes, pending: &mut VecDeque<Bytes>) -> Option<PacketMessage> {
    // for the log, as parsing consumes `raw`
    let raw_len = raw.len();
    let raw_emsg = raw
        .get(..4)
        .map(|emsg| u32::from_le_bytes([emsg[0], emsg[1], emsg[2], emsg[3]]));

    let packet = match PacketMessage::from_bytes(raw) {
        Ok(packet) => packet,
        Err(MessageError::EMsg(EMsgError::ValueNotFound(_))) => {
            let emsg = raw_emsg.unwrap_or_default() & EMSG_MASK;
            debug!("Skipping a message with unknown EMsg {} ({} bytes)", emsg, raw_len);
            return None;
        }
        Err(error) => {
            warn!("Skipping a message that could not be parsed: {}", error);
            return None;
        }
    };

    if packet.emsg() != EMsg::Multi {
        trace!("Received {:?}", packet.emsg());
        return Some(packet);
    }
    if !matches!(packet.header(), MessageHeaderWrapper::Proto(_)) {
        warn!("Skipping a Multi without a protobuf header");
        return None;
    }

    let multi = match ClientMessage::<CMsgMulti>::from_proto_packet(packet) {
        Ok(multi) => multi.body,
        Err(error) => {
            warn!("Skipping a Multi that could not be parsed: {}", error);
            return None;
        }
    };
    match unpack_multi(multi, pending) {
        Ok(unpacked) => trace!("Unpacked {} messages from a Multi", unpacked),
        Err(error) => warn!("Skipping a Multi that could not be unpacked: {}", error),
    }
    None
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use bytes::BytesMut;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use steam_crypto::SessionKey;
    use steam_language_gen::SerializableBytes;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHeartBeat;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
    use tokio::io::duplex;
    use tokio::io::AsyncWriteExt;
    use tokio::io::DuplexStream;
    use tokio_util::codec::Encoder;

    use super::*;

    fn key() -> SessionKey {
        SessionKey::from([3; 32])
    }

    fn proto<M: steam_protobuf::protobuf::Message>(emsg: EMsg, body: M) -> Vec<u8> {
        let mut message = ClientMessage::<M>::new_proto(emsg);
        message.body = body;
        message.to_bytes()
    }

    fn multi(packets: &[Vec<u8>], gzip: bool) -> Vec<u8> {
        let mut body = vec![];
        for packet in packets {
            body.extend_from_slice(&(packet.len() as u32).to_le_bytes());
            body.extend_from_slice(packet);
        }
        let mut multi = CMsgMulti::new();
        if gzip {
            multi.set_size_unzipped(body.len() as u32);
            let mut encoder = GzEncoder::new(vec![], Compression::default());
            encoder.write_all(&body).unwrap();
            body = encoder.finish().unwrap();
        }
        multi.set_message_body(body.into());
        proto(EMsg::Multi, multi)
    }

    fn logged_off() -> Vec<u8> {
        let mut body = CMsgClientLoggedOff::new();
        body.set_eresult(1);
        proto(EMsg::ClientLoggedOff, body)
    }

    fn unknown_emsg() -> Vec<u8> {
        let mut raw = (0x7FFF_0000u32 | 0x8000_0000).to_le_bytes().to_vec();
        raw.extend_from_slice(&[0, 0, 0, 0]);
        raw
    }

    /// A connection already encrypted with `key()`, and the server's end of it.
    fn connection() -> (Connection<DuplexStream>, DuplexStream) {
        let (client, server) = duplex(64 * 1024);
        (
            Connection::from_encrypted(Framed::new(client, EncryptedCodec::new(key()))),
            server,
        )
    }

    async fn write_encrypted(server: &mut DuplexStream, messages: &[Vec<u8>]) {
        let mut frames = BytesMut::new();
        for message in messages {
            EncryptedCodec::new(key()).encode(message.clone(), &mut frames).unwrap();
        }
        server.write_all(&frames).await.unwrap();
    }

    #[tokio::test]
    async fn skips_unknown_messages_and_unpacks_multis() {
        let (mut connection, mut server) = connection();
        let heartbeat = proto(EMsg::ClientHeartBeat, CMsgClientHeartBeat::new());

        write_encrypted(
            &mut server,
            &[
                unknown_emsg(),
                vec![1, 2],
                multi(
                    &[heartbeat.clone(), unknown_emsg(), multi(&[logged_off()], false)],
                    true,
                ),
                heartbeat,
            ],
        )
        .await;
        drop(server);

        assert_eq!(connection.recv().await.unwrap().emsg(), EMsg::ClientHeartBeat);
        assert_eq!(connection.recv().await.unwrap().emsg(), EMsg::ClientLoggedOff);
        assert_eq!(connection.recv().await.unwrap().emsg(), EMsg::ClientHeartBeat);
        assert!(matches!(connection.recv().await, Err(ClientError::Dropped)));
    }

    #[tokio::test]
    async fn sends_encrypted_frames() {
        let (connection, mut server) = connection();
        connection.send(logged_off()).await.unwrap();

        let mut server = Framed::new(&mut server, EncryptedCodec::new(key()));
        assert_eq!(server.next().await.unwrap().unwrap(), logged_off());
    }
}

//! This module handles connections to Content Manager Server
//! First you connect into the ip using a tcp socket
//! Then reads/writes into it
//!
//! Packets are sent at the following format: packet_len + packet_magic + data
//! packet length: u32
//! packet magic: VT01
//!
//! Apparently, bytes received are in little endian

use std::error::Error;

use async_trait::async_trait;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::StreamExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::codec::FramedRead;

use steam_crypto::SessionKeys;

use crate::{errors::PacketError, messages::packet::PacketMessage};
use crate::errors::ConnectionError;
use crate::messages::codec::PacketMessageCodec;
use crate::messages::message::ClientMessage;

mod encryption;

const PACKET_MAGIC_BYTES: &[u8] = br#"VT01"#;

/// This should be an abstraction over low-level socket handlers and is not to be used directly.
/// [SteamClient] is used for binding and connecting.
#[derive(Debug)]
pub(crate) struct SteamConnection<S> {
    /// Stream of data to Steam Content server. May be TCP or Websocket.
    stream: S,
    /// Address to which the connection is bound.
    endpoint: String,
    /// Current encryption state
    state: EncryptionState,
    /// Populated after the initial handshake with Steam
    session_keys: Option<SessionKeys>,
}

#[async_trait]
trait Connection<S> {
    async fn new_connection(ip_addr: &str) -> Result<SteamConnection<S>, Box<dyn Error>>;
    async fn read_packets(&mut self) -> Result<PacketMessage, PacketError>;
    async fn write_packets(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>>;
}

#[cfg(not(feature = "websockets"))]
impl SteamConnection<TcpStream> {
    async fn main_loop(&mut self) -> Result<(), ConnectionError> {
        let (rx, tx) = self.stream.split();

        let mut framed_read = FramedRead::new(rx, PacketMessageCodec::default());

        // tokio::spawn(async move { while let Some(stuff) = framed_read.next().await {} });
        Ok(())
    }
}

#[cfg(not(feature = "websockets"))]
#[async_trait]
impl Connection<TcpStream> for SteamConnection<TcpStream> {
    /// Opens a tcp stream to specified IP
    async fn new_connection(ip_addr: &str) -> Result<SteamConnection<TcpStream>, Box<dyn Error>> {
        trace!("Connecting to ip: {}", &ip_addr);

        let stream = TcpStream::connect(ip_addr).await?;

        Ok(SteamConnection {
            stream,
            endpoint: ip_addr.to_string(),
            state: EncryptionState::Disconnected,
            session_keys: None,
        })
    }

    #[inline]
    async fn read_packets(&mut self) -> Result<PacketMessage, PacketError> {
        let mut framed_stream = FramedRead::new(&mut self.stream, PacketMessageCodec::default());
        Ok(framed_stream.next().await.unwrap().unwrap())
    }

    #[inline]
    async fn write_packets(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut output_buffer = BytesMut::with_capacity(1024);

        trace!("payload size: {} ", data.len());

        output_buffer.extend_from_slice(&(data.len() as u32).to_le_bytes());
        output_buffer.extend_from_slice(PACKET_MAGIC_BYTES);
        output_buffer.extend_from_slice(data);
        let output_buffer = output_buffer.freeze();

        trace!("Writing {} bytes of data to stream..", output_buffer.len());
        trace!("Payload bytes: {:?}", output_buffer);

        let write_result = self.stream.write(&output_buffer).await?;
        trace!("write result: {}", write_result);
        Ok(())
    }
}

#[cfg(feature = "websockets")]
mod connection_method {
    use tokio_tungstenite::{connect_async, stream::Stream, WebSocketStream};

    use tokio_tls::TlsStream;

    use super::*;

    type Ws = WebSocketStream<Stream<TcpStream, TlsStream<TcpStream>>>;

    #[async_trait]
    impl Connection<Ws> for SteamConnection<Ws> {
        async fn new_connection(ws_url: &str) -> Result<SteamConnection<Ws>, Box<dyn Error>> {
            let formatted_ws_url = format!("wss://{}/cmsocket/", ws_url);
            debug!("Connecting to addr: {}", formatted_ws_url);

            let (stream, _) = connect_async(&formatted_ws_url).await?;

            Ok(SteamConnection {
                stream,
                endpoint: formatted_ws_url,
                state: EncryptionState::Disconnected,
            })
        }
        #[inline]
        async fn read_packets(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
            let mut data_len: [u8; 4] = [0; 4];
            self.stream.get_mut().read_exact(&mut data_len).await?;

            let mut packet_magic: [u8; 4] = [0; 4];
            self.stream.get_mut().read_exact(&mut packet_magic).await?;

            if packet_magic != PACKET_MAGIC_BYTES {
                log::error!("Could not find magic packet on read.");
            }

            let mut incoming_data = BytesMut::with_capacity(1024);
            self.stream.get_mut().read_buf(&mut incoming_data).await?;

            // sanity check
            debug!("data length: {}", u32::from_le_bytes(data_len));
            trace!("data: {:?}", incoming_data);

            Ok(incoming_data.to_vec())
        }

        #[inline]
        async fn write_packets(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
            unimplemented!()
        }
    }
}

#[derive(Debug, Copy, Clone)]
/// Represents the current state of encryption of the connection.
/// Steam is always encrypted, with the exception when the connection is starting.
pub(crate) enum EncryptionState {
    /// After initial connection is established, Steam requests to encrypt messages
    /// through a [EMsg::ChannelEncryptRequest]
    Connected,
    /// We are challenged after Steam returns a [EMsg::ChannelEncryptResult].
    ///
    /// After checking the result for a positive outcome, we should be `Encrypted`, else we get disconnected,
    /// and try again.
    Challenged,
    /// We are encrypted and there is nothing left to do.
    Encrypted,
    /// State only after logOff or if encryption fails.
    Disconnected,
}

#[cfg(test)]
mod tests {
    use env_logger::{Builder, Target};
    use log::LevelFilter;

    use steam_language_gen::generated::enums::EMsg;

    use crate::content_manager::dump_tcp_servers;
    use crate::connection::encryption::handle_encrypt_request;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    fn init() {
        let _ = Builder::from_default_env()
            .filter_module("steam_api", LevelFilter::Trace)
            .is_test(true)
            .try_init();
    }

    #[tokio::test]
    #[cfg(not(feature = "websockets"))]
    async fn connect_to_web_server() {
        init();

        let dumped_cm_servers = dump_tcp_servers().await.unwrap();
        let steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await;
        assert!(steam_connection.is_ok());
    }

    #[tokio::test]
    #[cfg(not(feature = "websockets"))]
    async fn test_spawn() {
        let dumped_cm_servers = dump_tcp_servers().await.unwrap();
        let mut steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap();

        let packet_message = steam_connection.read_packets().await.unwrap();
        assert_eq!(packet_message.emsg(), EMsg::ChannelEncryptRequest);

        let answer = handle_encrypt_request(packet_message);
        steam_connection.write_packets(answer.as_slice()).await.unwrap();
        let data = steam_connection.read_packets().await.unwrap();
        assert_eq!(data.emsg(), EMsg::ChannelEncryptResult);
        // steam_connection.main_loop().await.unwrap()
    }

    // #[tokio::test()]
    // #[cfg(not(feature = "websockets"))]
    // async fn answer_encrypt_request() {
    // init();
    //
    // let cm_servers = CmServerSvList::fetch_servers(env!("STEAM_API")).await;
    // let dumped_cm_servers = cm_servers.unwrap().dump_tcp_servers();
    //
    // let mut steam_connection: SteamConnection<TcpStream> =
    // SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap(); let data =
    // steam_connection.read_packets().await.unwrap(); let message = EMsg::from_raw_message(&data);
    //
    // assert_eq!(message.unwrap(), EMsg::ChannelEncryptRequest);
    //
    //
    // let answer = handle_encrypt_request(PacketMessage::from_rawdata(&data));
    // steam_connection.write_packets(answer.as_slice()).await.unwrap();
    // let data = steam_connection.read_packets().await.unwrap();
    // let message = EMsg::from_raw_message(&data).unwrap();
    // assert_eq!(message, EMsg::ChannelEncryptResult);
    // }

    #[tokio::test(threaded_scheduler)]
    #[cfg(feature = "websockets")]
    async fn connect_to_ws_server() {
        init();

        let get_results = CmServerSvList::fetch_servers("1").await;
        let fetched_servers = get_results.unwrap().dump_ws_servers();

        let steam_connection = SteamConnection::new_connection(&fetched_servers[0]).await;
        assert!(steam_connection.is_ok())
    }
}

//! This module handles connections to Content Manager Server
//! First you connect into the ip using a tcp socket
//! Then reads/writes into it
//!
//! Packets are sent at the following format: packet_len + packet_magic + data
//! packet length: u32
//! packet magic: VT01
//!
//! Apparently, bytes received are in little endian
//!

use std::{convert::TryInto, error::Error, future::Future, io::BufRead};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{task::Context, FutureExt, StreamExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncBufReadExt,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    macros::support::{Pin, Poll},
    net::TcpStream,
    task,
};
// use tokio_util::codec::{Decoder, Encoder};
use tracing::{info, instrument};

use async_trait::async_trait;

const PACKET_MAGIC_BYTES: &[u8] = br#"VT01"#;

/// This should be an abstraction over low-level socket handlers and is not to be used directly.
/// Use [SteamClient] instead for binding and connecting.
// Should be a way to register event handlers, so we can listen to certain types of events,
// like friends logging in, or receiving trade requests.
pub struct SteamConnection<S> {
    /// Stream of data to Steam Content server. May be TCP or Websocket.
    stream: S,
    /// Address to which the connection is bound.
    endpoint: String,
    /// Current encryption state
    state: EncryptionState,
}

#[async_trait]
trait Connection<S> {
    async fn new_connection(ip_addr: &str) -> Result<SteamConnection<S>, Box<dyn Error>>;
    async fn read_packets(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn write_packets(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>>;
}

impl<S> SteamConnection<S> {}

#[cfg(not(feature = "websockets"))]
impl SteamConnection<TcpStream> {
    async fn main_loop(&mut self) -> Result<(), Box<dyn Error>> {
        let (rx, tx) = self.stream.split();
        loop {
            tokio::spawn(async move {});
        }
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
        })
    }

    #[inline]
    async fn read_packets(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut data_len: [u8; 4] = [0; 4];
        self.stream.read_exact(&mut data_len).await?;

        let mut packet_magic: [u8; 4] = [0; 4];
        self.stream.read_exact(&mut packet_magic).await?;

        if packet_magic != PACKET_MAGIC_BYTES {
            log::error!("Could not find magic packet on read.");
        }

        let data_length = u32::from_le_bytes(data_len);

        let mut incoming_data = vec![0u8; data_length as usize];
        let buffer_read_size = self.stream.read(&mut incoming_data).await?;
        debug_assert_eq!(buffer_read_size as u32, data_length);

        // Sanity check
        trace!("data: {:?}", incoming_data);

        Ok(incoming_data)
    }

    #[inline]
    async fn write_packets(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut output_buffer = BytesMut::with_capacity(1024);

        trace!("payload size: {} ", data.len());

        output_buffer.extend_from_slice(&(data.len() as u32).to_le_bytes());
        output_buffer.extend_from_slice(PACKET_MAGIC_BYTES);
        output_buffer.extend_from_slice(data);

        trace!("Writing {} bytes of data to stream..", output_buffer.len());
        trace!("Payload bytes: {:?}", output_buffer.bytes());

        let write_result = self.stream.write(output_buffer.bytes()).await?;
        trace!("write result: {}", write_result);
        Ok(())
    }
}

#[cfg(feature = "websockets")]
mod connection_method {
    use tokio_tls::TlsStream;
    use tokio_tungstenite::{connect_async, stream::Stream, WebSocketStream};

    use super::*;

    type Ws = WebSocketStream<Stream<TcpStream, TlsStream<TcpStream>>>;

    #[async_trait]
    impl Connection<Ws> for SteamConnection<Ws> {
        async fn new_connection(ws_url: &str) -> Result<SteamConnection<Ws>, Box<dyn Error>> {
            let formatted_ws_url = format!("wss://{}/cmsocket/", ws_url);
            debug!("Connecting to addr: {}", formatted_ws_url);

            let (ws_stream, _) = connect_async(&formatted_ws_url).await?;

            Ok(SteamConnection {
                stream: ws_stream,
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

            //sanity check
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

#[derive(new, Serialize, Deserialize)]
/// Abstraction over bytes received through the stream connected to Steam3
struct SteamBytes {
    data_len: u32,
    #[new(value = "PACKET_MAGIC_BYTES")]
    magic: &'static [u8],
    payload: Vec<u8>,
}

// impl Encoder for SteamBytes {
//     type Item = SteamBytes;
//     type Error = Box<dyn Error>;
//
//     fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
//         dst.reserve(1024);
//         dst.put(bincode::serialize(&item).unwrap().as_slice());
//         Ok(())
//     }
// }

enum EncryptionState {
    Disconnected,
    Connected,
    Challenged,
    Encrypted,
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use env_logger::{Builder, Target};
    use log::LevelFilter;
    use tokio::time::Duration;

    use steam_language_gen::generated::enums::EMsg;

    use crate::cmserver::CmServerSvList;
    use crate::encrypted_connection::handle_encrypt_request;
    use crate::messages::packetmessage::PacketMessage;

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

        let cm_servers = CmServerSvList::fetch_servers("1").await;
        let dumped_cm_servers = cm_servers.unwrap().dump_tcp_servers();

        let steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await;
        assert!(steam_connection.is_ok());
    }

    #[tokio::test]
    #[cfg(not(feature = "websockets"))]
    async fn test_spawn() {
        let cm_servers = CmServerSvList::fetch_servers("1").await;
        let dumped_cm_servers = cm_servers.unwrap().dump_tcp_servers();
        let mut steam_connection =
            SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap();

        // let local = tokio::task::LocalSet::new();
        let data = steam_connection.read_packets().await.unwrap();
        let message = EMsg::from_raw_message(&data);
        assert_eq!(message.unwrap(), EMsg::ChannelEncryptRequest);

        let answer = handle_encrypt_request(PacketMessage::from_rawdata(&data));
        steam_connection.write_packets(answer.as_slice()).await.unwrap();
        // let data = steam_connection.read_packets().await.unwrap();
        // let message = EMsg::from_raw_message(&data).unwrap();
        // assert_eq!(message, EMsg::ChannelEncryptResult);
        steam_connection.main_loop().await.unwrap()
    }

    /*#[tokio::test()]
    #[cfg(not(feature = "websockets"))]
    async fn answer_encrypt_request() {
        init();

        let cm_servers = CmServerSvList::fetch_servers(env!("STEAM_API")).await;
        let dumped_cm_servers = cm_servers.unwrap().dump_tcp_servers();

        let mut steam_connection: SteamConnection<TcpStream> = SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap();
        let data = steam_connection.read_packets().await.unwrap();
        let message = EMsg::from_raw_message(&data);

        assert_eq!(message.unwrap(), EMsg::ChannelEncryptRequest);

        //
        let answer = handle_encrypt_request(PacketMessage::from_rawdata(&data));
        steam_connection.write_packets(answer.as_slice()).await.unwrap();
        let data = steam_connection.read_packets().await.unwrap();
        let message = EMsg::from_raw_message(&data).unwrap();
        assert_eq!(message, EMsg::ChannelEncryptResult);
    }*/

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

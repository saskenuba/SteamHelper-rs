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
use std::sync::atomic::AtomicI32;

use async_trait::async_trait;
use atomic::{Atomic, Ordering};
use futures::{SinkExt, StreamExt};
use steam_crypto::SessionKeys;
use steam_language_gen::generated::enums::{EMsg, EResult};
use steam_language_gen::SerializableBytes;
use steam_protobuf::steam::steammessages_clientserver_login::CMsgClientLogonResponse;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
#[cfg(feature = "websockets")]
use tokio_tungstenite::{connect_async, WebSocketStream};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::connection::encryption::handle_encryption_negotiation;
use crate::errors::ConnectionError;
use crate::messages::codec::PacketMessageCodec;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::utils::EResultCast;

pub(crate) mod encryption;

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
    state: Atomic<EncryptionState>,
    /// Populated after the initial handshake with Steam
    session_keys: Option<SessionKeys>,

    heartbeat_seconds: AtomicI32,
}

impl<S> SteamConnection<S> {
    pub fn change_encryption_state(&self, new_state: EncryptionState) {
        self.state.swap(new_state, Ordering::AcqRel);
    }
}

#[async_trait]
pub(crate) trait CMConnectionExt<S> {
    async fn new_connection(cm_ip_addr: String) -> Result<SteamConnection<S>, ConnectionError>;
}

pub(crate) type PacketTx = UnboundedSender<PacketMessage>;
pub(crate) type MessageTx<T> = UnboundedSender<ClientMessage<T>>;

pub(crate) type DynBytes = Box<dyn SerializableBytes>;
pub(crate) type BytesRx = UnboundedReceiver<Vec<u8>>;
pub(crate) type BytesTx = UnboundedSender<Vec<u8>>;

#[cfg(not(feature = "websockets"))]
impl SteamConnection<TcpStream> {
    pub async fn main_loop(self) -> Result<(), ConnectionError> {
        let (outgoing_messages_tx, mut incoming_messagex_rx): (BytesTx, BytesRx) = mpsc::unbounded_channel();

        let connection_state = self.state;
        let (stream_rx, stream_tx) = self.stream.into_split();

        let mut framed_read = FramedRead::new(stream_rx, PacketMessageCodec::default());
        let mut framed_write = FramedWrite::new(stream_tx, PacketMessageCodec::default());

        tokio::spawn(async move {
            if let Some(message) = incoming_messagex_rx.recv().await {
                framed_write.send(message).await.unwrap();
                // if let EncryptionState::Encrypted = state_reader.load(Ordering::SeqCst) {
                // } else {
                //     framed_write.send(message).await.unwrap();
                // }
            }
        });

        while let Some(packet_message) = framed_read.next().await {
            let packet_message = packet_message.unwrap();

            match packet_message.emsg() {
                EMsg::ChannelEncryptRequest | EMsg::ChannelEncryptResponse | EMsg::ChannelEncryptResult => {
                    handle_encryption_negotiation(outgoing_messages_tx.clone(), &connection_state, packet_message)
                        .unwrap();
                }
                _ => {
                    unimplemented!()
                }
            };
        }

        Ok(())
    }
}

/// Information coming from the logon result, should bubble up to the client.
fn handle_logon_response(message: PacketMessage) {
    // TODO this could be a proto or extended header..
    let incoming_message: ClientMessage<CMsgClientLogonResponse> = ClientMessage::from_packet_message(message);

    let result = incoming_message.body.get_eresult().as_eresult();

    let proto_header = incoming_message.proto_header().unwrap();
    let session_id = proto_header.get_client_sessionid();
    let steamid = proto_header.get_steamid();

    let cell_id = incoming_message.body.get_cell_id();
    let heartbeat_delay = incoming_message.body.get_out_of_game_heartbeat_seconds();
}

#[cfg(not(feature = "websockets"))]
#[async_trait]
impl CMConnectionExt<TcpStream> for SteamConnection<TcpStream> {
    /// Opens a tcp stream to specified IP
    async fn new_connection(cm_ip_addr: String) -> Result<SteamConnection<TcpStream>, ConnectionError> {
        trace!("Connecting to ip: {}", &cm_ip_addr);

        let stream = TcpStream::connect(&cm_ip_addr).await?;

        Ok(SteamConnection {
            stream,
            endpoint: cm_ip_addr,
            state: Atomic::new(EncryptionState::Disconnected),
            session_keys: None,
            heartbeat_seconds: Default::default(),
        })
    }
}

#[cfg(feature = "websockets")]
pub type WsStream = WebSocketStream<TcpStream>;

#[cfg(feature = "websockets")]
#[async_trait]
impl CMConnectionExt<WsStream> for SteamConnection<WsStream> {
    async fn new_connection(ws_url: &str) -> Result<SteamConnection<WsStream>, ConnectionError> {
        let formatted_ws_url = format!("wss://{}/cmsocket/", ws_url);
        debug!("Connecting to addr: {}", formatted_ws_url);

        let ((stream, resp), _) = connect_async(&formatted_ws_url).await?;

        Ok(SteamConnection {
            stream,
            endpoint: formatted_ws_url,
            state: Atomic::new(EncryptionState::Disconnected),
            session_keys: None,
            heartbeat_seconds: Default::default(),
        })
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
    use env_logger::Builder;
    use log::LevelFilter;
    use steam_language_gen::generated::enums::EMsg;
    use steam_language_gen::SerializableBytes;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use crate::connection::encryption::handle_encrypt_request;
    use crate::content_manager::dump_tcp_servers;

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[cfg(not(feature = "websockets"))]
    async fn main_loop() {
        let dumped_cm_servers = dump_tcp_servers().await.unwrap();
        let steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap();
        steam_connection.main_loop().await.unwrap()
    }

    #[tokio::test]
    #[cfg(not(feature = "websockets"))]
    async fn test_spawn() {
        let dumped_cm_servers = dump_tcp_servers().await.unwrap();
        let mut steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await.unwrap();

        let packet_message = steam_connection.read_packets().await.unwrap();
        assert_eq!(packet_message.emsg(), EMsg::ChannelEncryptRequest);

        let answer = handle_encrypt_request(packet_message).to_bytes();
        steam_connection.write_packets(&answer).await.unwrap();
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

    #[cfg(feature = "websockets")]
    async fn connect_to_ws_server() {
        init();

        let dumped_cm_servers = dump_tcp_servers().await.unwrap();
        let steam_connection = SteamConnection::new_connection(&dumped_cm_servers[0]).await;
        assert!(steam_connection.is_ok())
    }
}

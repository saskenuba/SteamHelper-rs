use std::{io, thread};
use std::error::Error;

use tokio::{
    net::TcpStream,
    prelude::*,
};

use async_trait::{async_trait};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

///! This module handles connections to Content Manager Server
///! First you connect into the ip using a tcp socket
///! Then reads/writes into it
///
/// Packets are sent at the following format: packet_len + packet_magic + data
/// packet length: u32
/// packet magic: VT01
///
/// Apparently, bytes received are in little endian

const PACKET_MAGIC: &[u8] = br#"VT01"#;

/// This should be an abstraction over low-level socket handlers.
#[derive(Debug)]
struct SteamConnection {
    stream: TcpStream,
}

#[async_trait]
trait SocketConnection {
    async fn new_connection(ip_addr: &str) -> Result<SteamConnection, Box<dyn Error>>;
    async fn read_packet(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn write_packet(&mut self, data: &[u8]);
    async fn send_heartbeat(interval_sec: u32);
}

#[async_trait]
impl SocketConnection for SteamConnection {

    /// Opens a tcp stream to specified IP
    async fn new_connection(ip_addr: &str) -> Result<SteamConnection, Box<dyn Error>> {
        let stream = TcpStream::connect(&ip_addr).await.unwrap();

        Ok(SteamConnection {
            stream
        })
    }

    async fn read_packet(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut data_len: [u8; 4] = [0; 4];
        self.stream.read_exact(&mut data_len);

        let mut packet_magic: [u8; 4] = [0; 4];
        self.stream.read_exact(&mut packet_magic);

        if packet_magic != PACKET_MAGIC {
            panic!("Could not find magic packet on read.");
        }

        let mut incoming_data: Vec<u8> = Vec::new();
        self.stream.read_to_end(&mut incoming_data);

        //sanity check
        println!("data length: {}", u32::from_le_bytes(data_len));
        println!("vector length: {}", incoming_data.len());

        Ok(incoming_data)
    }

    async fn write_packet(&mut self, data: &[u8]) {
        let data_len: [u8; 4] = (data.len() as u32).to_be_bytes();
        self.stream.write_all(&data_len);
        self.stream.write_all(PACKET_MAGIC);
        self.stream.write_all(data);

    }

    async fn send_heartbeat(interval_sec: u32) {
        unimplemented!()
    }
}


fn send_heartbeat() {

    thread::spawn(move || {

    });
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use crate::CMserver::{CmServerWebApi, fetch_servers};

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

//    #[test]
//    fn connect_to_web_server() {
//        let get_results = fetch_servers(env!("STEAM_API"));
//        let fetched_servers = get_results.unwrap();
//        let mut connection = new_connection(&fetched_servers.dump_servers()[0]).unwrap();
//        let data = read_packet(&mut connection).unwrap();
//        println!("{:?}", data);
//    }
}

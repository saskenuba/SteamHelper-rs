use std::io::{Read, Write};
use std::{io, net::TcpStream};

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

#[derive(Debug)]
struct SteamConnection {
    current_address: TcpStream,
}

/// Opens a tcp stream to specified IP
fn new_connection(ip_with_port: &str) -> Result<TcpStream, io::Error> {
    // we can also pass a list of ips

    let stream: TcpStream = match TcpStream::connect(&ip_with_port) {
        Ok(stream) => stream,
        _ => panic!("Could not connect to server"),
    };

    Ok(stream)
}

fn read_packet(stream: &mut TcpStream) -> Result<Vec<u8>, io::Error> {
    let mut data_len: [u8; 4] = [0; 4];
    stream.read_exact(&mut data_len);

    let mut packet_magic: [u8; 4] = [0; 4];
    stream.read_exact(&mut packet_magic);

    if packet_magic != PACKET_MAGIC {
        panic!("Could not find magic packet on read.");
    }

    let mut data: Vec<u8> = Vec::new();
    stream.read_to_end(&mut data);

    //sanity check
    println!("data length: {}", u32::from_le_bytes(data_len));

    Ok(data)
}

fn write_packet(stream: &mut TcpStream, data: &[u8]) -> Result<bool, io::Error> {
    let data_len: [u8; 4] = (data.len() as u32).to_be_bytes();
    stream.write_all(&data_len);
    stream.write_all(PACKET_MAGIC);
    stream.write_all(data);

    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::server::{fetch_servers, CmServerWebApi};

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn connect_to_web_server() {
        let get_results = fetch_servers(env!("STEAM_API"));
        let fetched_servers = get_results.unwrap();
        let mut connection = new_connection(&fetched_servers.dump_servers()[0]).unwrap();
        let data = read_packet(&mut connection).unwrap();
        println!("{:?}", data);
    }
}

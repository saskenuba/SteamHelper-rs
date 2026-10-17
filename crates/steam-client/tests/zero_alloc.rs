//! Deserialization doesn't allocate: from the frames read on the socket to parsed packets, and their bodies when they
//! have a fixed layout.
//!
//! A global allocator counts the allocations of the thread being measured, so this is a test binary of its own.
//! Buffers are set up before measuring, as on a connection that has been reading: the read buffer already yielded a
//! frame, which makes it shared (see `the_first_frame_shares_the_read_buffer`), and the queue of packets unpacked from
//! a `Multi` has room.
//!
//! Protobuf bodies still allocate (strings, repeated fields, nested messages): they are measured, not asserted on.
//! Run with `--nocapture` to see the counts.

#![allow(unsafe_code)]

use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::alloc::System;
use std::cell::Cell;
use std::collections::VecDeque;
use std::io::Write;

use bytes::Bytes;
use bytes::BytesMut;
use flate2::write::GzEncoder;
use flate2::Compression;
use steam_client::__private::decode_plain;
use steam_client::__private::EncryptedCodec;
use steam_client::__private::Packet;
use steam_crypto::symmetric_encrypt_hmac_iv;
use steam_crypto::SessionKey;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::generated::messages::MsgChannelEncryptRequest;
use steam_language_gen::generated::messages::MsgClientChatEnter;
use steam_language_gen::MessageHeaderWrapper;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AllowedConfirmation;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Response;
use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

thread_local! {
    /// Allocations made by this thread, while measuring.
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct CountingAllocator;

impl CountingAllocator {
    fn count(&self) {
        // fails while the thread is torn down, when nothing is measured anyway
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get().map(|count| count + 1)));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.count();
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.count();
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.count();
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

/// Runs `f`, and returns its result with how many allocations it made on this thread.
fn allocations<R>(f: impl FnOnce() -> R) -> (R, usize) {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let result = f();
    let count = ALLOCATIONS.with(|count| count.take()).unwrap();
    (result, count)
}

/// ChannelEncryptRequest captured from a CM, with a standard header. Also in `connection/encryption.rs`.
const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
    23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0, 0,
    0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
];

/// ClientChatEnter, with an extended header. Also in `messages/message.rs`.
const CLIENT_CHAT_ENTER: [u8; 353] = [
    0x27, 0x03, 0x00, 0x00, 0x24, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xEF, 0xAC, 0x15, 0x89, 0x00, 0x01, 0x00, 0x10, 0x01, 0x8E, 0x56, 0x11, 0x00, 0xBC, 0x4E,
    0x2A, 0x00, 0x00, 0x00, 0x88, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0xBC,
    0x4E, 0x2A, 0x00, 0x00, 0x00, 0x70, 0x01, 0xBC, 0x4E, 0x2A, 0x00, 0x00, 0x00, 0x70, 0x01, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x04, 0x00, 0x00, 0x00, 0x53, 0x61, 0x78, 0x74, 0x6F, 0x6E, 0x20, 0x48, 0x65, 0x6C, 0x6C, 0x00, 0x00, 0x4D,
    0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D,
    0x69, 0x64, 0x00, 0xAC, 0x15, 0x89, 0x00, 0x01, 0x00, 0x10, 0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73,
    0x69, 0x6F, 0x6E, 0x73, 0x00, 0x7B, 0x03, 0x00, 0x00, 0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x4D, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74,
    0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D, 0x69, 0x64, 0x00, 0x00, 0x28, 0x90, 0x00, 0x01, 0x00, 0x10, 0x01, 0x02,
    0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00, 0x08, 0x00, 0x00, 0x00, 0x02, 0x44, 0x65,
    0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x04, 0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x4D, 0x65, 0x73, 0x73, 0x61, 0x67,
    0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00, 0x07, 0x73, 0x74, 0x65, 0x61, 0x6D, 0x69, 0x64, 0x00, 0xB0, 0xDC,
    0x5B, 0x04, 0x01, 0x00, 0x10, 0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00,
    0x08, 0x00, 0x00, 0x00, 0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08,
    0x00, 0x4D, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x4F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x00, 0x07, 0x73, 0x74, 0x65,
    0x61, 0x6D, 0x69, 0x64, 0x00, 0x39, 0xCB, 0x77, 0x05, 0x01, 0x00, 0x10, 0x01, 0x02, 0x70, 0x65, 0x72, 0x6D, 0x69,
    0x73, 0x73, 0x69, 0x6F, 0x6E, 0x73, 0x00, 0x1A, 0x03, 0x00, 0x00, 0x02, 0x44, 0x65, 0x74, 0x61, 0x69, 0x6C, 0x73,
    0x00, 0x02, 0x00, 0x00, 0x00, 0x08, 0x08, 0xE8, 0x03, 0x00, 0x00,
];

/// Anonymous ClientLogOnResponse captured from a CM, with a protobuf header. Also in `client.rs`, which documents it.
const LOGON_RESPONSE: [u8; 99] = [
    0xEF, 0x02, 0x00, 0x80, 0x14, 0x00, 0x00, 0x00, 0x09, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0x10, 0xE3,
    0x82, 0xD5, 0xBD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x08, 0x01, 0x10, 0x09, 0x18, 0x09, 0x20, 0x87, 0xE2, 0x81,
    0xD8, 0x0C, 0x2D, 0x7B, 0xFB, 0xD2, 0x6A, 0x30, 0x00, 0x38, 0x19, 0x60, 0xFF, 0xFF, 0xFF, 0xFF, 0x07, 0x72, 0x00,
    0x7A, 0x05, 0x0D, 0x07, 0x71, 0x00, 0xCB, 0xA1, 0x01, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0xAA, 0x01,
    0x02, 0x55, 0x53, 0xC0, 0x01, 0x00, 0xC8, 0x01, 0x00, 0xD8, 0x01, 0xC4, 0xD2, 0xC7, 0xD9, 0xD1, 0xF0, 0xA3, 0x8B,
    0x05, 0xE0, 0x01, 0x00,
];

fn key() -> SessionKey {
    SessionKey::from([3; 32])
}

/// A frame as it comes from the socket: the message length, the magic, then the message.
fn framed(message: &[u8]) -> Vec<u8> {
    let mut frame = (message.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(b"VT01");
    frame.extend_from_slice(message);
    frame
}

fn encrypted(message: &[u8]) -> Vec<u8> {
    framed(&symmetric_encrypt_hmac_iv(message, &key()))
}

/// A read buffer holding `frame`, after it yielded a first frame.
fn read_buffer(first: Vec<u8>, frame: Vec<u8>, decode: impl FnOnce(&mut BytesMut)) -> BytesMut {
    let mut src = BytesMut::from(&[first, frame].concat()[..]);
    decode(&mut src);
    src
}

fn plain_read_buffer(message: &[u8]) -> BytesMut {
    read_buffer(framed(b"first"), framed(message), |src| {
        decode_plain(src).unwrap().unwrap();
    })
}

fn encrypted_read_buffer(codec: &mut EncryptedCodec, message: &[u8]) -> BytesMut {
    read_buffer(encrypted(b"first"), encrypted(message), |src| {
        codec.decode(src).unwrap().unwrap();
    })
}

/// A queue of unpacked packets with room, as after a first `Multi`.
fn pending() -> VecDeque<Bytes> {
    VecDeque::with_capacity(16)
}

fn proto_packet<M: Message>(emsg: EMsg, header: CMsgProtoBufHeader, body: &M) -> Vec<u8> {
    let mut packet = vec![];
    MessageHeaderWrapper::Proto(header).write(emsg, &mut packet);
    packet.extend(body.write_to_bytes().unwrap());
    packet
}

fn multi(packets: &[&[u8]], gzip: bool) -> Vec<u8> {
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
    proto_packet(EMsg::Multi, CMsgProtoBufHeader::new(), &multi)
}

/// Checks the bodies with a fixed layout, as a check that parsing ran.
fn check_body(emsg: EMsg, packet: Packet) {
    match emsg {
        EMsg::ChannelEncryptRequest => {
            let (body, nonce) = packet.parse_body::<MsgChannelEncryptRequest>().unwrap();
            // a packed struct: copied out instead of borrowed
            let universe = body.universe;
            assert_eq!(universe, EUniverse::Public);
            assert_eq!(nonce.len(), 16);
        }
        EMsg::ClientChatEnter => {
            let (_, payload) = packet.parse_body::<MsgClientChatEnter>().unwrap();
            assert!(payload.starts_with(b"Saxton Hell\0"));
        }
        _ => assert!(matches!(packet.header(), MessageHeaderWrapper::Proto(_))),
    }
}

#[test]
fn plain_packets_dont_allocate() {
    let cases = [
        (EMsg::ChannelEncryptRequest, &CHANNEL_ENCRYPT_REQUEST[..]),
        (EMsg::ClientChatEnter, &CLIENT_CHAT_ENTER),
        (EMsg::ClientLogOnResponse, &LOGON_RESPONSE),
    ];
    for (emsg, message) in cases {
        let mut src = plain_read_buffer(message);

        let ((), count) = allocations(|| {
            let frame = decode_plain(&mut src).unwrap().unwrap();
            let packet = Packet::from_bytes(frame).unwrap();
            assert_eq!(packet.emsg(), emsg);
            check_body(emsg, packet);
        });
        println!("plain {:?}: {} allocations", emsg, count);
        assert_eq!(count, 0, "plain {:?}", emsg);
    }
}

#[test]
fn encrypted_packets_dont_allocate() {
    let cases = [
        (EMsg::ChannelEncryptRequest, &CHANNEL_ENCRYPT_REQUEST[..]),
        (EMsg::ClientChatEnter, &CLIENT_CHAT_ENTER),
        (EMsg::ClientLogOnResponse, &LOGON_RESPONSE),
    ];
    for (emsg, message) in cases {
        let mut codec = EncryptedCodec::new(key());
        let mut src = encrypted_read_buffer(&mut codec, message);
        let mut pending = pending();

        let ((), count) = allocations(|| {
            let frame = codec.decode(&mut src).unwrap().unwrap();
            let packet = Packet::parse(frame, &mut pending).unwrap();
            assert_eq!(packet.emsg(), emsg);
            check_body(emsg, packet);
        });
        println!("encrypted {:?}: {} allocations", emsg, count);
        assert_eq!(count, 0, "encrypted {:?}", emsg);
    }
}

#[test]
fn skipping_unknown_messages_doesnt_allocate() {
    let mut unknown = (0x7FFF_0000u32 | 0x8000_0000).to_le_bytes().to_vec();
    unknown.extend_from_slice(&[0; 4]);
    let mut codec = EncryptedCodec::new(key());
    let mut src = encrypted_read_buffer(&mut codec, &unknown);
    let mut pending = pending();

    let (packet, count) = allocations(|| Packet::parse(codec.decode(&mut src).unwrap().unwrap(), &mut pending));
    assert!(packet.is_none());
    assert_eq!(count, 0);
}

/// Reads an encrypted Multi, and the packets it carries. Returns their EMsgs, and how many allocations it took.
fn read_multi(multi: &[u8]) -> (Vec<EMsg>, usize) {
    let mut codec = EncryptedCodec::new(key());
    let mut src = encrypted_read_buffer(&mut codec, multi);
    let mut pending = pending();
    let mut emsgs = Vec::with_capacity(16);

    let ((), count) = allocations(|| {
        let frame = codec.decode(&mut src).unwrap().unwrap();
        assert!(Packet::parse(frame, &mut pending).is_none());
        while let Some(raw) = pending.pop_front() {
            if let Some(packet) = Packet::parse(raw, &mut pending) {
                let emsg = packet.emsg();
                check_body(emsg, packet);
                emsgs.push(emsg);
            }
        }
    });
    (emsgs, count)
}

const MULTI_CONTENTS: [EMsg; 4] = [
    EMsg::ClientLogOnResponse,
    EMsg::ClientChatEnter,
    EMsg::ChannelEncryptRequest,
    EMsg::ClientLogOnResponse,
];

#[test]
fn raw_multis_dont_allocate() {
    let nested = multi(&[&CHANNEL_ENCRYPT_REQUEST, &LOGON_RESPONSE], false);
    let (emsgs, count) = read_multi(&multi(&[&LOGON_RESPONSE, &CLIENT_CHAT_ENTER, &nested], false));

    println!("raw Multi: {} allocations", count);
    assert_eq!(emsgs, MULTI_CONTENTS);
    assert_eq!(count, 0);
}

#[test]
fn gzipped_multis_allocate_their_buffer_only() {
    let nested = multi(&[&CHANNEL_ENCRYPT_REQUEST, &LOGON_RESPONSE], false);
    let (emsgs, count) = read_multi(&multi(&[&LOGON_RESPONSE, &CLIENT_CHAT_ENTER, &nested], true));

    println!("gzipped Multi: {} allocations", count);
    assert_eq!(emsgs, MULTI_CONTENTS);
    // the unzipped buffer, and its reference count: `bytes` allocates one when a buffer is first shared, here by
    // the packets sliced from it
    assert_eq!(count, 2);
}

#[test]
fn hostile_unzipped_sizes_dont_allocate() {
    let mut multi = CMsgMulti::new();
    multi.set_size_unzipped(u32::MAX);
    multi.set_message_body(Bytes::from_static(&[0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 0xFF, 3, 0]));
    let multi = proto_packet(EMsg::Multi, CMsgProtoBufHeader::new(), &multi);

    let (emsgs, count) = read_multi(&multi);
    assert!(emsgs.is_empty());
    assert_eq!(count, 0);
}

#[test]
fn the_first_frame_shares_the_read_buffer() {
    let mut src = BytesMut::from(&framed(&CHANNEL_ENCRYPT_REQUEST)[..]);
    let (_, count) = allocations(|| decode_plain(&mut src).unwrap().unwrap());

    // `bytes` allocates the reference count of the read buffer, once per buffer
    println!("first frame of a read buffer: {} allocations", count);
    assert!(count <= 1);
}

/// Protobuf bodies: not asserted on yet, see the module docs.
#[test]
fn measures_protobuf_bodies() {
    fn measure<M: Message>(name: &str, emsg: EMsg, packet: &[u8]) {
        let mut src = plain_read_buffer(packet);
        let packet = Packet::from_bytes(decode_plain(&mut src).unwrap().unwrap()).unwrap();
        assert_eq!(packet.emsg(), emsg);
        let (body, count) = allocations(|| packet.parse_proto::<M>());
        body.unwrap();
        println!("protobuf body {}: {} allocations", name, count);
    }

    measure::<CMsgClientLogonResponse>("CMsgClientLogonResponse", EMsg::ClientLogOnResponse, &LOGON_RESPONSE);

    let mut logged_off = CMsgClientLoggedOff::new();
    logged_off.set_eresult(1);
    let packet = proto_packet(EMsg::ClientLoggedOff, CMsgProtoBufHeader::new(), &logged_off);
    measure::<CMsgClientLoggedOff>("CMsgClientLoggedOff", EMsg::ClientLoggedOff, &packet);

    let packet = multi(&[&LOGON_RESPONSE], false);
    measure::<CMsgMulti>("CMsgMulti", EMsg::Multi, &packet);

    // what account logon will read: a service method response
    let mut response = CAuthentication_BeginAuthSessionViaCredentials_Response::new();
    response.set_client_id(1234);
    response.set_request_id(Bytes::from_static(&[7; 16]));
    response.set_interval(5.0);
    response.set_steamid(76561197960265728);
    response.set_weak_token("eyJhbGciOiJFZERTQSJ9.weak".into());
    for associated_message in ["", "a@example.com"] {
        let mut confirmation = CAuthentication_AllowedConfirmation::new();
        confirmation.set_associated_message(associated_message.into());
        response.allowed_confirmations.push(confirmation);
    }
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(42);
    header.set_eresult(1);
    let packet = proto_packet(EMsg::ServiceMethodResponse, header, &response);
    measure::<CAuthentication_BeginAuthSessionViaCredentials_Response>(
        "CAuthentication_BeginAuthSessionViaCredentials_Response",
        EMsg::ServiceMethodResponse,
        &packet,
    );
}

/// Strings in a protobuf header allocate, as in bodies.
#[test]
fn measures_protobuf_headers_with_strings() {
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(42);
    header.set_target_job_name("Authentication.BeginAuthSessionViaCredentials#1".into());
    let packet = proto_packet(EMsg::ServiceMethodResponse, header, &CMsgClientLoggedOff::new());
    let mut src = plain_read_buffer(&packet);

    let (packet, count) = allocations(|| Packet::from_bytes(decode_plain(&mut src).unwrap().unwrap()).unwrap());
    assert_eq!(packet.emsg(), EMsg::ServiceMethodResponse);
    println!("protobuf header with target_job_name: {} allocations", count);
}

//! The protocol driven by hand: bytes in, bytes out, and a simulated clock. No runtime, no sockets.

use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;
use steam_language_gen::generated::enums::EAccountType;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::generated::headers::ExtendedMessageHeader;
use steam_language_gen::MessageHeaderExt;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;

use super::*;
use crate::errors::PacketError;
use crate::service::methods::GenerateAccessTokenForApp;
use crate::service::methods::GetPasswordRSAPublicKey;

/// ChannelEncryptRequest captured from a CM.
const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
    23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0, 0,
    0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
];

/// `ClientLogOnResponse` to an anonymous logon, captured from a CM (decrypted, after unpacking its `Multi`).
///
/// The public IP (in `public_ip` and `deprecated_public_ip`) was replaced with 203.0.113.7, a documentation address,
/// and `ip_country_code` with `US`. The SteamID and session id are the ones Steam assigned to this anonymous session.
pub(crate) const ANONYMOUS_LOGON_RESPONSE: [u8; 99] = [
    0xEF, 0x02, 0x00, 0x80, 0x14, 0x00, 0x00, 0x00, 0x09, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0x10, 0xE3,
    0x82, 0xD5, 0xBD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x08, 0x01, 0x10, 0x09, 0x18, 0x09, 0x20, 0x87, 0xE2, 0x81,
    0xD8, 0x0C, 0x2D, 0x7B, 0xFB, 0xD2, 0x6A, 0x30, 0x00, 0x38, 0x19, 0x60, 0xFF, 0xFF, 0xFF, 0xFF, 0x07, 0x72, 0x00,
    0x7A, 0x05, 0x0D, 0x07, 0x71, 0x00, 0xCB, 0xA1, 0x01, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0xAA, 0x01,
    0x02, 0x55, 0x53, 0xC0, 0x01, 0x00, 0xC8, 0x01, 0x00, 0xD8, 0x01, 0xC4, 0xD2, 0xC7, 0xD9, 0xD1, 0xF0, 0xA3, 0x8B,
    0x05, 0xE0, 0x01, 0x00,
];
/// The session of [`ANONYMOUS_LOGON_RESPONSE`].
pub(crate) const ANONYMOUS_STEAM_ID: u64 = 117316669797350672;
const ANONYMOUS_SESSION_ID: i32 = -139116189;
/// Its heartbeat interval.
const HEARTBEAT: Duration = Duration::from_secs(9);

/// Captures what the calling thread logs, at every level, so that log arguments run and tests can check what is
/// logged.
pub(crate) mod logs {
    use std::cell::RefCell;
    use std::sync::Once;

    thread_local! {
        static LINES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    struct Capture;

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            let line = format!("{} {}", record.level(), record.args());
            // fails while the thread is torn down, when nobody reads the lines anyway
            let _ = LINES.try_with(|lines| lines.borrow_mut().push(line));
        }

        fn flush(&self) {}
    }

    /// Starts capturing, for the whole test binary, and forgets what this thread logged so far.
    pub(crate) fn capture() {
        static START: Once = Once::new();
        START.call_once(|| {
            log::set_logger(&Capture).expect("no other logger");
            log::set_max_level(log::LevelFilter::Trace);
        });
        take();
    }

    /// What this thread logged since the last call.
    pub(crate) fn take() -> Vec<String> {
        LINES.with(|lines| lines.take())
    }

    /// Checks that nothing logged since the last call contains one of `secrets`.
    pub(crate) fn assert_none_contains(secrets: &[&str]) {
        let lines = take();
        assert!(!lines.is_empty(), "nothing was logged");
        for line in lines {
            for secret in secrets {
                assert!(!line.contains(secret), "logged a secret: {}", line);
            }
        }
    }
}

fn key() -> SessionKey {
    SessionKey::from([9; 32])
}

/// A message as Steam sends it once the channel is encrypted.
fn from_steam(message: &[u8]) -> Vec<u8> {
    framing::encrypt(message, &key())
}

/// A message nothing waits for, told apart by `version`.
fn hello(version: u32) -> Vec<u8> {
    let mut hello = CMsgClientHello::new();
    hello.set_protocol_version(version);
    proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), hello)
}

/// The version of a hello.
fn version(event: &Event) -> Option<u32> {
    match event {
        Event::Message(packet) if packet.emsg() == EMsg::ClientHello => {
            Some(packet.parse_proto::<CMsgClientHello>().unwrap().protocol_version())
        }
        _ => None,
    }
}

/// The versions of the hellos kept as events, in order. Fails on other events.
fn hellos(protocol: &mut CmProtocol) -> Vec<u32> {
    let mut versions = vec![];
    while let Some(event) = protocol.poll_event() {
        match event {
            Event::Message(packet) if packet.emsg() == EMsg::ClientHello => {
                versions.push(packet.parse_proto::<CMsgClientHello>().unwrap().protocol_version())
            }
            event => panic!("expected a hello, got {:?}", event),
        }
    }
    versions
}

fn encrypt_result(result: EResult) -> Vec<u8> {
    let mut raw = (EMsg::ChannelEncryptResult as u32).to_le_bytes().to_vec();
    raw.extend_from_slice(&[0xFF; 16]);
    raw.extend_from_slice(&(result as u32).to_le_bytes());
    raw
}

fn rsa_key_request() -> CAuthentication_GetPasswordRSAPublicKey_Request {
    let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
    request.set_account_name("account".to_string());
    request
}

/// A response to job `job_id`, told apart by `name`.
fn response(job_id: u64, result: EResult, name: &str) -> Vec<u8> {
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(job_id);
    header.set_eresult(result as i32);
    let mut body = CAuthentication_GetPasswordRSAPublicKey_Response::new();
    body.set_publickey_mod(name.to_string());
    proto_message(EMsg::ServiceMethodResponse, header, body)
}

fn logged_off(result: EResult) -> Vec<u8> {
    let mut body = CMsgClientLoggedOff::new();
    body.set_eresult(result as i32);
    proto_message(EMsg::ClientLoggedOff, CMsgProtoBufHeader::new(), body)
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
    proto_message(EMsg::Multi, CMsgProtoBufHeader::new(), multi)
}

/// A message with `header` and `body` as they are, not checked.
fn raw_message(emsg: EMsg, header: MessageHeaderWrapper, body: &[u8]) -> Vec<u8> {
    let mut raw = vec![];
    header.write(emsg, &mut raw);
    raw.extend_from_slice(body);
    raw
}

fn extended_header() -> MessageHeaderWrapper {
    MessageHeaderWrapper::Ext(ExtendedMessageHeader::create())
}

fn unknown_emsg() -> Vec<u8> {
    let mut raw = (0x7FFF_0000u32 | 0x8000_0000).to_le_bytes().to_vec();
    raw.extend_from_slice(&[0, 0, 0, 0]);
    raw
}

/// What the protocol gave to write next: one frame, encrypted with `key`.
/// Everything the protocol gave to write: frames encrypted with `key`.
fn all_sent_with(protocol: &mut CmProtocol, key: &SessionKey) -> Vec<PacketMessage> {
    let mut src = BytesMut::new();
    while let Some(transmit) = protocol.poll_transmit() {
        src.extend_from_slice(&transmit);
    }
    let mut messages = vec![];
    while let Some(frame) = framing::decode(&mut src).unwrap() {
        messages.push(PacketMessage::from_bytes(framing::decrypt(frame, key).unwrap()).unwrap());
    }
    assert!(src.is_empty(), "whole frames");
    messages
}

/// The one message the protocol gave to write.
fn sent_with(protocol: &mut CmProtocol, key: &SessionKey) -> PacketMessage {
    let mut messages = all_sent_with(protocol, key);
    assert_eq!(messages.len(), 1, "one message to write");
    messages.remove(0)
}

fn sent(protocol: &mut CmProtocol) -> PacketMessage {
    sent_with(protocol, &key())
}

fn proto_header(packet: &PacketMessage) -> &CMsgProtoBufHeader {
    match packet.header() {
        MessageHeaderWrapper::Proto(header) => header,
        header => panic!("expected a protobuf header, got {:?}", header),
    }
}

/// Logged on anonymously at `now`, with the captured response.
fn logged_on(now: Instant) -> CmProtocol {
    let mut protocol = CmProtocol::encrypted(key());
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
    sent(&mut protocol);
    protocol
        .handle_input(&from_steam(&ANONYMOUS_LOGON_RESPONSE), now)
        .unwrap();
    assert!(protocol.logon_result().unwrap().is_ok());
    protocol
}

fn assert_session(packet: &PacketMessage) {
    let header = proto_header(packet);
    assert_eq!(header.steamid(), ANONYMOUS_STEAM_ID);
    assert_eq!(header.client_sessionid(), ANONYMOUS_SESSION_ID);
}

#[test]
fn encrypts_the_channel() {
    let now = Instant::now();
    let mut protocol = CmProtocol::new(Transport::Tcp);
    assert!(!protocol.is_connected());
    // sent before the channel is encrypted: kept until it is
    protocol.send(EMsg::ClientHello, CMsgClientHello::new());
    assert!(protocol.poll_transmit().is_none());

    protocol
        .handle_input(&framing::frame(&CHANNEL_ENCRYPT_REQUEST), now)
        .unwrap();
    let response = protocol.poll_transmit().unwrap();
    let response = framing::decode(&mut BytesMut::from(&response[..])).unwrap().unwrap();
    let response = PacketMessage::from_bytes(response.freeze()).unwrap();
    assert_eq!(response.emsg(), EMsg::ChannelEncryptResponse);
    assert!(protocol.poll_transmit().is_none());
    assert!(!protocol.is_connected());
    assert!(protocol.session_key().is_none());

    // the result, and a message right behind it in the same read, encrypted with the new key
    let key = match &protocol.channel {
        Channel::AwaitingEncryptResult(encryption) => encryption.key().clone(),
        channel => panic!("expected to wait for the result, in {:?}", channel),
    };
    let input = [
        framing::frame(&encrypt_result(EResult::OK)),
        framing::encrypt(&hello(2), &key),
    ]
    .concat();
    protocol.handle_input(&input, now).unwrap();
    assert!(protocol.is_connected());
    assert_eq!(protocol.session_key(), Some(&key));
    assert_eq!(hellos(&mut protocol), [2]);

    // the hello sent before goes out now, encrypted
    assert_eq!(sent_with(&mut protocol, &key).emsg(), EMsg::ClientHello);
    assert!(protocol.poll_transmit().is_none());
}

#[test]
fn skips_other_messages_during_the_handshake() {
    let now = Instant::now();
    let mut protocol = CmProtocol::new(Transport::Tcp);

    // a result before the request, and an unrelated message
    let input = [
        framing::frame(&encrypt_result(EResult::OK)),
        framing::frame(&hello(1)),
        framing::frame(&CHANNEL_ENCRYPT_REQUEST),
    ]
    .concat();
    protocol.handle_input(&input, now).unwrap();
    assert!(protocol.poll_transmit().is_some());

    // a second request isn't answered
    protocol
        .handle_input(&framing::frame(&CHANNEL_ENCRYPT_REQUEST), now)
        .unwrap();
    assert!(protocol.poll_transmit().is_none());
    assert!(protocol.poll_event().is_none());

    protocol
        .handle_input(&framing::frame(&encrypt_result(EResult::OK)), now)
        .unwrap();
    assert!(protocol.is_connected());
}

#[test]
fn rejects_other_universes() {
    let mut request = CHANNEL_ENCRYPT_REQUEST;
    request[24] = EUniverse::Beta as u8;
    let mut protocol = CmProtocol::new(Transport::Tcp);

    assert!(matches!(
        protocol.handle_input(&framing::frame(&request), Instant::now()),
        Err(ClientError::UnsupportedUniverse(EUniverse::Beta))
    ));
    assert!(protocol.poll_transmit().is_none());
    // nothing more is taken
    assert!(matches!(
        protocol.handle_input(&framing::frame(&CHANNEL_ENCRYPT_REQUEST), Instant::now()),
        Err(ClientError::Dropped)
    ));
}

#[test]
fn refused_encryption_is_an_error() {
    let mut protocol = CmProtocol::new(Transport::Tcp);
    protocol
        .handle_input(&framing::frame(&CHANNEL_ENCRYPT_REQUEST), Instant::now())
        .unwrap();

    assert!(matches!(
        protocol.handle_input(&framing::frame(&encrypt_result(EResult::Fail)), Instant::now()),
        Err(ClientError::EncryptionFailed(EResult::Fail))
    ));
    assert!(!protocol.is_connected());
}

#[test]
fn garbage_during_the_handshake_is_an_error() {
    let mut framed = framing::frame(&CHANNEL_ENCRYPT_REQUEST);
    framed[4] = b'X';
    assert!(matches!(
        CmProtocol::new(Transport::Tcp).handle_input(&framed, Instant::now()),
        Err(ClientError::Packet(PacketError::Malformed))
    ));

    // too short to be a message
    assert!(matches!(
        CmProtocol::new(Transport::Tcp).handle_input(&framing::frame(&[1, 2]), Instant::now()),
        Err(ClientError::Message(_))
    ));
}

#[test]
fn a_frame_that_fails_to_decrypt_ends_the_connection() {
    let now = Instant::now();
    let mut protocol = CmProtocol::encrypted(key());
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    sent(&mut protocol);

    // a plain frame, then one encrypted with another key: the first ends it
    let input = [
        framing::frame(&hello(1)),
        framing::encrypt(&hello(2), &SessionKey::from([1; 32])),
    ]
    .concat();
    assert!(matches!(
        protocol.handle_input(&input, now),
        Err(ClientError::Packet(PacketError::Decrypt(_)))
    ));
    assert!(matches!(protocol.take_response(&call), Some(Err(ClientError::Dropped))));

    // later input and calls fail at once, and nothing is sent anymore
    assert!(matches!(
        protocol.handle_input(&from_steam(&hello(3)), now),
        Err(ClientError::Dropped)
    ));
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    assert!(matches!(protocol.take_response(&call), Some(Err(ClientError::Dropped))));
    protocol.send(EMsg::ClientHello, CMsgClientHello::new());
    assert!(protocol.poll_transmit().is_none());
    assert!(protocol.poll_event().is_none());

    // so do logging on and off
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
    assert!(protocol.poll_transmit().is_none());
    assert!(matches!(protocol.logon_result(), Some(Err(ClientError::Dropped))));
    protocol.log_off();
    assert!(matches!(protocol.logged_off(), Some(Err(ClientError::Dropped))));
}

#[test]
fn garbage_after_the_handshake_is_an_error() {
    let mut framed = from_steam(&hello(1));
    framed[5] = b'X';
    assert!(matches!(
        CmProtocol::encrypted(key()).handle_input(&framed, Instant::now()),
        Err(ClientError::Packet(PacketError::Malformed))
    ));
}

#[test]
fn reads_frames_split_anywhere() {
    let stream = [from_steam(&hello(1)), from_steam(&hello(2)), from_steam(&hello(3))].concat();

    // a byte at a time
    let mut protocol = CmProtocol::encrypted(key());
    for byte in &stream {
        protocol.handle_input(&[*byte], Instant::now()).unwrap();
    }
    assert_eq!(hellos(&mut protocol), [1, 2, 3]);

    // in two reads, cut anywhere
    for cut in 0..=stream.len() {
        let mut protocol = CmProtocol::encrypted(key());
        protocol.handle_input(&stream[..cut], Instant::now()).unwrap();
        protocol.handle_input(&stream[cut..], Instant::now()).unwrap();
        assert_eq!(hellos(&mut protocol), [1, 2, 3], "cut at {}", cut);
    }
}

#[test]
fn reads_frames_read_together() {
    let mut protocol = CmProtocol::encrypted(key());
    let input = [from_steam(&hello(1)), from_steam(&hello(2))].concat();
    protocol.handle_input(&input, Instant::now()).unwrap();
    assert_eq!(hellos(&mut protocol), [1, 2]);
}

#[test]
fn unpacks_multis_in_order() {
    let mut protocol = CmProtocol::encrypted(key());
    let nested = multi(&[hello(4), hello(5)], false);
    let input = [
        from_steam(&multi(&[hello(1), hello(2)], false)),
        from_steam(&multi(&[hello(3), nested, hello(6)], true)),
        from_steam(&hello(7)),
    ]
    .concat();

    protocol.handle_input(&input, Instant::now()).unwrap();
    assert_eq!(hellos(&mut protocol), [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn skips_what_it_cant_parse() {
    let mut protocol = CmProtocol::encrypted(key());
    let truncated = {
        let mut multi = CMsgMulti::new();
        multi.set_message_body(vec![9, 0, 0, 0, 1].into());
        proto_message(EMsg::Multi, CMsgProtoBufHeader::new(), multi)
    };
    let bad_gzip = {
        let mut multi = CMsgMulti::new();
        multi.set_size_unzipped(10);
        multi.set_message_body(vec![1, 2, 3].into());
        proto_message(EMsg::Multi, CMsgProtoBufHeader::new(), multi)
    };
    let input = [
        from_steam(&unknown_emsg()),
        from_steam(&[1, 2]),
        from_steam(&truncated),
        from_steam(&bad_gzip),
        // a Multi needs a protobuf header and body
        from_steam(&raw_message(EMsg::Multi, extended_header(), &[])),
        from_steam(&raw_message(
            EMsg::Multi,
            MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()),
            &[0xFF, 0xFF],
        )),
        // inside a Multi too
        from_steam(&multi(&[unknown_emsg(), hello(1), vec![1]], false)),
        from_steam(&hello(2)),
    ]
    .concat();

    protocol.handle_input(&input, Instant::now()).unwrap();
    assert_eq!(hellos(&mut protocol), [1, 2]);
}

#[test]
fn job_ids_count_from_one() {
    let mut jobs = JobIds::new();
    assert_eq!((jobs.next(), jobs.next()), (1, 2));
}

#[test]
fn matches_responses_to_their_calls_and_keeps_the_rest() {
    let mut protocol = CmProtocol::encrypted(key());
    let first = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    let second = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    for (job_id, call) in (1..).zip(all_sent_with(&mut protocol, &key())) {
        assert_eq!(call.emsg(), EMsg::ServiceMethodCallFromClientNonAuthed);
        let header = proto_header(&call);
        assert_eq!(header.jobid_source(), job_id);
        assert_eq!(header.target_job_name(), "Authentication.GetPasswordRSAPublicKey#1");
        assert!(!header.has_steamid());
    }
    assert!(protocol.take_response(&first).is_none());

    let input = [
        from_steam(&hello(1)),
        from_steam(&response(2, EResult::OK, "second")),
        from_steam(&response(99, EResult::OK, "unknown")),
        from_steam(&hello(2)),
        from_steam(&response(1, EResult::OK, "first")),
    ]
    .concat();
    protocol.handle_input(&input, Instant::now()).unwrap();

    assert_eq!(
        protocol.take_response(&second).unwrap().unwrap().publickey_mod(),
        "second"
    );
    assert_eq!(
        protocol.take_response(&first).unwrap().unwrap().publickey_mod(),
        "first"
    );
    assert!(protocol.take_response(&first).is_none());

    // everything else is kept, in order
    let events: Vec<_> = std::iter::from_fn(|| protocol.poll_event()).collect();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], Event::Message(packet) if packet.emsg() == EMsg::ClientHello));
    assert!(matches!(&events[1], Event::Message(packet) if packet.parse().unwrap().header().target() == 99));
    assert!(matches!(&events[2], Event::Message(packet) if packet.emsg() == EMsg::ClientHello));
}

#[test]
fn failed_calls_are_errors() {
    let mut protocol = CmProtocol::encrypted(key());
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());

    protocol
        .handle_input(
            &from_steam(&response(1, EResult::RateLimitExceeded, "")),
            Instant::now(),
        )
        .unwrap();
    assert!(matches!(
        protocol.take_response(&call),
        Some(Err(ClientError::ServiceMethod {
            method: "Authentication.GetPasswordRSAPublicKey#1",
            result: EResult::RateLimitExceeded,
            ..
        }))
    ));
}

#[test]
fn a_second_response_to_a_job_is_an_event() {
    let mut protocol = CmProtocol::encrypted(key());
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    let input = [
        from_steam(&response(1, EResult::OK, "first")),
        from_steam(&response(1, EResult::OK, "second")),
    ]
    .concat();
    protocol.handle_input(&input, Instant::now()).unwrap();

    assert_eq!(protocol.take_response(&call).unwrap().unwrap().publickey_mod(), "first");
    assert!(
        matches!(protocol.poll_event(), Some(Event::Message(packet)) if packet.parse().unwrap().header().target() == 1)
    );
}

#[test]
fn a_forgotten_calls_response_is_an_event() {
    let mut protocol = CmProtocol::encrypted(key());
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    assert_eq!(call.job_id(), 1);
    protocol.forget(call.job_id());

    protocol
        .handle_input(&from_steam(&response(1, EResult::OK, "late")), Instant::now())
        .unwrap();
    assert!(
        matches!(protocol.poll_event(), Some(Event::Message(packet)) if packet.parse().unwrap().header().target() == 1)
    );
}

#[test]
fn logs_on() {
    let now = Instant::now();
    let mut protocol = CmProtocol::encrypted(key());
    let mut logon = CMsgClientLogon::new();
    logon.set_protocol_version(65581);
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), logon);

    let message = sent(&mut protocol);
    assert_eq!(message.emsg(), EMsg::ClientLogon);
    let header = proto_header(&message);
    assert_eq!(header.steamid(), 0x01A0_0000_0000_0000);
    assert!(header.has_client_sessionid());
    assert_eq!(header.client_sessionid(), 0);
    assert_eq!(header.jobid_source(), u64::MAX);
    assert_eq!(
        ClientMessage::<CMsgClientLogon>::from_proto_packet(message)
            .unwrap()
            .body
            .protocol_version(),
        65581
    );
    assert!(protocol.logon_result().is_none());
    assert!(protocol.poll_timeout().is_none());

    // as Steam sends it: in a gzipped Multi, behind another message
    protocol
        .handle_input(
            &from_steam(&multi(&[hello(1), ANONYMOUS_LOGON_RESPONSE.to_vec()], true)),
            now,
        )
        .unwrap();
    let logon = protocol.logon_result().unwrap().unwrap();
    assert_eq!(logon.session.steam_id.to_steam64(), ANONYMOUS_STEAM_ID);
    assert_eq!(logon.session.session_id, ANONYMOUS_SESSION_ID);
    assert_eq!(logon.heartbeat_interval, HEARTBEAT);
    assert_eq!(logon.cell_id, 25);
    assert_eq!(logon.public_ip, Some(Ipv4Addr::new(203, 0, 113, 7)));
    assert_eq!(hellos(&mut protocol), [1]);
}

#[test]
fn parses_a_captured_anonymous_logon_response() {
    let packet = PacketMessage::from_bytes(Bytes::from_static(&ANONYMOUS_LOGON_RESPONSE)).unwrap();
    assert_eq!(packet.emsg(), EMsg::ClientLogOnResponse);

    let logon = logon_result(packet.clone()).unwrap();
    assert_eq!(logon.session.steam_id.account_type(), Some(EAccountType::AnonUser));
    assert_eq!(logon.session.steam_id.universe(), Some(EUniverse::Public));

    let response = ClientMessage::<CMsgClientLogonResponse>::from_proto_packet(packet)
        .unwrap()
        .body;
    assert_eq!(response.eresult(), EResult::OK as i32);
    assert_eq!(response.client_supplied_steamid(), ANONYMOUS_STEAM_ID);
}

#[test]
fn refused_logons_are_errors() {
    let mut protocol = CmProtocol::encrypted(key());
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
    sent(&mut protocol);

    let mut response = CMsgClientLogonResponse::new();
    response.set_eresult(EResult::ServiceUnavailable as i32);
    response.set_eresult_extended(EResult::TryAnotherCM as i32);
    let response = proto_message(EMsg::ClientLogOnResponse, CMsgProtoBufHeader::new(), response);
    protocol.handle_input(&from_steam(&response), Instant::now()).unwrap();

    assert!(matches!(
        protocol.logon_result(),
        Some(Err(ClientError::LogonFailed {
            result: EResult::ServiceUnavailable,
            extended: Some(EResult::TryAnotherCM),
        }))
    ));
    assert!(protocol.poll_timeout().is_none());
    // and calls fail at once
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    assert!(protocol.poll_transmit().is_none());
    assert!(matches!(
        protocol.take_response(&call),
        Some(Err(ClientError::LogonFailed { .. }))
    ));
}

#[test]
fn invalid_logon_responses_are_refusals() {
    let invalid = [
        // Steam sends it without a protobuf header when it got no `ClientLogon`
        raw_message(EMsg::ClientLogOnResponse, extended_header(), &[]),
        raw_message(
            EMsg::ClientLogOnResponse,
            MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()),
            &[0xFF, 0xFF],
        ),
    ];
    for response in invalid {
        let mut protocol = CmProtocol::encrypted(key());
        protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
        protocol.handle_input(&from_steam(&response), Instant::now()).unwrap();
        assert!(matches!(
            protocol.logon_result(),
            Some(Err(ClientError::LogonFailed {
                result: EResult::Invalid,
                extended: None,
            }))
        ));
    }
}

#[test]
fn a_logon_response_nothing_waits_for_is_an_event() {
    let mut protocol = CmProtocol::encrypted(key());
    protocol
        .handle_input(&from_steam(&ANONYMOUS_LOGON_RESPONSE), Instant::now())
        .unwrap();
    assert!(protocol.logon_result().is_none());
    assert!(
        matches!(protocol.poll_event(), Some(Event::Message(packet)) if packet.emsg() == EMsg::ClientLogOnResponse)
    );
}

#[test]
fn falls_back_on_the_heartbeat_interval() {
    let mut response = CMsgClientLogonResponse::new();
    assert_eq!(heartbeat_interval(&response), Duration::from_secs(1));
    response.set_heartbeat_seconds(15);
    assert_eq!(heartbeat_interval(&response), Duration::from_secs(15));
    response.set_legacy_out_of_game_heartbeat_seconds(9);
    assert_eq!(heartbeat_interval(&response), Duration::from_secs(9));
}

#[test]
fn stamps_the_session_on_later_messages() {
    let mut protocol = logged_on(Instant::now());

    protocol.send(EMsg::ClientHello, CMsgClientHello::new());
    assert_session(&sent(&mut protocol));

    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    let message = sent(&mut protocol);
    assert_eq!(message.emsg(), EMsg::ServiceMethodCallFromClient);
    assert_session(&message);
    assert_eq!(proto_header(&message).jobid_source(), call.job_id());

    protocol.log_off();
    let message = sent(&mut protocol);
    assert_eq!(message.emsg(), EMsg::ClientLogOff);
    assert_session(&message);
}

#[test]
fn sends_no_heartbeat_before_logon() {
    let now = Instant::now();
    let mut protocol = CmProtocol::encrypted(key());
    assert!(protocol.poll_timeout().is_none());
    protocol.handle_timeout(now + Duration::from_secs(3600));
    assert!(protocol.poll_transmit().is_none());

    // nor while logging on
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
    sent(&mut protocol);
    assert!(protocol.poll_timeout().is_none());
    protocol.handle_timeout(now + Duration::from_secs(3600));
    assert!(protocol.poll_transmit().is_none());
}

#[test]
fn sends_a_heartbeat_every_interval() {
    let logon = Instant::now();
    let mut protocol = logged_on(logon);
    assert_eq!(protocol.poll_timeout(), Some(logon + HEARTBEAT));

    // early does nothing
    protocol.handle_timeout(logon + HEARTBEAT - Duration::from_millis(1));
    assert!(protocol.poll_transmit().is_none());

    // an hour, waking up when asked
    let mut heartbeats = vec![];
    while let Some(deadline) = protocol
        .poll_timeout()
        .filter(|&deadline| deadline <= logon + Duration::from_secs(3600))
    {
        protocol.handle_timeout(deadline);
        while protocol.poll_transmit().is_some() {
            heartbeats.push(deadline - logon);
        }
    }
    assert_eq!(heartbeats.len(), 400);
    for (count, at) in heartbeats.iter().enumerate() {
        assert_eq!(*at, HEARTBEAT * (count as u32 + 1));
    }

    let heartbeat = {
        protocol.handle_timeout(logon + Duration::from_secs(3609));
        sent(&mut protocol)
    };
    assert_eq!(heartbeat.emsg(), EMsg::ClientHeartBeat);
    assert_session(&heartbeat);
}

#[test]
fn a_late_heartbeat_is_not_followed_by_a_burst() {
    let logon = Instant::now();
    let mut protocol = logged_on(logon);

    protocol.handle_timeout(logon + Duration::from_secs(30));
    assert!(protocol.poll_transmit().is_some());
    assert!(protocol.poll_transmit().is_none());
    assert_eq!(protocol.poll_timeout(), Some(logon + Duration::from_secs(39)));
}

#[test]
fn being_logged_off_ends_the_session() {
    let now = Instant::now();
    let mut protocol = logged_on(now);
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    sent(&mut protocol);
    assert!(protocol.logged_off().is_none());

    protocol
        .handle_input(&from_steam(&logged_off(EResult::LogonSessionReplaced)), now)
        .unwrap();
    assert!(matches!(
        protocol.take_response(&call),
        Some(Err(ClientError::LoggedOff(EResult::LogonSessionReplaced)))
    ));
    assert!(matches!(
        protocol.poll_event(),
        Some(Event::LoggedOff(EResult::LogonSessionReplaced))
    ));
    assert!(matches!(protocol.logged_off(), Some(Ok(EResult::LogonSessionReplaced))));

    // no more heartbeats, and calls fail at once
    assert!(protocol.poll_timeout().is_none());
    protocol.handle_timeout(now + Duration::from_secs(3600));
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    assert!(protocol.poll_transmit().is_none());
    assert!(matches!(
        protocol.take_response(&call),
        Some(Err(ClientError::LoggedOff(EResult::LogonSessionReplaced)))
    ));
}

#[test]
fn being_logged_off_while_logging_on_is_an_error() {
    let mut protocol = CmProtocol::encrypted(key());
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());

    // with a body that can't be parsed
    let logged_off = raw_message(
        EMsg::ClientLoggedOff,
        MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()),
        &[0xFF, 0xFF],
    );
    protocol.handle_input(&from_steam(&logged_off), Instant::now()).unwrap();
    assert!(matches!(
        protocol.logon_result(),
        Some(Err(ClientError::LoggedOff(EResult::Invalid)))
    ));
}

#[test]
fn keeps_a_bounded_number_of_events_from_earlier_reads() {
    logs::capture();
    let mut protocol = CmProtocol::encrypted(key());
    let count = EVENT_QUEUE_CAPACITY as u32 + 2;
    protocol
        .handle_input(&from_steam(&logged_off(EResult::OK)), Instant::now())
        .unwrap();
    for version in 0..count {
        protocol
            .handle_input(&from_steam(&hello(version)), Instant::now())
            .unwrap();
    }
    // the oldest were dropped, and that comes first
    assert!(matches!(protocol.poll_event(), Some(Event::Lagged(3))));
    let kept = hellos(&mut protocol);
    assert_eq!(kept.len(), EVENT_QUEUE_CAPACITY);
    assert_eq!(kept.first(), Some(&2));
    assert_eq!(kept.last(), Some(&(count - 1)));
    let dropped = logs::take()
        .into_iter()
        .filter(|line| line.contains("dropped the oldest"))
        .count();
    assert_eq!(dropped, 3);
}

#[test]
fn keeps_every_event_of_a_read_up_to_a_bound() {
    let mut protocol = CmProtocol::encrypted(key());
    protocol.handle_input(&from_steam(&hello(0)), Instant::now()).unwrap();
    // one read: the event before goes, those of the read stay
    let count = EVENT_QUEUE_CAPACITY as u32 + 40;
    let input: Vec<u8> = (1..=count).flat_map(|version| from_steam(&hello(version))).collect();
    protocol.handle_input(&input, Instant::now()).unwrap();
    assert!(matches!(protocol.poll_event(), Some(Event::Lagged(1))));
    assert_eq!(hellos(&mut protocol), (1..=count).collect::<Vec<_>>());

    // past the bound, the read's oldest go too
    let count = MAX_EVENTS_PER_READ as u32 + 3;
    let input: Vec<u8> = (0..count).flat_map(|version| from_steam(&hello(version))).collect();
    protocol.handle_input(&input, Instant::now()).unwrap();
    assert!(matches!(protocol.poll_event(), Some(Event::Lagged(3))));
    assert_eq!(hellos(&mut protocol), (3..count).collect::<Vec<_>>());
}

#[test]
fn logs_no_secrets() {
    logs::capture();
    let now = Instant::now();
    let mut protocol = CmProtocol::new(Transport::Tcp);
    protocol
        .handle_input(&framing::frame(&CHANNEL_ENCRYPT_REQUEST), now)
        .unwrap();
    let key = match &protocol.channel {
        Channel::AwaitingEncryptResult(encryption) => encryption.key().clone(),
        channel => panic!("expected to wait for the result, in {:?}", channel),
    };
    protocol
        .handle_input(&framing::frame(&encrypt_result(EResult::OK)), now)
        .unwrap();

    // tokens in a logon, a call and its response
    let mut logon = CMsgClientLogon::new();
    logon.set_access_token("logon-secret".to_string());
    protocol.log_on(SteamID::from_steam64(0x0110_0001_0000_0000), logon);
    let mut request = CAuthentication_AccessToken_GenerateForApp_Request::new();
    request.set_refresh_token("refresh-secret".to_string());
    let call = protocol.call::<GenerateAccessTokenForApp>(request);
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(call.job_id());
    header.set_eresult(EResult::OK as i32);
    let mut response = CAuthentication_AccessToken_GenerateForApp_Response::new();
    response.set_access_token("access-secret".to_string());
    let response = proto_message(EMsg::ServiceMethodResponse, header, response);
    protocol.handle_input(&framing::encrypt(&response, &key), now).unwrap();
    assert!(protocol.take_response(&call).unwrap().is_ok());

    let key_bytes = format!("{:?}", key.as_bytes());
    let key_hex: String = key.as_bytes().iter().map(|byte| format!("{:02x}", byte)).collect();
    assert!(!format!("{:?}", protocol).contains(&key_bytes[1..20]));
    logs::assert_none_contains(&[
        &key_bytes[1..20],
        &key_hex,
        "logon-secret",
        "refresh-secret",
        "access-secret",
    ]);
}

#[test]
fn over_websocket_nothing_to_write_is_none() {
    let mut protocol = CmProtocol::new(Transport::WebSocket);
    assert!(protocol.poll_transmit().is_none());
}

#[test]
fn over_websocket_messages_are_neither_framed_nor_encrypted() {
    let mut protocol = CmProtocol::new(Transport::WebSocket);
    assert!(protocol.is_connected());

    protocol.send(EMsg::ClientHello, CMsgClientHello::new());
    let sent = protocol.poll_transmit().unwrap();
    assert_eq!(
        sent,
        proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), CMsgClientHello::new())
    );

    // one message per input
    protocol.handle_input(&[], Instant::now()).unwrap();
    protocol.handle_input(&hello(1), Instant::now()).unwrap();
    protocol
        .handle_input(&multi(&[hello(2), hello(3)], true), Instant::now())
        .unwrap();
    assert_eq!(hellos(&mut protocol), [1, 2, 3]);
}

#[test]
fn debug_hides_the_session_key() {
    let protocol = CmProtocol::encrypted(key());
    let debug = format!("{:?}", protocol);
    assert!(debug.contains("Encrypted"), "{}", debug);
    assert!(!debug.contains("9, 9"), "{}", debug);
}

#[test]
fn unknown_results_are_invalid() {
    assert_eq!(eresult(1), EResult::OK);
    assert_eq!(eresult(-12345), EResult::Invalid);
}

#[test]
fn forgetting_an_answered_call_keeps_its_response_as_an_event() {
    let mut protocol = CmProtocol::encrypted(key());
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    protocol
        .handle_input(&from_steam(&response(1, EResult::OK, "answered")), Instant::now())
        .unwrap();
    assert!(protocol.poll_event().is_none());

    // as a call cancelled after its response came, before it was taken
    protocol.forget(call.job_id());
    match protocol.poll_event() {
        Some(Event::Message(message)) => assert_eq!(
            message
                .parse_proto::<CAuthentication_GetPasswordRSAPublicKey_Response>()
                .unwrap()
                .publickey_mod(),
            "answered"
        ),
        event => panic!("expected the response, got {:?}", event),
    }
    assert!(protocol.take_response(&call).is_none());
    // forgetting a call that failed, or twice, keeps nothing
    protocol.forget(call.job_id());
    let failed = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    protocol
        .handle_input(&from_steam(&logged_off(EResult::OK)), Instant::now())
        .unwrap();
    assert!(matches!(protocol.poll_event(), Some(Event::LoggedOff(EResult::OK))));
    protocol.forget(failed.job_id());
    assert!(protocol.poll_event().is_none());
}

#[test]
fn a_response_and_a_log_off_in_the_same_read_keep_the_response() {
    let now = Instant::now();
    let mut protocol = logged_on(now);
    let answered = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    let waiting = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    let input = [
        from_steam(&response(answered.job_id(), EResult::OK, "answered")),
        from_steam(&logged_off(EResult::LogonSessionReplaced)),
    ]
    .concat();
    protocol.handle_input(&input, now).unwrap();

    assert_eq!(
        protocol.take_response(&answered).unwrap().unwrap().publickey_mod(),
        "answered"
    );
    assert!(matches!(
        protocol.take_response(&waiting),
        Some(Err(ClientError::LoggedOff(EResult::LogonSessionReplaced)))
    ));
}

#[test]
fn a_closed_connection_ends_everything() {
    let now = Instant::now();
    let mut protocol = logged_on(now);
    let call = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    assert!(protocol.poll_timeout().is_some());

    protocol.handle_closed();
    assert!(matches!(protocol.take_response(&call), Some(Err(ClientError::Dropped))));
    assert!(protocol.poll_timeout().is_none());
    assert!(matches!(protocol.logged_off(), Some(Err(ClientError::Dropped))));
    // what wasn't written yet is gone, and nothing more is sent
    assert!(protocol.poll_transmit().is_none());
    let later = protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    protocol.log_off();
    protocol.log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new());
    assert!(protocol.poll_transmit().is_none());
    assert!(matches!(
        protocol.take_response(&later),
        Some(Err(ClientError::Dropped))
    ));
    assert!(matches!(protocol.logon_result(), Some(Err(ClientError::Dropped))));
    assert!(matches!(
        protocol.handle_input(&from_steam(&hello(1)), now),
        Err(ClientError::Dropped)
    ));
}

#[test]
fn closing_after_a_log_off_keeps_why() {
    let now = Instant::now();
    let mut protocol = logged_on(now);
    protocol
        .handle_input(&from_steam(&logged_off(EResult::OK)), now)
        .unwrap();
    protocol.handle_closed();
    assert!(matches!(protocol.logged_off(), Some(Ok(EResult::OK))));
}

#[test]
fn reads_into_its_own_buffer() {
    let mut protocol = CmProtocol::encrypted(key());
    let input = from_steam(&hello(7));
    let buffer = protocol.receive_buffer();
    assert!(buffer.capacity() >= MIN_READ);
    buffer.extend_from_slice(&input);
    protocol.handle_received(Instant::now()).unwrap();
    assert_eq!(hellos(&mut protocol), [7]);
}

#[test]
fn a_big_frames_buffer_goes_once_its_messages_do() {
    let now = Instant::now();
    let mut protocol = CmProtocol::encrypted(key());
    // a hello with a long protocol buffer field Steam doesn't define, as a big message
    let mut big = hello(1);
    let padding = 2 * MAX_KEPT_FRAME;
    assert_eq!(padding % (1 << 14), 0, "its length is a 3-byte varint");
    big.extend_from_slice(&[0xFA, 0x3F]);
    big.extend_from_slice(&[0x80, 0x80, (padding >> 14) as u8]);
    big.resize(big.len() + padding, 0);
    let big = from_steam(&big);

    // split across reads, as from the socket
    let (first, rest) = big.split_at(big.len() / 2);
    protocol.handle_input(first, now).unwrap();
    assert!(protocol.received.capacity() > RECEIVE_BUFFER_CAPACITY);
    protocol.handle_input(rest, now).unwrap();
    assert_eq!(protocol.received.capacity(), RECEIVE_BUFFER_CAPACITY);
    assert!(!protocol.received_grew);
    assert_eq!(hellos(&mut protocol), [1]);

    // a big frame with the start of the next one behind it keeps the buffer until that one is whole
    let next = from_steam(&hello(2));
    let input = [&big[..], &next[..4]].concat();
    protocol.handle_input(&input, now).unwrap();
    assert!(protocol.received_grew);
    protocol.handle_input(&next[4..], now).unwrap();
    assert!(!protocol.received_grew);
    assert_eq!(protocol.received.capacity(), RECEIVE_BUFFER_CAPACITY);
    assert_eq!(hellos(&mut protocol), [1, 2]);
}

#[test]
fn a_buffer_that_grew_for_a_frame_up_to_the_limit_is_kept() {
    let now = Instant::now();
    let mut protocol = CmProtocol::encrypted(key());
    // a hello with a long field Steam doesn't define, as in `a_big_frames_buffer_goes_once_its_messages_do`, that
    // encrypts to a frame of exactly the limit: 16 bytes of IV, then the message padded to the next 16
    let mut big = hello(1);
    let padding = MAX_KEPT_FRAME - 16 - 16 - big.len() - 5;
    big.extend_from_slice(&[0xFA, 0x3F]);
    big.extend_from_slice(&[
        0x80 | (padding & 0x7F) as u8,
        0x80 | (padding >> 7 & 0x7F) as u8,
        (padding >> 14) as u8,
    ]);
    big.resize(big.len() + padding, 0);
    let big = from_steam(&big);
    assert_eq!(big.len() - 8, MAX_KEPT_FRAME);

    // reused for the next frames: `tests/zero_alloc.rs` counts that reading them allocates nothing
    for chunk in big.chunks(MIN_READ) {
        protocol.handle_input(chunk, now).unwrap();
    }
    assert!(!protocol.received_grew);
    assert_eq!(protocol.poll_event().map(|event| version(&event)), Some(Some(1)));
}

#[test]
fn the_receive_buffer_has_room_for_a_read() {
    let mut protocol = CmProtocol::encrypted(key());
    // what is left of the buffer is less than a read
    let buffer = protocol.receive_buffer();
    let filled = buffer.capacity() - 100;
    buffer.resize(filled, 0);
    let buffer = protocol.receive_buffer();
    assert!(buffer.capacity() - buffer.len() >= MIN_READ);
}

#[test]
fn starts_with_empty_queues() {
    let protocol = CmProtocol::encrypted(key());
    assert_eq!(protocol.events.capacity(), 0);
    assert_eq!(protocol.unpacked.capacity(), 0);
    assert_eq!(protocol.outgoing.capacity(), 0);
}

#[test]
fn transmits_everything_at_once_over_tcp() {
    let now = Instant::now();
    let mut protocol = logged_on(now);
    protocol.call::<GetPasswordRSAPublicKey>(rsa_key_request());
    protocol.handle_timeout(now + Duration::from_secs(9));
    let transmit = protocol.poll_transmit().unwrap();
    assert!(protocol.poll_transmit().is_none());

    let mut src = BytesMut::from(&transmit[..]);
    let mut emsgs = vec![];
    while let Some(frame) = framing::decode(&mut src).unwrap() {
        emsgs.push(
            PacketMessage::from_bytes(framing::decrypt(frame, &key()).unwrap())
                .unwrap()
                .emsg(),
        );
    }
    assert_eq!(emsgs, [EMsg::ServiceMethodCallFromClient, EMsg::ClientHeartBeat]);
}

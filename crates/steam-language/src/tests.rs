//! Parsing and writing whole packets, with captured and hand-built bytes.

use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;

use crate::emsg::EMsg;
use crate::enums::EUniverse;
use crate::headers::ExtendedMessageHeader;
use crate::headers::StandardMessageHeader;
use crate::messages::MsgChannelEncryptRequest;
use crate::messages::MsgClientChatEnter;
use crate::HasJobId;
use crate::MessageBodyExt;
use crate::MessageError;
use crate::MessageHeaderExt;
use crate::MessageHeaderWrapper;
use crate::SerializableBytes;

/// ChannelEncryptRequest captured from a CM: standard header, body, then a 16 byte random challenge.
const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
    23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0, 0,
    0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
];

/// ClientChatEnter captured from a CM: extended header, body, then the chat name and members.
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

/// Hand-built ClientLogon with a protobuf header.
const PROTO_CLIENT_LOGON: [u8; 31] = [
    0x8A, 0x15, 0x00, 0x80, // EMsg::ClientLogon, flagged as protobuf
    0x15, 0x00, 0x00, 0x00, // header length, 21
    0x09, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x10, 0x01, // steamid
    0x10, 0xD2, 0x09, // client_sessionid
    0x51, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // jobid_source
    0x08, 0x01, // body
];

fn proto_client_logon_header() -> CMsgProtoBufHeader {
    let mut header = CMsgProtoBufHeader::new();
    header.set_steamid(76561197960265728);
    header.set_client_sessionid(1234);
    header.set_jobid_source(42);
    header
}

#[test]
fn parses_a_protobuf_packet() {
    let (emsg, header, body) = MessageHeaderWrapper::parse(&PROTO_CLIENT_LOGON).unwrap();

    assert_eq!(emsg, EMsg::ClientLogon);
    assert_eq!(header, MessageHeaderWrapper::Proto(proto_client_logon_header()));
    assert_eq!(header.source(), 42);
    assert_eq!(header.target(), u64::MAX);
    assert_eq!(body, [0x08, 0x01]);
}

#[test]
fn writes_a_protobuf_packet() {
    let mut packet = vec![];
    MessageHeaderWrapper::Proto(proto_client_logon_header()).write(EMsg::ClientLogon, &mut packet);
    packet.extend_from_slice(&[0x08, 0x01]);

    assert_eq!(packet, PROTO_CLIENT_LOGON);
}

#[test]
fn round_trips_every_header_kind() {
    let mut extended = ExtendedMessageHeader::new();
    extended.set_target(7);
    let headers = [
        (
            EMsg::ChannelEncryptResponse,
            MessageHeaderWrapper::Std(StandardMessageHeader::new()),
        ),
        (EMsg::ClientChatEnter, MessageHeaderWrapper::Ext(extended)),
        (
            EMsg::ClientLogon,
            MessageHeaderWrapper::Proto(proto_client_logon_header()),
        ),
        (
            EMsg::ClientLogon,
            MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()),
        ),
    ];

    for (emsg, header) in headers {
        let mut packet = vec![];
        header.write(emsg, &mut packet);
        packet.extend_from_slice(b"body");

        let (parsed_emsg, parsed_header, body) = MessageHeaderWrapper::parse(&packet).unwrap();
        assert_eq!(parsed_emsg, emsg);
        assert_eq!(parsed_header, header);
        assert_eq!(body, b"body");
    }
}

#[test]
fn extended_header_size_includes_the_emsg() {
    let header = ExtendedMessageHeader::new().to_bytes();

    assert_eq!(header.len() + 4, 36);
    assert_eq!(header[0], 36);
}

#[test]
fn parses_a_captured_channel_encrypt_request() {
    let (emsg, header, body) = MessageHeaderWrapper::parse(&CHANNEL_ENCRYPT_REQUEST).unwrap();
    assert_eq!(emsg, EMsg::ChannelEncryptRequest);
    assert_eq!(header, MessageHeaderWrapper::Std(StandardMessageHeader::new()));

    let (message, challenge) = MsgChannelEncryptRequest::parse(body).unwrap();
    assert_eq!(
        message,
        MsgChannelEncryptRequest {
            protocol_version: 1,
            universe: EUniverse::Public,
        }
    );
    assert_eq!(challenge, &CHANNEL_ENCRYPT_REQUEST[28..]);
}

#[test]
fn parses_a_captured_client_chat_enter() {
    let (emsg, header, body) = MessageHeaderWrapper::parse(&CLIENT_CHAT_ENTER).unwrap();
    assert_eq!(emsg, EMsg::ClientChatEnter);
    let MessageHeaderWrapper::Ext(header) = header else {
        panic!("expected an extended header, got {:?}", header);
    };
    assert_eq!(header.to_bytes(), &CLIENT_CHAT_ENTER[4..36]);
    assert_eq!((header.target(), header.source()), (u64::MAX, u64::MAX));

    let (message, payload) = MsgClientChatEnter::parse(body).unwrap();
    assert_eq!(message.to_bytes(), &body[..body.len() - payload.len()]);
    assert!(payload.starts_with(b"Saxton Hell\0"));
}

#[test]
fn short_packets_are_errors() {
    // every packet cut before the end of its header and body
    let packets: [(&[u8], usize); 3] = [
        (&CHANNEL_ENCRYPT_REQUEST, 4 + 16 + 8),
        (&CLIENT_CHAT_ENTER, 4 + 32 + 45),
        (&PROTO_CLIENT_LOGON, 4 + 4 + 21),
    ];

    for (packet, header_and_body) in packets {
        for len in 0..header_and_body {
            let parsed = MessageHeaderWrapper::parse(&packet[..len]).and_then(|(emsg, _, body)| match emsg {
                EMsg::ChannelEncryptRequest => MsgChannelEncryptRequest::parse(body).map(drop),
                EMsg::ClientChatEnter => MsgClientChatEnter::parse(body).map(drop),
                // the protobuf body is opaque here, so only its header can be short
                _ => Ok(()),
            });
            assert!(parsed.is_err(), "{:?} cut to {len} bytes parsed", &packet[..4]);
        }
    }
}

#[test]
fn protobuf_header_longer_than_the_packet_is_an_error() {
    let mut packet = PROTO_CLIENT_LOGON;
    packet[4] = 0xFF;

    assert!(matches!(
        MessageHeaderWrapper::parse(&packet),
        Err(MessageError::TooShort { needed: 259, got: 27 })
    ));
}

#[test]
fn malformed_protobuf_header_is_an_error() {
    let packet = [0x8A, 0x15, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0xFF];

    assert!(matches!(
        CMsgProtoBufHeader::parse(&packet[4..]),
        Err(MessageError::Protobuf(_))
    ));
    assert!(MessageHeaderWrapper::parse(&packet).is_err());
}

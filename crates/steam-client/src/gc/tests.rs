use std::time::Duration;

use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::*;
use crate::protocol::proto_message;

const CS2: u32 = 730;

/// A protobuf GC payload: the type, flagged, the header's length, the header, then the body.
pub(crate) fn proto_payload(msg_type: u32, header: &CMsgProtoBufHeader, body: &[u8]) -> Vec<u8> {
    let header = header.write_to_bytes().unwrap();
    let mut payload = (msg_type | PROTO_MASK).to_le_bytes().to_vec();
    payload.extend_from_slice(&(header.len() as u32).to_le_bytes());
    payload.extend_from_slice(&header);
    payload.extend_from_slice(body);
    payload
}

/// A GC payload that isn't protobuf: version 1, no target job, source job 7, then the body.
fn plain_payload(body: &[u8]) -> Vec<u8> {
    let mut payload = 1u16.to_le_bytes().to_vec();
    payload.extend_from_slice(&u64::MAX.to_le_bytes());
    payload.extend_from_slice(&7u64.to_le_bytes());
    payload.extend_from_slice(body);
    payload
}

/// A `ClientFromGC` of app `app_id`, as Steam sends it.
pub(crate) fn from_gc(app_id: u32, msg_type: u32, payload: Vec<u8>) -> Vec<u8> {
    let mut envelope = CMsgGCClient::new();
    envelope.set_appid(app_id);
    envelope.set_msgtype(msg_type);
    envelope.set_payload(payload.into());
    proto_message(EMsg::ClientFromGC, CMsgProtoBufHeader::new(), envelope)
}

pub(crate) fn raw(packet: Vec<u8>) -> RawMessage {
    RawMessage::new(EMsg::ClientFromGC, packet.into())
}

/// A message of type `msg_type` from CS2's GC, its body `version` as a `CMsgClientHello`.
fn message(msg_type: u32, version: u32) -> GcMessage {
    let mut hello = CMsgClientHello::new();
    hello.set_protocol_version(version);
    let payload = proto_payload(msg_type, &CMsgProtoBufHeader::new(), &hello.write_to_bytes().unwrap());
    raw(from_gc(CS2, msg_type | PROTO_MASK, payload))
        .gc_message()
        .unwrap()
        .unwrap()
}

fn version(event: &GcEvent) -> u32 {
    match event {
        GcEvent::Message(message) => message.parse::<CMsgClientHello>().unwrap().protocol_version(),
        event => panic!("expected a message, got {:?}", event),
    }
}

/// A channel and the task's end of it, with nowhere to send to.
fn open() -> (GcChannel, Route) {
    let (commands, _) = mpsc::unbounded_channel();
    channel(CS2, commands.downgrade())
}

async fn recv(channel: &mut GcChannel) -> Option<GcEvent> {
    timeout(Duration::from_secs(1), channel.recv()).await.expect("an event")
}

#[test]
fn a_proto_body_follows_its_header() {
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(42);
    let payload = Bytes::from(proto_payload(9157, &header, b"body"));
    let body = gc_body(9157 | PROTO_MASK, payload.clone()).unwrap();
    assert_eq!(&body[..], b"body");
    // a slice of the payload, not a copy
    assert_eq!(body.as_ptr(), payload[payload.len() - 4..].as_ptr());

    // an empty header, and an empty body
    let payload = proto_payload(4004, &CMsgProtoBufHeader::new(), b"");
    assert_eq!(payload.len(), 8);
    assert!(gc_body(4004 | PROTO_MASK, payload.into()).unwrap().is_empty());
}

#[test]
fn a_body_that_isnt_proto_follows_an_18_byte_header() {
    let body = gc_body(1003, plain_payload(b"raw").into()).unwrap();
    assert_eq!(&body[..], b"raw");
    assert!(gc_body(1003, plain_payload(b"").into()).unwrap().is_empty());
}

#[test]
fn truncated_payloads_are_errors() {
    let too_short = |msg_type: u32, payload: Vec<u8>| match gc_body(msg_type, payload.into()) {
        Err(MessageError::TooShort { needed, got }) => (needed, got),
        result => panic!("expected TooShort, got {:?}", result),
    };
    let proto = PROTO_MASK | 9157;
    assert_eq!(too_short(proto, vec![]), (8, 0));
    assert_eq!(too_short(proto, vec![0xC5, 0x23, 0, 0x80, 0, 0, 0]), (8, 7));
    // a header longer than what follows
    let mut header = CMsgProtoBufHeader::new();
    header.set_jobid_target(42);
    let mut payload = proto_payload(9157, &header, b"");
    let length = payload.len();
    payload.pop();
    assert_eq!(too_short(proto, payload), (length, length - 1));
    // a length past anything: negative as SteamKit reads it, or the largest
    let mut payload = vec![0xC5, 0x23, 0, 0x80];
    payload.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(too_short(proto, payload).1, 8);

    assert_eq!(too_short(1003, vec![]), (18, 0));
    let mut payload = plain_payload(b"");
    payload.pop();
    assert_eq!(too_short(1003, payload), (18, 17));
}

#[test]
fn reads_a_message_from_a_game_coordinator() {
    let message = message(4004, 3);
    assert_eq!(message.app_id(), CS2);
    assert_eq!(message.msg_type(), 4004);
    assert!(message.is_proto());
    assert_eq!(message.parse::<CMsgClientHello>().unwrap().protocol_version(), 3);
    assert_eq!(
        format!("{:?}", message),
        "GcMessage { app_id: 730, msg_type: 4004, is_proto: true, len: 2 }"
    );

    let plain = raw(from_gc(CS2, 1003, plain_payload(b"raw")))
        .gc_message()
        .unwrap()
        .unwrap();
    assert_eq!(plain.msg_type(), 1003);
    assert!(!plain.is_proto());
    assert_eq!(&plain.body()[..], b"raw");
}

#[test]
fn only_messages_from_game_coordinators_are_read_as_theirs() {
    let hello = proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), CMsgClientHello::new());
    assert!(RawMessage::new(EMsg::ClientHello, hello.into()).gc_message().is_none());

    // an envelope that can't be parsed, or a truncated payload
    let mut garbage = from_gc(CS2, 4004 | PROTO_MASK, vec![]);
    garbage.extend_from_slice(&[0xFF, 0xFF]);
    assert!(matches!(raw(garbage).gc_message(), Some(Err(ClientError::Message(_)))));
    let truncated = raw(from_gc(CS2, 4004 | PROTO_MASK, vec![1, 2, 3]));
    assert!(matches!(
        truncated.gc_message(),
        Some(Err(ClientError::Message(MessageError::TooShort { .. })))
    ));
}

#[test]
fn sends_with_an_empty_gc_header() {
    let mut hello = CMsgClientHello::new();
    hello.set_protocol_version(5);
    let envelope = client_to_gc(CS2, 4006, &hello).unwrap();
    assert_eq!(envelope.appid(), CS2);
    assert_eq!(envelope.msgtype(), 4006 | PROTO_MASK);
    let payload = envelope.payload.clone().unwrap();
    assert_eq!(&payload[..8], &[0xA6, 0x0F, 0x00, 0x80, 0, 0, 0, 0]);
    // what a GC reads back
    let body = gc_body(envelope.msgtype(), payload).unwrap();
    assert_eq!(CMsgClientHello::parse_from_tokio_bytes(&body).unwrap(), hello);
    // a type already flagged stays the same
    assert_eq!(client_to_gc(CS2, 4006 | PROTO_MASK, &hello).unwrap(), envelope);
}

#[tokio::test]
async fn messages_past_the_capacity_are_counted_where_they_were_dropped() {
    let (mut channel, route) = open();
    let sent = GC_CAPACITY as u32 + 10;
    for version in 0..sent {
        assert!(route.queue.push(message(4004, version)));
    }
    for version in 0..GC_CAPACITY as u32 {
        assert_eq!(version_of(recv(&mut channel).await), version);
    }
    // read once the queue is empty: the drops came after every message kept
    assert!(matches!(recv(&mut channel).await, Some(GcEvent::Lagged(10))));

    // a new drop is counted before the next message
    for version in 0..=GC_CAPACITY as u32 {
        route.queue.push(message(4004, version));
    }
    for _ in 0..GC_CAPACITY {
        recv(&mut channel).await;
    }
    route.queue.push(message(4004, 999));
    assert!(matches!(recv(&mut channel).await, Some(GcEvent::Lagged(1))));
    assert_eq!(version_of(recv(&mut channel).await), 999);
}

fn version_of(event: Option<GcEvent>) -> u32 {
    version(&event.expect("an event"))
}

#[tokio::test]
async fn a_lost_session_is_never_dropped_and_said_once() {
    let (mut channel, route) = open();
    for version in 0..GC_CAPACITY as u32 + 3 {
        route.queue.push(message(4004, version));
    }
    assert!(route.queue.session_lost());
    // lost again with nothing in between: once is enough
    assert!(route.queue.session_lost());
    for _ in 0..GC_CAPACITY {
        recv(&mut channel).await;
    }
    assert!(matches!(recv(&mut channel).await, Some(GcEvent::Lagged(3))));
    assert!(matches!(recv(&mut channel).await, Some(GcEvent::SessionLost)));

    // a session with a message in it is another
    route.queue.push(message(4004, 1));
    route.queue.session_lost();
    assert_eq!(version_of(recv(&mut channel).await), 1);
    assert!(matches!(recv(&mut channel).await, Some(GcEvent::SessionLost)));
    assert!(channel.queue.lock().events.is_empty());
}

#[tokio::test]
async fn the_channel_ends_once_the_task_lets_go_of_it() {
    let (mut channel, route) = open();
    route.queue.push(message(4004, 1));
    drop(route);
    // what is left is read first
    assert_eq!(version_of(recv(&mut channel).await), 1);
    assert!(recv(&mut channel).await.is_none());
    assert!(matches!(
        channel.send(4006, &CMsgClientHello::new()),
        Err(ClientError::Closed)
    ));
}

#[tokio::test]
async fn recv_waits_for_the_next_event_and_is_cancel_safe() {
    let (mut channel, route) = open();
    assert!(timeout(Duration::from_millis(10), channel.recv()).await.is_err());
    let push = async {
        tokio::task::yield_now().await;
        route.queue.push(message(4004, 2));
    };
    let (event, ()) = tokio::join!(recv(&mut channel), push);
    assert_eq!(version_of(event), 2);
}

#[test]
fn a_dropped_channel_takes_nothing() {
    let (channel, route) = open();
    drop(channel);
    assert!(!route.queue.push(message(4004, 1)));
    assert!(!route.queue.session_lost());
}

#[test]
fn routes_by_app_and_gives_back_what_it_doesnt_route() {
    let mut routes = GcRoutes::default();
    let packet = || {
        raw(from_gc(
            CS2,
            4004 | PROTO_MASK,
            proto_payload(4004, &CMsgProtoBufHeader::new(), b""),
        ))
    };
    // no route at all
    assert!(routes.route(packet()).is_some());

    let (channel, route) = open();
    routes.add(route);
    assert!(routes.route(packet()).is_none());
    assert_eq!(channel.queue.lock().messages, 1);
    // another app, another kind of message, a message that can't be read
    assert!(routes
        .route(raw(from_gc(
            570,
            4004 | PROTO_MASK,
            proto_payload(4004, &CMsgProtoBufHeader::new(), b"")
        )))
        .is_some());
    let hello = proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), CMsgClientHello::new());
    assert!(routes.route(RawMessage::new(EMsg::ClientHello, hello.into())).is_some());
    assert!(routes.route(raw(from_gc(CS2, 4004 | PROTO_MASK, vec![1]))).is_some());
    assert_eq!(channel.queue.lock().messages, 1);
    // which the channel misses, as one dropped: it may have been its own; so does one whose envelope can't be read
    assert_eq!(channel.queue.lock().dropped, 1);
    let mut garbage = from_gc(CS2, 4004 | PROTO_MASK, vec![]);
    garbage.extend_from_slice(&[0xFF, 0xFF]);
    assert!(routes.route(raw(garbage)).is_some());
    let mut channel = channel;
    assert!(matches!(channel.try_recv(), Ok(GcEvent::Message(_))));
    assert!(matches!(channel.try_recv(), Ok(GcEvent::Lagged(2))));

    // once dropped, its route goes
    drop(channel);
    assert!(routes.route(packet()).is_some());
    assert!(routes.0.is_empty());
}

#[tokio::test]
async fn a_new_channel_for_an_app_replaces_the_old_one() {
    let mut routes = GcRoutes::default();
    let (mut old, route) = open();
    routes.add(route);
    let (new, route) = open();
    routes.add(route);
    assert!(recv(&mut old).await.is_none());
    let packet = raw(from_gc(
        CS2,
        4004 | PROTO_MASK,
        proto_payload(4004, &CMsgProtoBufHeader::new(), b""),
    ));
    assert!(routes.route(packet).is_none());
    assert_eq!(new.queue.lock().messages, 1);
}

#[test]
fn sending_needs_the_task() {
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (channel, _route) = channel(CS2, commands.downgrade());
    channel.send(4006, &CMsgClientHello::new()).unwrap();
    match receiver.try_recv() {
        Ok(Command::ToGc(message)) => assert_eq!(message.appid(), CS2),
        _ => panic!("expected a message to the GC"),
    }
    // the channel doesn't keep the client running
    drop(commands);
    assert!(matches!(
        channel.send(4006, &CMsgClientHello::new()),
        Err(ClientError::Closed)
    ));
    assert_eq!(channel.app_id(), CS2);
    assert_eq!(format!("{:?}", channel), "GcChannel { app_id: 730, waiting: 0 }");
}

#[test]
fn try_recv_doesnt_wait() {
    let (mut channel, route) = open();
    assert!(matches!(channel.try_recv(), Err(TryRecvError::Empty)));
    route.queue.push(message(4004, 1));
    route.queue.session_lost();
    assert_eq!(version(&channel.try_recv().unwrap()), 1);
    assert!(matches!(channel.try_recv(), Ok(GcEvent::SessionLost)));
    assert!(matches!(channel.try_recv(), Err(TryRecvError::Empty)));
    // what is left is read first
    route.queue.push(message(4004, 2));
    drop(route);
    assert_eq!(version(&channel.try_recv().unwrap()), 2);
    assert!(matches!(channel.try_recv(), Err(TryRecvError::Stopped)));
}

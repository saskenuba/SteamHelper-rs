//! Heap per account: what a client keeps while idle, and after a big message.
//!
//! A global allocator counts the bytes allocated and not freed yet, by the test's thread, so this is a test binary of
//! its own. Run with `--nocapture` to see the numbers; the bounds are regression guards, set from the measured values
//! plus a margin.
//!
//! The accounts talk to fake CMs over in-memory pipes, set up before measuring. What the pipes buffer is counted with
//! the client (a socket's buffers would be in the kernel), so the numbers are a little high.

#![allow(unsafe_code)]

use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::alloc::System;
use std::cell::Cell;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use steam_client::__private::LoggedOnConnection;
use steam_client::__private::Protocol;
use steam_client::events::Event;
use steam_cryptography::symmetric_encrypt_hmac_iv;
use steam_cryptography::SessionKey;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::MessageHeaderWrapper;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
use tokio::io::duplex;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

thread_local! {
    /// Bytes this thread allocated and didn't free. Each test runs on a thread of its own, with a runtime on that
    /// thread, so the tests don't count each other's.
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn add(bytes: isize) {
    // fails while the thread is torn down, when nothing is measured anyway
    let _ = LIVE.try_with(|live| live.set(live.get() + bytes));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(layout.size() as isize);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        add(layout.size() as isize);
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        add(new_size as isize - layout.size() as isize);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        add(-(layout.size() as isize));
        System.dealloc(ptr, layout)
    }
}

fn live() -> isize {
    LIVE.with(Cell::get)
}

fn key() -> SessionKey {
    SessionKey::from([3; 32])
}

fn encrypted(message: &[u8]) -> Vec<u8> {
    let encrypted = symmetric_encrypt_hmac_iv(message, &key());
    let mut frame = (encrypted.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(b"VT01");
    frame.extend_from_slice(&encrypted);
    frame
}

fn proto_packet<M: Message>(emsg: EMsg, body: &M) -> Vec<u8> {
    let mut packet = vec![];
    MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()).write(emsg, &mut packet);
    packet.extend(body.write_to_bytes().unwrap());
    packet
}

/// A raw `Multi` of `count` messages of about 1 KB: a frame of `count` KB, as after an account logon.
fn big_multi(count: usize) -> Vec<u8> {
    let mut hello = CMsgClientHello::new();
    hello.set_protocol_version(1);
    let mut packet = proto_packet(EMsg::ClientHello, &hello);
    packet.resize(1024, 0);

    let mut body = vec![];
    for _ in 0..count {
        body.extend_from_slice(&(packet.len() as u32).to_le_bytes());
        body.extend_from_slice(&packet);
    }
    let mut multi = CMsgMulti::new();
    multi.set_message_body(body.into());
    proto_packet(EMsg::Multi, &multi)
}

/// Anonymous ClientLogOnResponse captured from a CM. Also in `zero_alloc.rs`, and in `protocol/tests.rs`, which
/// documents it.
const LOGON_RESPONSE: [u8; 99] = [
    0xEF, 0x02, 0x00, 0x80, 0x14, 0x00, 0x00, 0x00, 0x09, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0x10, 0xE3,
    0x82, 0xD5, 0xBD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x08, 0x01, 0x10, 0x09, 0x18, 0x09, 0x20, 0x87, 0xE2, 0x81,
    0xD8, 0x0C, 0x2D, 0x7B, 0xFB, 0xD2, 0x6A, 0x30, 0x00, 0x38, 0x19, 0x60, 0xFF, 0xFF, 0xFF, 0xFF, 0x07, 0x72, 0x00,
    0x7A, 0x05, 0x0D, 0x07, 0x71, 0x00, 0xCB, 0xA1, 0x01, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0xAA, 0x01,
    0x02, 0x55, 0x53, 0xC0, 0x01, 0x00, 0xC8, 0x01, 0x00, 0xD8, 0x01, 0xC4, 0xD2, 0xC7, 0xD9, 0xD1, 0xF0, 0xA3, 0x8B,
    0x05, 0xE0, 0x01, 0x00,
];

/// How many accounts are measured together.
const ACCOUNTS: usize = 100;

/// The CM end of an account: answers its logon with `logon_response`, sends `then`, and reads what comes until the
/// client is gone, into a buffer on the stack, so that it allocates nothing more.
/// The messages are shared, so that the CMs free nothing while measuring.
async fn fake_cm(mut stream: DuplexStream, logon_response: Bytes, then: Bytes) {
    let mut buffer = [0; 1024];
    if !matches!(stream.read(&mut buffer).await, Ok(read) if read > 0) {
        return;
    }
    let _ = stream.write_all(&logon_response).await;
    let _ = stream.write_all(&then).await;
    while matches!(stream.read(&mut buffer).await, Ok(read) if read > 0) {}
}

/// Starts `count` fake CMs, which send `then` after the logon. Returns the client ends.
///
/// The pipes buffer at most 4 KB, so that what they keep stays small next to what the client does.
async fn fake_cms(count: usize, then: &Bytes) -> Vec<DuplexStream> {
    let logon_response = Bytes::from(encrypted(&LOGON_RESPONSE));
    let clients = (0..count)
        .map(|_| {
            let (client, server) = duplex(4 * 1024);
            tokio::spawn(fake_cm(server, logon_response.clone(), then.clone()));
            client
        })
        .collect();
    // until they wait for the logon
    tokio::task::yield_now().await;
    clients
}

/// Fails the test if `future` takes longer than any wait here, in paused time, instead of hanging it.
async fn soon<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(600), future)
        .await
        .expect("still waiting")
}

/// What idle accounts take each, once logged on and past a few heartbeats, as `per account` bytes.
async fn idle_accounts<A>(log_on: impl Fn(LoggedOnConnection<DuplexStream>) -> A) -> (Vec<A>, isize) {
    let clients = fake_cms(ACCOUNTS, &Bytes::new()).await;
    let mut accounts = Vec::with_capacity(ACCOUNTS);
    let before = live();
    for client in clients {
        accounts.push(log_on(soon(LoggedOnConnection::log_on(client, key())).await.unwrap()));
    }
    tokio::time::sleep(Duration::from_secs(60)).await;
    let per_account = (live() - before) / ACCOUNTS as isize;
    (accounts, per_account)
}

#[tokio::test(start_paused = true)]
async fn an_idle_spawned_account_takes_a_few_kb() {
    let (accounts, per_account) = idle_accounts(LoggedOnConnection::spawn).await;
    println!("spawned and idle: {} bytes per account", per_account);
    assert!(per_account < 16 * 1024, "{} bytes per account", per_account);
    drop(accounts);
}

#[tokio::test(start_paused = true)]
async fn an_idle_account_without_a_task_takes_less() {
    let (accounts, per_account) = idle_accounts(|account| account).await;
    println!("without a task, idle: {} bytes per account", per_account);
    assert!(per_account < 12 * 1024, "{} bytes per account", per_account);
    drop(accounts);
}

#[tokio::test(start_paused = true)]
async fn a_spawned_account_gives_back_a_big_messages_memory() {
    let big = Bytes::from(encrypted(&big_multi(300)));
    let mut clients = fake_cms(1, &big).await;
    let before = live();

    let (handle, mut events, _task) = soon(LoggedOnConnection::log_on(clients.remove(0), key()))
        .await
        .unwrap()
        .spawn();
    // past the event queue's capacity, and none lost: the messages of one read are all kept
    let mut kept = vec![];
    while kept.len() < 300 {
        match soon(events.recv()).await.unwrap() {
            Event::Message(message) => kept.push(message),
            event => panic!("expected the Multi's messages, got {:?}", event),
        }
    }
    let holding = live() - before;
    drop(kept);
    tokio::time::sleep(Duration::from_secs(60)).await;
    let after = live() - before;
    println!(
        "a {} KB frame: {} bytes while its messages are kept, {} bytes once they're gone",
        big.len() / 1024,
        holding,
        after
    );
    assert!(holding > big.len() as isize);
    // an idle account, and the little room its queues keep
    assert!(after < 32 * 1024, "{} bytes kept", after);
    drop((handle, events));
}

#[test]
fn a_big_message_doesnt_stay_in_the_buffer() {
    let input = encrypted(&big_multi(300));
    let mut protocol = Protocol::encrypted(key());
    protocol
        .handle_input(
            &encrypted(&proto_packet(EMsg::ClientHello, &CMsgClientHello::new())),
            Instant::now(),
        )
        .unwrap();
    while protocol.poll_message().is_some() {}
    let before = live();

    protocol.handle_input(&input, Instant::now()).unwrap();
    while protocol.poll_message().is_some() {}
    let after = live() - before;
    println!(
        "protocol, kept after a {} KB frame: {} bytes",
        input.len() / 1024,
        after
    );
    assert!(after <= 0, "{} bytes kept", after);
}

#[test]
fn measures_a_protocol() {
    let before = live();
    let protocol = Protocol::encrypted(key());
    println!("a protocol: {} bytes", live() - before);
    drop(protocol);
}

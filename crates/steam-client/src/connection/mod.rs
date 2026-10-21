//! The connection to a CM server: the tokio driver of [`CmProtocol`], and the only IO in the client.
//!
//! One owner reads the socket into the protocol, writes what it gives, and wakes it for its heartbeat. The connection
//! only does so while one of its methods is awaited: a [spawned client](crate::handle) awaits them all the time; a
//! client used without one must keep calling, or keep reading [events](crate::client::SteamClient::next_event).

use std::fmt;
use std::future::poll_fn;
use std::future::Future;
use std::io;
use std::pin::pin;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use bytes::Buf;
use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
use steamid_parser::SteamID;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio::time::Instant;

use crate::errors::ClientError;
use crate::events::Event;
use crate::job::Job;
use crate::job::JobMessage;
use crate::protocol::CmProtocol;
use crate::protocol::Exchange;
use crate::protocol::LogonInfo;
use crate::protocol::Transport;
use crate::service::Service;
use crate::service::ServiceMethod;

/// How long Steam has to answer a request.
pub(crate) const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a TCP connection is quiet before the OS checks that Steam is still there, and then how often. Only a quiet
/// connection is checked: one the client keeps writing to (heartbeats every 9 s or so once logged on) fails by TCP's
/// retransmission timeout instead, in minutes.
const KEEPALIVE_TIME: Duration = Duration::from_secs(30);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// A connection to a CM server, over a stream such as a [`TcpStream`].
///
/// Its methods are cancel-safe, as [`Connection::step`] says.
pub(crate) struct Connection<T = TcpStream> {
    stream: T,
    protocol: CmProtocol,
    /// What the protocol gave to write, and isn't written yet. Writing resumes from here if a write was cancelled.
    writing: Bytes,
}

impl Connection<TcpStream> {
    /// Connects over TCP to `endpoint` (`ip:port`), and encrypts the channel.
    pub(crate) async fn connect(endpoint: &str) -> Result<Self, ClientError> {
        debug!("Connecting to {}", endpoint);
        let stream = TcpStream::connect(endpoint).await?;
        keep_alive(&stream)?;
        Self::open(stream).await
    }
}

impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub(crate) fn new(stream: T, protocol: CmProtocol) -> Self {
        Self {
            stream,
            protocol,
            writing: Bytes::new(),
        }
    }

    /// Runs the channel encryption handshake on a stream that was just opened.
    pub(crate) async fn open(stream: T) -> Result<Self, ClientError> {
        let mut connection = Self::new(stream, CmProtocol::new(Transport::Tcp));
        connection
            .wait(|protocol| protocol.is_connected().then_some(()))
            .await?;
        Ok(connection)
    }

    /// The protocol, to make requests and take what came, between steps.
    pub(crate) fn protocol(&mut self) -> &mut CmProtocol {
        &mut self.protocol
    }

    /// How many bytes wait to be written: what the protocol gave, and what it holds.
    pub(crate) fn unsent(&self) -> usize {
        self.writing.len() + self.protocol.unsent()
    }

    /// Sends a message, with the session on its header once logged on.
    pub(crate) async fn send<M: Message>(&mut self, emsg: EMsg, body: M) -> Result<(), ClientError> {
        self.protocol.send(emsg, body);
        self.flush().await
    }

    /// Sends a message routed to app `app_id`, with the session on its header once logged on.
    pub(crate) async fn send_routed<M: Message>(
        &mut self,
        emsg: EMsg,
        app_id: u32,
        body: M,
    ) -> Result<(), ClientError> {
        self.protocol.send_routed(emsg, app_id, body);
        self.flush().await
    }

    /// Calls a service method and waits for its response, at most [`RESPONSE_TIMEOUT`]. If it fails or is
    /// cancelled, the job is forgotten: a response that still comes is kept as an event.
    pub(crate) async fn call<M: ServiceMethod>(&mut self, request: M::Request) -> Result<M::Response, ClientError> {
        self.exchange::<Service<M>>(request).await
    }

    /// Sends a job message and waits for the message answering it, as [`Connection::call`] does.
    pub(crate) async fn call_job<J: JobMessage>(&mut self, request: J::Request) -> Result<J::Response, ClientError> {
        self.exchange::<Job<J>>(request).await
    }

    async fn exchange<E: Exchange>(&mut self, request: E::Request) -> Result<E::Response, ClientError> {
        let call = self.protocol.exchange::<E>(request);
        let guard = ForgetOnDrop {
            job_id: call.job_id(),
            connection: self,
        };
        timeout(
            RESPONSE_TIMEOUT,
            guard.connection.wait(|protocol| protocol.take_response(&call)),
        )
        .await
        .unwrap_or(Err(ClientError::Timeout(E::RESPONSE)))?
    }

    /// Sends `ClientLogon`, with `steam_id` on the header, and waits for Steam's answer.
    pub(crate) async fn log_on(&mut self, steam_id: SteamID, logon: CMsgClientLogon) -> Result<LogonInfo, ClientError> {
        self.protocol.log_on(steam_id, logon);
        timeout(RESPONSE_TIMEOUT, self.wait(|protocol| protocol.logon_result()))
            .await
            .map_err(|_| ClientError::Timeout(EMsg::ClientLogOnResponse))??
    }

    /// Logs off, and waits for Steam to confirm or close the connection.
    pub(crate) async fn log_off(&mut self) -> Result<(), ClientError> {
        self.protocol.log_off();
        let logged_off = timeout(RESPONSE_TIMEOUT, self.wait(|protocol| protocol.logged_off()))
            .await
            .map_err(|_| ClientError::Timeout(EMsg::ClientLoggedOff))?;
        match logged_off.and_then(|result| result) {
            Ok(result) => info!("Logged off: {:?}", result),
            Err(ClientError::Dropped) => info!("Logged off, Steam closed the connection"),
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// Waits for the next message Steam sends that nothing waits for.
    pub(crate) async fn next_event(&mut self) -> Result<Event, ClientError> {
        self.wait(CmProtocol::poll_event).await
    }

    /// Drives the connection until `until` takes something from the protocol: writes what it gives, reads into it,
    /// and wakes it when it asks.
    async fn wait<R>(&mut self, mut until: impl FnMut(&mut CmProtocol) -> Option<R>) -> Result<R, ClientError> {
        loop {
            self.flush().await?;
            if let Some(result) = until(&mut self.protocol) {
                return Ok(result);
            }
            self.receive().await?;
        }
    }

    /// Writes what the protocol gave while it waits for the next read or heartbeat, and gives that to the protocol. A
    /// write that blocks doesn't keep it from reading: Steam's messages, and the responses to calls, still come in.
    ///
    /// Cancel-safe, as every method that awaits it: what was read is in the protocol, and what wasn't written yet is
    /// written by the next step. What a cancelled method waited for stays in the protocol, except for a call, whose
    /// response becomes an event.
    pub(crate) async fn step(&mut self) -> Result<(), ClientError> {
        let mut timeout = pin!(sleep_until(self.protocol.poll_timeout()));
        poll_fn(|cx| {
            if let Poll::Ready(Err(error)) = self.poll_flush(cx) {
                return Poll::Ready(Err(error));
            }
            self.poll_receive(cx, timeout.as_mut())
        })
        .await
    }

    /// Waits for one read, or the next heartbeat.
    async fn receive(&mut self) -> Result<(), ClientError> {
        let mut timeout = pin!(sleep_until(self.protocol.poll_timeout()));
        poll_fn(|cx| self.poll_receive(cx, timeout.as_mut())).await
    }

    /// Writes everything the protocol gave to write.
    async fn flush(&mut self) -> Result<(), ClientError> {
        poll_fn(|cx| self.poll_flush(cx)).await
    }

    /// Reads into the protocol, or wakes it once `timeout` passed.
    fn poll_receive(
        &mut self,
        cx: &mut Context<'_>,
        timeout: Pin<&mut impl Future<Output = ()>>,
    ) -> Poll<Result<(), ClientError>> {
        // a new future for each poll: reading is cancel-safe, so nothing is lost when it's dropped
        let read = pin!(self.stream.read_buf(self.protocol.receive_buffer())).poll(cx);
        match read {
            Poll::Ready(Ok(0)) => return Poll::Ready(Err(self.closed(None))),
            Poll::Ready(Ok(_)) => return Poll::Ready(self.protocol.handle_received(now())),
            Poll::Ready(Err(error)) => return Poll::Ready(Err(self.closed(Some(error)))),
            Poll::Pending => {}
        }
        match timeout.poll(cx) {
            Poll::Ready(()) => {
                self.protocol.handle_timeout(now());
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    /// Writes what the protocol gave, and flushes it. Ready once everything is written.
    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), ClientError>> {
        loop {
            if self.writing.is_empty() {
                // not kept once written, so the protocol's buffer is free to reuse
                self.writing = match self.protocol.poll_transmit() {
                    Some(transmit) => transmit,
                    None => Bytes::new(),
                };
                if self.writing.is_empty() {
                    break;
                }
            }
            match Pin::new(&mut self.stream).poll_write(cx, &self.writing) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(self.closed(None))),
                Poll::Ready(Ok(written)) => self.writing.advance(written),
                Poll::Ready(Err(error)) => return Poll::Ready(Err(self.closed(Some(error)))),
                Poll::Pending => return Poll::Pending,
            }
        }
        match Pin::new(&mut self.stream).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(self.closed(Some(error)))),
            Poll::Pending => Poll::Pending,
        }
    }

    /// The connection closed, or failed: the protocol takes nothing more. Steam closing it is `Dropped`, however the
    /// socket says it.
    fn closed(&mut self, error: Option<io::Error>) -> ClientError {
        self.protocol.handle_closed();
        self.writing = Bytes::new();
        match error {
            None => ClientError::Dropped,
            Some(error) => match error.kind() {
                io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::UnexpectedEof => ClientError::Dropped,
                _ => error.into(),
            },
        }
    }
}

/// Forgets a call when `Connection::call` ends, whether it got its response, failed or was cancelled.
struct ForgetOnDrop<'a, T> {
    connection: &'a mut Connection<T>,
    job_id: u64,
}

impl<T> Drop for ForgetOnDrop<'_, T> {
    fn drop(&mut self) {
        // nothing to forget once the response was taken
        self.connection.protocol.forget(self.job_id);
    }
}

impl<T> fmt::Debug for Connection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("protocol", &self.protocol)
            .field("writing", &self.writing.len())
            .finish_non_exhaustive()
    }
}

/// Has the OS check that the other end of a quiet connection is still there, so that a connection Steam lost without
/// closing it fails. See [`KEEPALIVE_TIME`].
fn keep_alive(stream: &TcpStream) -> io::Result<()> {
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(KEEPALIVE_TIME)
        .with_interval(KEEPALIVE_INTERVAL);
    socket2::SockRef::from(stream).set_tcp_keepalive(&keepalive)
}

/// The time, for the protocol. Tokio's clock, so tests can pause it.
fn now() -> std::time::Instant {
    Instant::now().into_std()
}

/// Sleeps until `deadline`, or forever if there is none.
pub(crate) async fn sleep_until(deadline: Option<std::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::panic::Location;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering as AtomicOrdering;
    use std::sync::Mutex;
    use std::sync::Once;
    use std::sync::PoisonError;

    use bytes::Bytes;
    use bytes::BytesMut;
    use steam_cryptography::SessionKey;
    use steam_language_gen::generated::enums::EResult;
    use steam_language_gen::HasJobId;
    use steam_language_gen::MessageHeaderWrapper;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
    use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;
    use tokio::io::duplex;
    use tokio::io::AsyncWriteExt;
    use tokio::io::DuplexStream;

    use super::*;
    use crate::messages::packet::PacketMessage;
    use crate::protocol::framing;
    use crate::protocol::proto_message;
    use crate::protocol::tests::ANONYMOUS_LOGON_RESPONSE;
    use crate::protocol::tests::ANONYMOUS_STEAM_ID;
    use crate::service::methods::GetPasswordRSAPublicKey;
    use crate::service::ServiceRequest;

    /// ChannelEncryptRequest captured from a CM.
    const CHANNEL_ENCRYPT_REQUEST: [u8; 44] = [
        23, 5, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 1, 0,
        0, 0, 66, 126, 251, 245, 88, 122, 243, 123, 102, 163, 11, 54, 151, 145, 31, 54,
    ];

    /// Longer than any wait in a test. In paused time it costs nothing: a wait that never ends fails the test, saying
    /// where, instead of hanging it.
    pub(crate) const TEST_TIMEOUT: Duration = Duration::from_secs(600);

    /// Fails the test, saying where, if `future` takes longer than [`TEST_TIMEOUT`].
    #[track_caller]
    pub(crate) fn soon<F: Future>(future: F) -> impl Future<Output = F::Output> {
        within(TEST_TIMEOUT, future)
    }

    /// Fails the test, saying where, if `future` takes longer than `limit`: for tests in real time.
    #[track_caller]
    pub(crate) fn within<F: Future>(limit: Duration, future: F) -> impl Future<Output = F::Output> {
        let caller = Location::caller();
        async move {
            let _watched = Watched::new(caller);
            timeout(limit, future)
                .await
                .unwrap_or_else(|_| panic!("still waiting after {:?}, at {}", limit, caller))
        }
    }

    /// How long a wait may take in real time. Paused time only moves while the runtime has nothing to run, so a task
    /// that never stops running (a livelock) keeps a timeout from firing: past this, the test binary stops instead.
    const REAL_TIME_LIMIT: Duration = Duration::from_secs(60);

    /// The waits running, by id: when each started, and where. A thread checks them every second.
    static WAITS: Mutex<Vec<(u64, std::time::Instant, &Location<'static>)>> = Mutex::new(Vec::new());

    /// A wait [`REAL_TIME_LIMIT`] watches, until dropped.
    struct Watched(u64);

    impl Watched {
        fn new(caller: &'static Location<'static>) -> Self {
            static IDS: AtomicU64 = AtomicU64::new(0);
            static WATCHDOG: Once = Once::new();
            WATCHDOG.call_once(|| {
                std::thread::spawn(|| loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let waits = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some((_, _, caller)) = waits.iter().find(|wait| wait.1.elapsed() > REAL_TIME_LIMIT) {
                        eprintln!(
                            "still waiting after {:?} of real time, at {}: something never stops running",
                            REAL_TIME_LIMIT, caller
                        );
                        std::process::abort();
                    }
                });
            });
            let id = IDS.fetch_add(1, AtomicOrdering::Relaxed);
            let mut waits = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
            waits.push((id, std::time::Instant::now(), caller));
            Self(id)
        }
    }

    impl Drop for Watched {
        fn drop(&mut self) {
            let mut waits = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
            waits.retain(|wait| wait.0 != self.0);
        }
    }

    pub(crate) fn key() -> SessionKey {
        SessionKey::from([5; 32])
    }

    /// A connection already encrypted, and a fake CM at the other end of it.
    pub(crate) fn connection() -> (Connection<DuplexStream>, FakeCm) {
        let (client, server) = duplex(64 * 1024);
        (
            Connection::new(client, CmProtocol::encrypted(key())),
            FakeCm::new(server, key()),
        )
    }

    /// The CM end of a connection: reads what the client sent, and answers.
    pub(crate) struct FakeCm<S = DuplexStream> {
        stream: S,
        key: SessionKey,
        received: BytesMut,
    }

    /// A message the fake CM received.
    #[derive(Debug)]
    pub(crate) struct Call {
        pub(crate) emsg: EMsg,
        pub(crate) header: CMsgProtoBufHeader,
        pub(crate) body: Bytes,
    }

    impl Call {
        pub(crate) fn body<M: Message>(&self) -> M {
            M::parse_from_tokio_bytes(&self.body).unwrap()
        }
    }

    impl<S: AsyncRead + AsyncWrite + Unpin> FakeCm<S> {
        pub(crate) fn new(stream: S, key: SessionKey) -> Self {
            Self {
                stream,
                key,
                received: BytesMut::new(),
            }
        }

        /// The next frame the client sent, as is. `None` once the client closed the connection.
        async fn read_frame(&mut self) -> Option<BytesMut> {
            loop {
                if let Some(frame) = framing::decode(&mut self.received).unwrap() {
                    return Some(frame);
                }
                if self.stream.read_buf(&mut self.received).await.unwrap() == 0 {
                    return None;
                }
            }
        }

        /// The next message the client sent, which must have a protobuf header. Fails the test if none comes
        /// [`soon`].
        #[track_caller]
        pub(crate) fn read(&mut self) -> impl Future<Output = Call> + '_ {
            soon(async move {
                let frame = self.read_frame().await.expect("the client closed the connection");
                let message = framing::decrypt(frame, &self.key).unwrap();
                let (emsg, header, body) = PacketMessage::from_bytes(message).unwrap().into_parts();
                match header {
                    MessageHeaderWrapper::Proto(header) => Call { emsg, header, body },
                    header => panic!("expected a protobuf header, got {:?}", header),
                }
            })
        }

        /// Reads a call with an `R`, checking how it was sent.
        #[track_caller]
        pub(crate) fn expect<R: ServiceRequest>(&mut self, emsg: EMsg) -> impl Future<Output = (Call, R)> + '_ {
            let read = self.read();
            async move {
                let call = read.await;
                assert_eq!(call.emsg, emsg);
                assert_eq!(call.header.target_job_name(), R::Method::METHOD);
                let body = call.body::<R>();
                (call, body)
            }
        }

        pub(crate) async fn send(&mut self, message: Vec<u8>) {
            self.stream
                .write_all(&framing::encrypt(&message, &self.key))
                .await
                .unwrap();
        }

        /// Answers `call` with `result` and `body`.
        pub(crate) async fn reply<R: Message>(&mut self, call: &Call, result: EResult, body: R) {
            let mut header = CMsgProtoBufHeader::new();
            header.set_jobid_target(call.header.jobid_source());
            header.set_eresult(result as i32);
            self.send(proto_message(EMsg::ServiceMethodResponse, header, body))
                .await;
        }

        /// Answers the channel encryption handshake as Steam, with `result`. Can't read what the client sends after.
        pub(crate) async fn encrypt_channel(&mut self, result: EResult) {
            self.stream
                .write_all(&framing::frame(&CHANNEL_ENCRYPT_REQUEST))
                .await
                .unwrap();
            let response = soon(self.read_frame()).await.unwrap();
            let response = PacketMessage::from_bytes(response.freeze()).unwrap();
            assert_eq!(response.emsg(), EMsg::ChannelEncryptResponse);

            let mut encrypt_result = (EMsg::ChannelEncryptResult as u32).to_le_bytes().to_vec();
            encrypt_result.extend_from_slice(&[0xFF; 16]);
            encrypt_result.extend_from_slice(&(result as u32).to_le_bytes());
            self.stream.write_all(&framing::frame(&encrypt_result)).await.unwrap();
        }

        /// Waits for the client to close the connection, skipping what it sends.
        #[track_caller]
        pub(crate) fn closed(&mut self) -> impl Future<Output = ()> + '_ {
            soon(async move { while self.read_frame().await.is_some() {} })
        }
    }

    fn rsa_key_request() -> CAuthentication_GetPasswordRSAPublicKey_Request {
        CAuthentication_GetPasswordRSAPublicKey_Request::new()
    }

    fn logged_off(result: EResult) -> Vec<u8> {
        let mut logged_off = CMsgClientLoggedOff::new();
        logged_off.set_eresult(result as i32);
        proto_message(EMsg::ClientLoggedOff, CMsgProtoBufHeader::new(), logged_off)
    }

    #[tokio::test(start_paused = true)]
    async fn opens_the_channel() {
        let (client, server) = duplex(64 * 1024);
        let mut steam = FakeCm::new(server, key());

        let (connection, ()) = tokio::join!(soon(Connection::open(client)), steam.encrypt_channel(EResult::OK));
        let mut connection = connection.unwrap();

        // the fake CM can't decrypt the session key: it takes the client's
        let mut cm = FakeCm::new(steam.stream, connection.protocol.session_key().unwrap().clone());
        connection
            .send(EMsg::ClientHello, CMsgClientHello::new())
            .await
            .unwrap();
        assert_eq!(cm.read().await.emsg, EMsg::ClientHello);
    }

    #[tokio::test(start_paused = true)]
    async fn a_refused_channel_is_an_error() {
        let (client, server) = duplex(64 * 1024);
        let mut steam = FakeCm::new(server, key());

        let (connection, ()) = tokio::join!(soon(Connection::open(client)), steam.encrypt_channel(EResult::Fail));
        assert!(matches!(connection, Err(ClientError::EncryptionFailed(EResult::Fail))));
    }

    #[tokio::test(start_paused = true)]
    async fn a_connection_closed_during_the_handshake_is_dropped() {
        let (client, server) = duplex(64 * 1024);
        drop(server);
        assert!(matches!(
            soon(Connection::open(client)).await,
            Err(ClientError::Dropped)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn writing_to_a_closed_connection_is_dropped() {
        let (mut connection, cm) = connection();
        drop(cm);
        // a broken pipe is Steam closing the connection
        assert!(matches!(
            connection.send(EMsg::ClientHello, CMsgClientHello::new()).await,
            Err(ClientError::Dropped)
        ));
        // and the protocol knows
        assert!(matches!(
            connection.protocol().logged_off(),
            Some(Err(ClientError::Dropped))
        ));
        connection.log_off().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_call_on_a_closed_connection_is_dropped() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
        });
        assert!(matches!(
            connection.call::<GetPasswordRSAPublicKey>(rsa_key_request()).await,
            Err(ClientError::Dropped)
        ));
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn a_frame_that_fails_to_decrypt_ends_the_connection() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
            cm.stream
                .write_all(&framing::encrypt(&[0; 16], &SessionKey::from([1; 32])))
                .await
                .unwrap();
            cm
        });
        assert!(matches!(
            connection.call::<GetPasswordRSAPublicKey>(rsa_key_request()).await,
            Err(ClientError::Packet(_))
        ));
        let _cm = soon(server).await.unwrap();

        // what follows ends at once, though the socket is still open
        assert!(matches!(
            connection.call::<GetPasswordRSAPublicKey>(rsa_key_request()).await,
            Err(ClientError::Dropped)
        ));
        assert!(matches!(
            connection
                .log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new())
                .await,
            Err(ClientError::Dropped)
        ));
        connection.log_off().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn logs_on_and_off() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            let logon = cm.read().await;
            assert_eq!(logon.emsg, EMsg::ClientLogon);
            assert_eq!(logon.header.steamid(), 0x01A0_0000_0000_0000);
            cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;

            let log_off = cm.read().await;
            assert_eq!(log_off.emsg, EMsg::ClientLogOff);
            assert_eq!(log_off.header.steamid(), ANONYMOUS_STEAM_ID);
            cm.send(logged_off(EResult::OK)).await;
        });

        let logon = connection
            .log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new())
            .await
            .unwrap();
        assert_eq!(logon.session.steam_id.to_steam64(), ANONYMOUS_STEAM_ID);
        connection.log_off().await.unwrap();
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn refused_logons_are_errors() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
            let mut response = CMsgClientLogonResponse::new();
            response.set_eresult(EResult::InvalidPassword as i32);
            cm.send(proto_message(
                EMsg::ClientLogOnResponse,
                CMsgProtoBufHeader::new(),
                response,
            ))
            .await;
        });

        let logon = connection
            .log_on(SteamID::from_steam64(0x0110_0001_0000_0000), CMsgClientLogon::new())
            .await;
        assert!(matches!(
            logon,
            Err(ClientError::LogonFailed {
                result: EResult::InvalidPassword,
                ..
            })
        ));
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn logging_off_succeeds_when_steam_closes_the_connection() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            assert_eq!(cm.read().await.emsg, EMsg::ClientLogOff);
        });
        connection.log_off().await.unwrap();
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn logging_off_fails_on_an_invalid_frame() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
            cm.stream.write_all(b"\x01\x00\x00\x00VT02\x00").await.unwrap();
            cm
        });
        assert!(matches!(connection.log_off().await, Err(ClientError::Packet(_))));
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn steam_has_a_while_to_answer() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            // a call answered too late
            let call = cm.read().await;
            tokio::time::sleep(RESPONSE_TIMEOUT + Duration::from_secs(1)).await;
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            )
            .await;
            // a logon and a log off never answered
            cm.read().await;
            cm.read().await;
            cm
        });

        assert!(matches!(
            connection.call::<GetPasswordRSAPublicKey>(rsa_key_request()).await,
            Err(ClientError::Timeout(EMsg::ServiceMethodResponse))
        ));
        // the late response is kept
        match soon(connection.next_event()).await.unwrap() {
            Event::Message(packet) => assert_eq!(packet.parse().unwrap().header().target(), 1),
            event => panic!("expected the late response, got {:?}", event),
        }

        assert!(matches!(
            connection
                .log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new())
                .await,
            Err(ClientError::Timeout(EMsg::ClientLogOnResponse))
        ));
        assert!(matches!(
            connection.log_off().await,
            Err(ClientError::Timeout(EMsg::ClientLoggedOff))
        ));
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn sends_heartbeats_while_waiting() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
            cm.send(ANONYMOUS_LOGON_RESPONSE.to_vec()).await;

            // answers 20 s late: two heartbeats come first, 9 s apart
            let call = cm.read().await;
            let start = Instant::now();
            let mut heartbeats = vec![];
            for _ in 0..2 {
                let heartbeat = cm.read().await;
                assert_eq!(heartbeat.emsg, EMsg::ClientHeartBeat);
                assert_eq!(heartbeat.header.steamid(), ANONYMOUS_STEAM_ID);
                heartbeats.push(start.elapsed());
            }
            tokio::time::sleep_until(start + Duration::from_secs(20)).await;
            cm.reply(
                &call,
                EResult::OK,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            )
            .await;
            heartbeats
        });

        connection
            .log_on(SteamID::from_steam64(0x01A0_0000_0000_0000), CMsgClientLogon::new())
            .await
            .unwrap();
        connection
            .call::<GetPasswordRSAPublicKey>(rsa_key_request())
            .await
            .unwrap();
        assert_eq!(
            soon(server).await.unwrap(),
            [Duration::from_secs(9), Duration::from_secs(18)]
        );
    }

    /// A stream that takes no bytes.
    struct Full;

    impl AsyncRead for Full {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Pending
        }
    }

    impl AsyncWrite for Full {
        fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(0))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_stream_that_takes_nothing_is_dropped() {
        let mut connection = Connection::new(Full, CmProtocol::encrypted(key()));
        assert!(matches!(
            connection.send(EMsg::ClientHello, CMsgClientHello::new()).await,
            Err(ClientError::Dropped)
        ));
    }

    /// A stream that writes the first bytes it's given, then takes nothing until `blocked` is cleared, so that a
    /// write can be cancelled halfway.
    struct Stalling {
        stream: DuplexStream,
        wrote: bool,
        blocked: bool,
    }

    impl AsyncRead for Stalling {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for Stalling {
        fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
            if !self.blocked {
                return Pin::new(&mut self.stream).poll_write(cx, buf);
            }
            if self.wrote {
                return Poll::Pending;
            }
            self.wrote = true;
            Pin::new(&mut self.stream).poll_write(cx, &buf[..5])
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.stream).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.stream).poll_shutdown(cx)
        }
    }

    fn hello(version: u32) -> CMsgClientHello {
        let mut hello = CMsgClientHello::new();
        hello.set_protocol_version(version);
        hello
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_write_resumes_where_it_stopped() {
        let (client, server) = duplex(64 * 1024);
        let stream = Stalling {
            stream: client,
            wrote: false,
            blocked: true,
        };
        let mut connection = Connection::new(stream, CmProtocol::encrypted(key()));
        let mut cm = FakeCm::new(server, key());

        // cancelled with 5 bytes of the frame written, then with nothing written
        let second = Duration::from_secs(1);
        assert!(timeout(second, connection.send(EMsg::ClientHello, hello(1)))
            .await
            .is_err());
        assert!(
            timeout(second, connection.call::<GetPasswordRSAPublicKey>(rsa_key_request()))
                .await
                .is_err()
        );

        // the next write resumes: every frame arrives whole, and decrypts
        connection.stream.blocked = false;
        connection.send(EMsg::ClientHello, hello(2)).await.unwrap();
        assert_eq!(cm.read().await.body::<CMsgClientHello>().protocol_version(), 1);
        let call = cm.read().await;
        assert_eq!(
            call.header.target_job_name(),
            "Authentication.GetPasswordRSAPublicKey#1"
        );
        assert_eq!(cm.read().await.body::<CMsgClientHello>().protocol_version(), 2);

        // and the cancelled call's response is an event
        cm.reply(
            &call,
            EResult::OK,
            CAuthentication_GetPasswordRSAPublicKey_Response::new(),
        )
        .await;
        match soon(connection.next_event()).await.unwrap() {
            Event::Message(message) => assert_eq!(message.emsg(), EMsg::ServiceMethodResponse),
            event => panic!("expected the response, got {:?}", event),
        }
    }

    /// A stream whose reads, writes and flushes fail with `kind`; flushes only if `flushes_fail`.
    struct Failing(io::ErrorKind, bool);

    impl AsyncRead for Failing {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(self.0.into()))
        }
    }

    impl AsyncWrite for Failing {
        fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
            Poll::Ready(Err(self.0.into()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(if self.1 { Err(self.0.into()) } else { Ok(()) })
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn steam_closing_the_connection_is_dropped_however_the_socket_says_it() {
        use io::ErrorKind::*;

        for kind in [BrokenPipe, ConnectionReset, ConnectionAborted, UnexpectedEof] {
            // on write
            let mut connection = Connection::new(Failing(kind, false), CmProtocol::encrypted(key()));
            assert!(matches!(
                connection.send(EMsg::ClientHello, hello(1)).await,
                Err(ClientError::Dropped)
            ));
            assert!(matches!(
                connection.protocol().logged_off(),
                Some(Err(ClientError::Dropped))
            ));
            connection.log_off().await.unwrap();

            // on read, and on flush
            let mut connection = Connection::new(Failing(kind, false), CmProtocol::encrypted(key()));
            assert!(matches!(soon(connection.next_event()).await, Err(ClientError::Dropped)));
            let mut connection = Connection::new(Failing(kind, true), CmProtocol::encrypted(key()));
            assert!(matches!(soon(connection.flush()).await, Err(ClientError::Dropped)));
        }

        // other errors are what they are, and end the connection too
        let mut connection = Connection::new(Failing(PermissionDenied, false), CmProtocol::encrypted(key()));
        assert!(matches!(
            soon(connection.next_event()).await,
            Err(ClientError::Packet(_))
        ));
        assert!(matches!(
            connection.protocol().logged_off(),
            Some(Err(ClientError::Dropped))
        ));
    }

    #[tokio::test]
    async fn tcp_connections_are_kept_alive() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let steam = async {
            let mut steam = FakeCm::new(listener.accept().await.unwrap().0, key());
            steam.encrypt_channel(EResult::OK).await;
            steam
        };
        let (connection, _steam) = within(Duration::from_secs(10), async {
            tokio::join!(Connection::connect(&endpoint), steam)
        })
        .await;
        let connection = connection.unwrap();
        assert!(socket2::SockRef::from(&connection.stream).keepalive().unwrap());
    }

    #[test]
    fn debug_shows_the_state() {
        let (connection, _cm) = connection();
        let debug = format!("{:?}", connection);
        assert!(debug.contains("Encrypted"), "{}", debug);
    }
}

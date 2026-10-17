//! Connects to a Steam CM server and logs on.
//!
//! The client is a type state: [`SteamClient<Connected>`] is connected over an encrypted channel, and logging on
//! consumes it into a [`SteamClient<LoggedOn>`].
//!
//! ```no_run
//! # async fn run() -> Result<(), steam_client::errors::ClientError> {
//! use steam_client::client::SteamClient;
//!
//! let client = SteamClient::connect().await?.login_anonymous().await?;
//! println!("Logged on as {}", client.steam_id());
//! client.log_off().await?;
//! # Ok(())
//! # }
//! ```

use std::net::Ipv4Addr;
use std::time::Duration;

use num_traits::FromPrimitive;
use steam_language_gen::generated::enums::EAccountType;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EOSType;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::MessageHeaderWrapper;
use steam_language_gen::SerializableBytes;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHeartBeat;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogOff;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogon;
use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLogonResponse;
use steamid_parser::SteamID;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::connection;
use crate::connection::Connection;
use crate::connection::Sink;
use crate::content_manager::dump_tcp_servers;
use crate::errors::ClientError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;

/// `MsgClientLogon.CurrentProtocol` in SteamKit's `steammsg.steamd`.
const PROTOCOL_VERSION: u32 = 65581;
/// How many servers of the CM list [`SteamClient::connect`] tries.
const SERVERS_TO_TRY: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(target_os = "windows")]
const OS_TYPE: EOSType = EOSType::Windows10;
#[cfg(target_os = "macos")]
const OS_TYPE: EOSType = EOSType::MacOSUnknown;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const OS_TYPE: EOSType = EOSType::LinuxUnknown;

/// A client of a Steam CM server, in state `S`: [`Connected`] or [`LoggedOn`].
#[derive(Debug)]
pub struct SteamClient<S> {
    connection: Connection,
    /// The CM server, as `ip:port`.
    endpoint: String,
    state: S,
}

/// Connected to a CM server over an encrypted channel, and not logged on.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Connected;

/// Logged on to Steam. The client sends heartbeats while in this state.
#[derive(Debug)]
pub struct LoggedOn {
    session: Session,
    heartbeat_interval: Duration,
    cell_id: u32,
    public_ip: Option<Ipv4Addr>,
    _heartbeat: Heartbeat,
}

impl<S> SteamClient<S> {
    /// The CM server, as `ip:port`.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl SteamClient<Connected> {
    /// Connects to one of the first CM servers of the list Steam's directory returns.
    pub async fn connect() -> Result<Self, ClientError> {
        let servers = dump_tcp_servers()
            .await
            .map_err(|error| ClientError::ServerList(error.into()))?;

        for endpoint in servers.iter().take(SERVERS_TO_TRY) {
            match Self::connect_to(endpoint).await {
                Ok(client) => return Ok(client),
                Err(error) => warn!("Could not connect to {}: {}", endpoint, error),
            }
        }
        Err(ClientError::NoServerAvailable)
    }

    /// Connects to the CM server at `endpoint` (`ip:port`), encrypts the channel and says hello.
    pub async fn connect_to(endpoint: &str) -> Result<Self, ClientError> {
        let connection = timeout(CONNECT_TIMEOUT, Connection::connect(endpoint))
            .await
            .map_err(|_| ClientError::Timeout(EMsg::ChannelEncryptResult))??;

        let mut hello = CMsgClientHello::new();
        hello.set_protocol_version(PROTOCOL_VERSION);
        trace!("Sending ClientHello");
        connection
            .send(proto_message(EMsg::ClientHello, CMsgProtoBufHeader::new(), hello))
            .await?;

        info!("Connected to {}", endpoint);
        Ok(Self {
            connection,
            endpoint: endpoint.to_string(),
            state: Connected,
        })
    }

    /// Logs on as an anonymous user, which can only use a few services, such as downloading public content.
    pub async fn login_anonymous(mut self) -> Result<SteamClient<LoggedOn>, ClientError> {
        trace!("Sending ClientLogon");
        self.connection.send(anonymous_logon()).await?;

        let response = timeout(RESPONSE_TIMEOUT, self.wait_for_logon_response())
            .await
            .map_err(|_| ClientError::Timeout(EMsg::ClientLogOnResponse))??;
        let (session, response) = logon_result(response)?;

        let heartbeat_interval = heartbeat_interval(&response);
        let heartbeat = Heartbeat::start(self.connection.sink(), session, heartbeat_interval);
        let public_ip = response
            .public_ip
            .has_v4()
            .then(|| Ipv4Addr::from(response.public_ip.v4()));
        info!("Logged on as {}", session.steam_id);

        Ok(SteamClient {
            connection: self.connection,
            endpoint: self.endpoint,
            state: LoggedOn {
                session,
                heartbeat_interval,
                cell_id: response.cell_id(),
                public_ip,
                _heartbeat: heartbeat,
            },
        })
    }

    async fn wait_for_logon_response(&mut self) -> Result<PacketMessage, ClientError> {
        loop {
            let packet = self.connection.recv().await?;
            match packet.emsg() {
                EMsg::ClientLogOnResponse => return Ok(packet),
                EMsg::ClientLoggedOff => return Err(ClientError::LoggedOff(logged_off_result(packet))),
                emsg => debug!("Skipping {:?} while logging on", emsg),
            }
        }
    }
}

impl SteamClient<LoggedOn> {
    /// Our SteamID, which Steam assigned on logon.
    pub fn steam_id(&self) -> SteamID {
        self.state.session.steam_id
    }

    /// The session id Steam assigned on logon.
    pub fn session_id(&self) -> i32 {
        self.state.session.session_id
    }

    /// How often the client sends a heartbeat.
    pub fn heartbeat_interval(&self) -> Duration {
        self.state.heartbeat_interval
    }

    /// The cell (region) Steam placed us in.
    pub fn cell_id(&self) -> u32 {
        self.state.cell_id
    }

    /// Our public IP, as Steam sees it.
    pub fn public_ip(&self) -> Option<Ipv4Addr> {
        self.state.public_ip
    }

    /// Logs off, and waits for Steam to confirm or close the connection.
    ///
    /// Dropping the client instead closes the connection without logging off.
    pub async fn log_off(mut self) -> Result<(), ClientError> {
        let session = self.state.session;
        trace!("Sending ClientLogOff");
        self.connection
            .send(proto_message(
                EMsg::ClientLogOff,
                session.header(),
                CMsgClientLogOff::new(),
            ))
            .await?;

        let confirmation = timeout(RESPONSE_TIMEOUT, async {
            loop {
                match self.connection.recv().await {
                    Ok(packet) if packet.emsg() == EMsg::ClientLoggedOff => {
                        info!("Logged off: {:?}", logged_off_result(packet));
                        return Ok(());
                    }
                    Ok(packet) => debug!("Skipping {:?} while logging off", packet.emsg()),
                    Err(ClientError::Dropped) => {
                        info!("Logged off, Steam closed the connection");
                        return Ok(());
                    }
                    Err(error) => return Err(error),
                }
            }
        });
        confirmation
            .await
            .map_err(|_| ClientError::Timeout(EMsg::ClientLoggedOff))?
    }
}

/// Identifies the session on the header of the messages we send, once logged on.
#[derive(Debug, Clone, Copy)]
struct Session {
    steam_id: SteamID,
    session_id: i32,
}

impl Session {
    fn header(&self) -> CMsgProtoBufHeader {
        let mut header = CMsgProtoBufHeader::new();
        header.set_steamid(self.steam_id.to_steam64());
        header.set_client_sessionid(self.session_id);
        header
    }
}

/// Sends `ClientHeartBeat` on an interval, until dropped.
#[derive(Debug)]
struct Heartbeat(JoinHandle<()>);

impl Heartbeat {
    fn start(sink: Sink<tokio::net::TcpStream>, session: Session, interval: Duration) -> Self {
        let heartbeat = proto_message(EMsg::ClientHeartBeat, session.header(), CMsgClientHeartBeat::new());

        Self(tokio::spawn(async move {
            let mut ticks = tokio::time::interval(interval);
            // the first tick completes immediately
            ticks.tick().await;
            loop {
                ticks.tick().await;
                if let Err(error) = connection::send(&sink, heartbeat.clone()).await {
                    warn!("Stopping the heartbeat, it could not be sent: {}", error);
                    return;
                }
                trace!("Sent a heartbeat");
            }
        }))
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The SteamID anonymous users log on with: an anonymous user in the public universe, instance 0, account id 0.
fn anonymous_steam_id() -> SteamID {
    SteamID::from_steam64((EUniverse::Public as u64) << 56 | (EAccountType::AnonUser as u64) << 52)
}

/// `ClientLogon` for an anonymous user, as SteamKit's `SteamUser.LogOnAnonymous`.
fn anonymous_logon() -> Vec<u8> {
    let mut header = CMsgProtoBufHeader::new();
    header.set_steamid(anonymous_steam_id().to_steam64());
    header.set_client_sessionid(0);

    let mut logon = CMsgClientLogon::new();
    logon.set_protocol_version(PROTOCOL_VERSION);
    logon.set_client_os_type(OS_TYPE as i32 as u32);
    logon.set_client_language("english".to_string());
    logon.set_cell_id(0);

    proto_message(EMsg::ClientLogon, header, logon)
}

/// Serializes a message with a protobuf header.
fn proto_message<M: Message>(emsg: EMsg, header: CMsgProtoBufHeader, body: M) -> Vec<u8> {
    let mut message = ClientMessage::<M>::new_proto(emsg);
    message.wrapped_header = MessageHeaderWrapper::Proto(header);
    message.body = body;
    message.to_bytes()
}

/// Checks a `ClientLogOnResponse`. Returns the session Steam assigned, with the response.
fn logon_result(packet: PacketMessage) -> Result<(Session, CMsgClientLogonResponse), ClientError> {
    if !matches!(packet.header(), MessageHeaderWrapper::Proto(_)) {
        // Steam sends it without a protobuf header when it got no `ClientLogon`
        return Err(ClientError::LogonFailed {
            result: EResult::Invalid,
            extended: None,
        });
    }
    let response = ClientMessage::<CMsgClientLogonResponse>::from_proto_packet(packet)?;

    let result = eresult(response.body.eresult());
    if result != EResult::OK {
        return Err(ClientError::LogonFailed {
            result,
            extended: response.body.eresult_extended.map(eresult),
        });
    }

    let header = match &response.wrapped_header {
        MessageHeaderWrapper::Proto(header) => header,
        _ => unreachable!("checked above"),
    };
    let session = Session {
        steam_id: SteamID::from_steam64(header.steamid()),
        session_id: header.client_sessionid(),
    };
    Ok((session, response.body))
}

fn heartbeat_interval(response: &CMsgClientLogonResponse) -> Duration {
    // SteamKit uses the out of game interval
    let seconds = match response.legacy_out_of_game_heartbeat_seconds() {
        seconds if seconds > 0 => seconds,
        _ => response.heartbeat_seconds(),
    };
    Duration::from_secs(seconds.max(1) as u64)
}

fn logged_off_result(packet: PacketMessage) -> EResult {
    match ClientMessage::<CMsgClientLoggedOff>::from_proto_packet(packet) {
        Ok(message) => eresult(message.body.eresult()),
        Err(_) => EResult::Invalid,
    }
}

fn eresult(value: i32) -> EResult {
    EResult::from_i32(value).unwrap_or_else(|| {
        warn!("Unknown EResult {}", value);
        EResult::Invalid
    })
}

#[cfg(test)]
mod tests {
    use steam_language_gen::generated::enums::EAccountType;
    use steam_language_gen::HasJobId;

    use super::*;
    use crate::messages::MessageKind;

    #[test]
    fn anonymous_steam_id_is_an_anonymous_public_user() {
        let steam_id = anonymous_steam_id();
        assert_eq!(steam_id.to_steam64(), 0x01A0_0000_0000_0000);
        assert_eq!(steam_id.universe(), Some(EUniverse::Public));
        assert_eq!(steam_id.account_type(), Some(EAccountType::AnonUser));
        assert_eq!(steam_id.instance(), 0);
        assert_eq!(steam_id.account_id(), 0);
    }

    #[test]
    fn writes_the_anonymous_logon() {
        let logon = anonymous_logon();

        // EMsg::ClientLogon flagged as protobuf
        assert_eq!(logon[..4], (5514u32 | 0x8000_0000).to_le_bytes());

        let packet = PacketMessage::from_raw_bytes(&logon).unwrap();
        assert_eq!(packet.emsg(), EMsg::ClientLogon);
        let header = match packet.header() {
            MessageHeaderWrapper::Proto(header) => header,
            header => panic!("expected a protobuf header, got {:?}", header),
        };
        assert_eq!(header.steamid(), 0x01A0_0000_0000_0000);
        assert!(header.has_client_sessionid());
        assert_eq!(header.client_sessionid(), 0);
        assert_eq!(packet.header().source(), u64::MAX);

        let body = CMsgClientLogon::parse_from_bytes(packet.payload()).unwrap();
        assert_eq!(body.protocol_version(), 65581);
        assert_eq!(body.client_os_type(), OS_TYPE as i32 as u32);
        assert_eq!(body.client_language(), "english");
        assert!(body.has_cell_id());
        assert!(!body.has_account_name());
        assert!(!body.has_access_token());
    }

    #[test]
    fn stamps_the_session_on_later_messages() {
        let session = Session {
            steam_id: SteamID::from_steam64(0x01A0_0000_1234_5678),
            session_id: -42,
        };
        let heartbeat = proto_message(EMsg::ClientHeartBeat, session.header(), CMsgClientHeartBeat::new());

        let packet = PacketMessage::from_raw_bytes(&heartbeat).unwrap();
        assert_eq!(packet.emsg(), EMsg::ClientHeartBeat);
        match packet.header() {
            MessageHeaderWrapper::Proto(header) => {
                assert_eq!(header.steamid(), 0x01A0_0000_1234_5678);
                assert_eq!(header.client_sessionid(), -42);
            }
            header => panic!("expected a protobuf header, got {:?}", header),
        }
    }

    /// `ClientLogOnResponse` to an anonymous logon, captured from a CM (decrypted, after unpacking its `Multi`).
    ///
    /// The public IP (in `public_ip` and `deprecated_public_ip`) was replaced with 203.0.113.7, a documentation
    /// address, and `ip_country_code` with `US`. The SteamID and session id are the ones Steam assigned to this
    /// anonymous session.
    const ANONYMOUS_LOGON_RESPONSE: [u8; 99] = [
        0xEF, 0x02, 0x00, 0x80, 0x14, 0x00, 0x00, 0x00, 0x09, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA, 0xA0, 0x01, 0x10,
        0xE3, 0x82, 0xD5, 0xBD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x08, 0x01, 0x10, 0x09, 0x18, 0x09, 0x20, 0x87,
        0xE2, 0x81, 0xD8, 0x0C, 0x2D, 0x7B, 0xFB, 0xD2, 0x6A, 0x30, 0x00, 0x38, 0x19, 0x60, 0xFF, 0xFF, 0xFF, 0xFF,
        0x07, 0x72, 0x00, 0x7A, 0x05, 0x0D, 0x07, 0x71, 0x00, 0xCB, 0xA1, 0x01, 0x10, 0xBD, 0x80, 0xBD, 0xE3, 0xCA,
        0xA0, 0x01, 0xAA, 0x01, 0x02, 0x55, 0x53, 0xC0, 0x01, 0x00, 0xC8, 0x01, 0x00, 0xD8, 0x01, 0xC4, 0xD2, 0xC7,
        0xD9, 0xD1, 0xF0, 0xA3, 0x8B, 0x05, 0xE0, 0x01, 0x00,
    ];

    #[test]
    fn parses_a_captured_anonymous_logon_response() {
        let packet = PacketMessage::from_raw_bytes(&ANONYMOUS_LOGON_RESPONSE).unwrap();
        assert_eq!(packet.emsg(), EMsg::ClientLogOnResponse);

        let (session, response) = logon_result(packet).unwrap();
        assert_eq!(response.eresult(), EResult::OK as i32);
        assert_eq!(session.steam_id.to_steam64(), 117316669797350672);
        assert_eq!(session.steam_id.account_type(), Some(EAccountType::AnonUser));
        assert_eq!(session.steam_id.universe(), Some(EUniverse::Public));
        assert_eq!(session.session_id, -139116189);
        assert_eq!(heartbeat_interval(&response), Duration::from_secs(9));
        assert_eq!(response.cell_id(), 25);
        assert_eq!(response.public_ip.v4(), u32::from(Ipv4Addr::new(203, 0, 113, 7)));
        assert_eq!(response.client_supplied_steamid(), 117316669797350672);
    }

    #[test]
    fn failed_logons_are_errors() {
        let mut response = CMsgClientLogonResponse::new();
        response.set_eresult(EResult::ServiceUnavailable as i32);
        response.set_eresult_extended(EResult::TryAnotherCM as i32);
        let packet = proto_message(EMsg::ClientLogOnResponse, CMsgProtoBufHeader::new(), response);

        assert!(matches!(
            logon_result(PacketMessage::from_raw_bytes(&packet).unwrap()),
            Err(ClientError::LogonFailed {
                result: EResult::ServiceUnavailable,
                extended: Some(EResult::TryAnotherCM),
            })
        ));
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
}

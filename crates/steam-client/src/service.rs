//! Service methods ("unified messages"): protobuf requests to a Steam service, such as `Authentication`, answered by a
//! response carrying the request's job id.
//!
//! Before logon they go as `ServiceMethodCallFromClientNonAuthed`, once logged on as `ServiceMethodCallFromClient`
//! with the session on the header. See [`SteamClient::call`](crate::client::SteamClient::call).

use std::marker::PhantomData;

use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::MessageHeaderWrapper;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_AccessToken_GenerateForApp_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_BeginAuthSessionViaCredentials_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_GetPasswordRSAPublicKey_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_PollAuthSessionStatus_Response;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request;
use steam_protobuf::protobufs::steammessages_auth_steamclient::CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_IncomingMessage_Notification;
use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_SendMessage_Request;
use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_SendMessage_Response;
use steam_protobuf::protobufs::steammessages_notifications_steamclient::CSteamNotification_NotificationsReceived_Notification;

use crate::errors::ClientError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::protocol::eresult;
use crate::protocol::proto_message;
use crate::protocol::Exchange;
use crate::protocol::Session;

/// A service method: its name, and the bodies of its request and response. Implemented by a marker type, which
/// names the method in [`SteamClient::call`](crate::client::SteamClient::call) and
/// [`SteamHandle::call`](crate::handle::SteamHandle::call).
///
/// The crate's own are in [`methods`]. Another method needs a marker type of yours (steam-protobuf's request types
/// can't implement a trait of this crate in your crate, Rust's orphan rule; a type of yours can):
///
/// ```
/// use steam_client::service::ServiceMethod;
/// use steam_protobuf::protobufs::steammessages_contentsystem_steamclient::CContentServerDirectory_GetServersForSteamPipe_Request;
/// use steam_protobuf::protobufs::steammessages_contentsystem_steamclient::CContentServerDirectory_GetServersForSteamPipe_Response;
///
/// enum GetServersForSteamPipe {}
///
/// impl ServiceMethod for GetServersForSteamPipe {
///     const METHOD: &'static str = "ContentServerDirectory.GetServersForSteamPipe#1";
///     type Request = CContentServerDirectory_GetServersForSteamPipe_Request;
///     type Response = CContentServerDirectory_GetServersForSteamPipe_Response;
/// }
///
/// # async fn run(
/// #     mut client: steam_client::client::SteamClient<steam_client::client::LoggedOn<steam_client::client::Anonymous>>,
/// # ) -> Result<(), steam_client::errors::ClientError> {
/// let mut request = CContentServerDirectory_GetServersForSteamPipe_Request::new();
/// request.set_cell_id(client.cell_id());
/// let servers = client.call::<GetServersForSteamPipe>(request).await?.servers;
/// # Ok(())
/// # }
/// ```
pub trait ServiceMethod: 'static {
    /// The method, as `Service.Method#1`.
    const METHOD: &'static str;
    /// The request's body.
    type Request: Message;
    /// The response's body.
    type Response: Message;
}

/// The request of a method in [`methods`], which names it: `call_service_method(request)` calls it, with no marker
/// type to write.
pub trait ServiceRequest: Message {
    /// The method this is the request of.
    type Method: ServiceMethod<Request = Self>;
}

/// The response of method `R` requests.
pub type ResponseOf<R> = <<R as ServiceRequest>::Method as ServiceMethod>::Response;

/// The service methods this crate calls, as marker types for [`ServiceMethod`].
pub mod methods {
    use super::*;

    macro_rules! service_methods {
        ($($(#[$doc:meta])* $method:ident: $request:ty => $response:ty, $name:literal;)*) => {
            $(
                $(#[$doc])*
                #[derive(Debug, Clone, Copy)]
                pub enum $method {}

                impl ServiceMethod for $method {
                    const METHOD: &'static str = $name;
                    type Request = $request;
                    type Response = $response;
                }

                impl ServiceRequest for $request {
                    type Method = $method;
                }
            )*
        };
    }

    service_methods! {
        /// `Authentication.GetPasswordRSAPublicKey#1`: the key to encrypt an account's password with.
        GetPasswordRSAPublicKey: CAuthentication_GetPasswordRSAPublicKey_Request
            => CAuthentication_GetPasswordRSAPublicKey_Response, "Authentication.GetPasswordRSAPublicKey#1";
        /// `Authentication.BeginAuthSessionViaCredentials#1`: begins authenticating with a password.
        BeginAuthSessionViaCredentials: CAuthentication_BeginAuthSessionViaCredentials_Request
            => CAuthentication_BeginAuthSessionViaCredentials_Response,
            "Authentication.BeginAuthSessionViaCredentials#1";
        /// `Authentication.UpdateAuthSessionWithSteamGuardCode#1`: answers Steam Guard with a code.
        UpdateAuthSessionWithSteamGuardCode: CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request
            => CAuthentication_UpdateAuthSessionWithSteamGuardCode_Response,
            "Authentication.UpdateAuthSessionWithSteamGuardCode#1";
        /// `Authentication.PollAuthSessionStatus#1`: the tokens, once the authentication is confirmed.
        PollAuthSessionStatus: CAuthentication_PollAuthSessionStatus_Request
            => CAuthentication_PollAuthSessionStatus_Response, "Authentication.PollAuthSessionStatus#1";
        /// `Authentication.GenerateAccessTokenForApp#1`: an access token, and maybe a new refresh token.
        GenerateAccessTokenForApp: CAuthentication_AccessToken_GenerateForApp_Request
            => CAuthentication_AccessToken_GenerateForApp_Response, "Authentication.GenerateAccessTokenForApp#1";
        /// `FriendMessages.SendMessage#1`: sends a friend a chat message.
        SendMessage: CFriendMessages_SendMessage_Request => CFriendMessages_SendMessage_Response,
            "FriendMessages.SendMessage#1";
    }
}

/// A call Steam makes on the client, a "notification": an [`EMsg::ServiceMethod`] with no response, its method on the
/// header's `target_job_name` (as SteamKit's `SteamUnifiedMessages` reads them). Implemented by a marker type, as
/// [`ServiceMethod`] is; read it with [`Event::notification`](crate::events::Event::notification).
///
/// The crate's own are in [`notifications`]; another needs a marker type of yours, as for [`ServiceMethod`].
pub trait ServiceNotification: 'static {
    /// The method, as `Service.Method#1`.
    const METHOD: &'static str;
    /// The notification's body.
    type Notification: Message;
}

/// The notifications this crate knows, as marker types for [`ServiceNotification`].
pub mod notifications {
    use super::*;

    /// `FriendMessagesClient.IncomingMessage#1`: a friend's chat message, or that they are typing.
    #[derive(Debug, Clone, Copy)]
    pub enum IncomingMessage {}

    impl ServiceNotification for IncomingMessage {
        const METHOD: &'static str = "FriendMessagesClient.IncomingMessage#1";
        type Notification = CFriendMessages_IncomingMessage_Notification;
    }

    /// `SteamNotificationClient.NotificationsReceived#1`: Steam's notifications, such as a new trade offer
    /// (`k_ESteamNotificationType_TradeOffer`), with their details as JSON in `body_data`.
    #[derive(Debug, Clone, Copy)]
    pub enum NotificationsReceived {}

    impl ServiceNotification for NotificationsReceived {
        const METHOD: &'static str = "SteamNotificationClient.NotificationsReceived#1";
        type Notification = CSteamNotification_NotificationsReceived_Notification;
    }
}

/// Where a call comes from: before logon, or in a session, whose header it carries.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Caller {
    NonAuthed,
    Authed(Session),
}

impl Caller {
    pub(crate) fn emsg(&self) -> EMsg {
        match self {
            Self::NonAuthed => EMsg::ServiceMethodCallFromClientNonAuthed,
            Self::Authed(_) => EMsg::ServiceMethodCallFromClient,
        }
    }

    pub(crate) fn header(&self) -> CMsgProtoBufHeader {
        match self {
            Self::NonAuthed => CMsgProtoBufHeader::new(),
            Self::Authed(session) => session.header(),
        }
    }
}

/// Service method `M`, as the protocol calls it.
pub(crate) struct Service<M>(PhantomData<fn() -> M>);

impl<M: ServiceMethod> Exchange for Service<M> {
    type Request = M::Request;
    type Response = M::Response;
    const RESPONSE: EMsg = EMsg::ServiceMethodResponse;

    fn request(caller: &Caller, job_id: u64) -> (EMsg, CMsgProtoBufHeader) {
        (caller.emsg(), request_header::<M>(caller, job_id))
    }

    fn response(packet: PacketMessage) -> Result<M::Response, ClientError> {
        response::<M>(packet)
    }
}

/// The header of a call of `M`, as job `job_id`.
pub(crate) fn request_header<M: ServiceMethod>(caller: &Caller, job_id: u64) -> CMsgProtoBufHeader {
    let mut header = caller.header();
    header.set_jobid_source(job_id);
    header.set_target_job_name(M::METHOD.to_string());
    header
}

/// The serialized call of `M`, as job `job_id`.
pub(crate) fn request<M: ServiceMethod>(caller: &Caller, job_id: u64, request: M::Request) -> Vec<u8> {
    proto_message(caller.emsg(), request_header::<M>(caller, job_id), request)
}

/// Checks that a request can be sent: that its required fields are set. Steam's own messages have none, but
/// messages of games' protocols may.
pub(crate) fn check<M: Message>(request: &M) -> Result<(), ClientError> {
    request
        .check_initialized()
        .map_err(|error| ClientError::Message(error.into()))
}

/// Checks the result on the response's header, and parses its body.
pub(crate) fn response<M: ServiceMethod>(packet: PacketMessage) -> Result<M::Response, ClientError> {
    let header = match packet.header() {
        MessageHeaderWrapper::Proto(header) => header,
        _ => {
            return Err(ClientError::ServiceMethod {
                method: M::METHOD,
                result: EResult::Invalid,
                message: "the response has no protobuf header".to_string(),
            })
        }
    };
    // an absent result is a failure: the header's default is `Fail`
    let result = eresult(header.eresult());
    if result != EResult::OK {
        return Err(ClientError::ServiceMethod {
            method: M::METHOD,
            result,
            message: header.error_message().to_string(),
        });
    }
    Ok(ClientMessage::<M::Response>::from_proto_packet(packet)?.body)
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use steam_language_gen::HasJobId;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHeartBeat;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientLoggedOff;

    use super::*;
    use crate::connection::tests::connection;
    use crate::connection::tests::soon;
    use crate::events::Event;
    use crate::service::methods::GetPasswordRSAPublicKey;

    fn rsa_key_request() -> CAuthentication_GetPasswordRSAPublicKey_Request {
        let mut request = CAuthentication_GetPasswordRSAPublicKey_Request::new();
        request.set_account_name("account".to_string());
        request
    }

    #[test]
    fn writes_non_authed_calls() {
        let raw = request::<GetPasswordRSAPublicKey>(&Caller::NonAuthed, 7, rsa_key_request());
        let packet = PacketMessage::from_bytes(Bytes::from(raw)).unwrap();
        assert_eq!(packet.emsg(), EMsg::ServiceMethodCallFromClientNonAuthed);
        match packet.header() {
            MessageHeaderWrapper::Proto(header) => {
                assert_eq!(header.jobid_source(), 7);
                assert_eq!(header.target_job_name(), "Authentication.GetPasswordRSAPublicKey#1");
                assert!(!header.has_steamid());
                assert!(!header.has_client_sessionid());
            }
            header => panic!("expected a protobuf header, got {:?}", header),
        }
    }

    #[test]
    fn writes_authed_calls_with_the_session() {
        let session = Session {
            steam_id: steamid_parser::SteamID::from_steam64(76561197960287930),
            session_id: -3,
        };

        let raw = request::<GetPasswordRSAPublicKey>(&Caller::Authed(session), 8, rsa_key_request());
        let packet = PacketMessage::from_bytes(Bytes::from(raw)).unwrap();
        assert_eq!(packet.emsg(), EMsg::ServiceMethodCallFromClient);
        match packet.header() {
            MessageHeaderWrapper::Proto(header) => {
                assert_eq!(header.jobid_source(), 8);
                assert_eq!(header.steamid(), 76561197960287930);
                assert_eq!(header.client_sessionid(), -3);
            }
            header => panic!("expected a protobuf header, got {:?}", header),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_the_response_to_its_job_and_keeps_the_rest() {
        let (mut connection, mut cm) = connection();

        let server = tokio::spawn(async move {
            let (call, body) = cm
                .expect::<CAuthentication_GetPasswordRSAPublicKey_Request>(EMsg::ServiceMethodCallFromClientNonAuthed)
                .await;
            assert_eq!(call.header.jobid_source(), 1);
            assert_eq!(body.account_name(), "account");

            // another job's response, and an unrelated message, come first
            let mut other = CAuthentication_GetPasswordRSAPublicKey_Response::new();
            other.set_publickey_mod("other".to_string());
            let mut header = CMsgProtoBufHeader::new();
            header.set_jobid_target(99);
            header.set_eresult(1);
            cm.send(proto_message(EMsg::ServiceMethodResponse, header, other)).await;
            cm.send(proto_message(
                EMsg::ClientHeartBeat,
                CMsgProtoBufHeader::new(),
                CMsgClientHeartBeat::new(),
            ))
            .await;

            let mut response = CAuthentication_GetPasswordRSAPublicKey_Response::new();
            response.set_publickey_mod("ours".to_string());
            cm.reply(&call, EResult::OK, response).await;
        });

        let response = connection
            .call::<GetPasswordRSAPublicKey>(rsa_key_request())
            .await
            .unwrap();
        assert_eq!(response.publickey_mod(), "ours");
        soon(server).await.unwrap();

        // what came meanwhile is kept, in order
        match soon(connection.next_event()).await.unwrap() {
            Event::Message(packet) => {
                assert_eq!(packet.emsg(), EMsg::ServiceMethodResponse);
                assert_eq!(packet.parse().unwrap().header().target(), 99);
            }
            event => panic!("expected the other response, got {:?}", event),
        }
        match soon(connection.next_event()).await.unwrap() {
            Event::Message(packet) => assert_eq!(packet.emsg(), EMsg::ClientHeartBeat),
            event => panic!("expected a heartbeat, got {:?}", event),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn failed_calls_are_errors() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            let call = cm.read().await;
            let mut header = CMsgProtoBufHeader::new();
            header.set_jobid_target(call.header.jobid_source());
            header.set_eresult(EResult::RateLimitExceeded as i32);
            header.set_error_message("slow down".to_string());
            cm.send(proto_message(
                EMsg::ServiceMethodResponse,
                header,
                CAuthentication_GetPasswordRSAPublicKey_Response::new(),
            ))
            .await;
        });

        let error = connection
            .call::<GetPasswordRSAPublicKey>(rsa_key_request())
            .await
            .unwrap_err();
        match error {
            ClientError::ServiceMethod {
                method,
                result,
                message,
            } => {
                assert_eq!(method, "Authentication.GetPasswordRSAPublicKey#1");
                assert_eq!(result, EResult::RateLimitExceeded);
                assert_eq!(message, "slow down");
            }
            error => panic!("expected a service method error, got {:?}", error),
        }
        soon(server).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn being_logged_off_while_waiting_is_an_error() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            cm.read().await;
            let mut logged_off = CMsgClientLoggedOff::new();
            logged_off.set_eresult(EResult::LogonSessionReplaced as i32);
            cm.send(proto_message(
                EMsg::ClientLoggedOff,
                CMsgProtoBufHeader::new(),
                logged_off,
            ))
            .await;
        });

        let error = connection
            .call::<GetPasswordRSAPublicKey>(rsa_key_request())
            .await
            .unwrap_err();
        assert!(
            matches!(error, ClientError::LoggedOff(EResult::LogonSessionReplaced)),
            "{:?}",
            error
        );
        soon(server).await.unwrap();
    }

    fn response_packet(header: MessageHeaderWrapper) -> PacketMessage {
        let mut raw = vec![];
        header.write(EMsg::ServiceMethodResponse, &mut raw);
        raw.extend(
            CAuthentication_GetPasswordRSAPublicKey_Response::new()
                .write_to_bytes()
                .unwrap(),
        );
        PacketMessage::from_bytes(Bytes::from(raw)).unwrap()
    }

    #[test]
    fn a_response_without_a_result_failed() {
        let packet = response_packet(MessageHeaderWrapper::Proto(CMsgProtoBufHeader::new()));
        assert!(matches!(
            response::<GetPasswordRSAPublicKey>(packet),
            Err(ClientError::ServiceMethod {
                result: EResult::Fail,
                ..
            })
        ));
    }

    #[test]
    fn a_response_without_a_protobuf_header_is_invalid() {
        use steam_language_gen::generated::headers::ExtendedMessageHeader;
        use steam_language_gen::MessageHeaderExt;

        let packet = response_packet(MessageHeaderWrapper::Ext(ExtendedMessageHeader::create()));
        assert!(matches!(
            response::<GetPasswordRSAPublicKey>(packet),
            Err(ClientError::ServiceMethod {
                result: EResult::Invalid,
                ..
            })
        ));
    }
}

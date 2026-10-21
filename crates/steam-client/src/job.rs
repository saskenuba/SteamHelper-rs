//! Job messages: requests to Steam that aren't service methods, each answered by one message of its own type
//! carrying the request's job id, such as `ClientRequestFreeLicense`, answered by `ClientRequestFreeLicenseResponse`.
//!
//! Only messages with exactly one answer fit. Not those answered by several (PICS product info, with
//! `response_pending` on all but the last), by pushes (`ClientRequestFriendData`) or not at all
//! (`ClientGamesPlayed`). What Steam pushes because of a request, such as the `ClientLicenseList` after a free
//! license is granted, is an [event](crate::events::Event), not part of the answer.
//!
//! See [`SteamClient::call_job`](crate::client::SteamClient::call_job).

use std::marker::PhantomData;

use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicense;
use steam_protobuf::protobufs::steammessages_clientserver_2::CMsgClientRequestFreeLicenseResponse;

use crate::errors::ClientError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::protocol::eresult;
use crate::protocol::Exchange;
use crate::service::Caller;

/// A job message: the request, and the message answering it. Implemented by a marker type, which names it in
/// [`SteamClient::call_job`](crate::client::SteamClient::call_job) and
/// [`SteamHandle::call_job`](crate::handle::SteamHandle::call_job).
///
/// The crate's own are in [`messages`]. Another needs a marker type of yours, as for
/// [`ServiceMethod`](crate::service::ServiceMethod):
///
/// ```
/// use steam_client::job::JobMessage;
/// use steam_language_gen::generated::enums::EMsg;
/// use steam_language_gen::generated::enums::EResult;
/// use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGetAppOwnershipTicket;
/// use steam_protobuf::protobufs::steammessages_clientserver::CMsgClientGetAppOwnershipTicketResponse;
///
/// enum GetAppOwnershipTicket {}
///
/// impl JobMessage for GetAppOwnershipTicket {
///     const REQUEST: EMsg = EMsg::ClientGetAppOwnershipTicket;
///     const RESPONSE: EMsg = EMsg::ClientGetAppOwnershipTicketResponse;
///     type Request = CMsgClientGetAppOwnershipTicket;
///     type Response = CMsgClientGetAppOwnershipTicketResponse;
///
///     fn result(response: &Self::Response) -> Option<i32> {
///         Some(response.eresult() as i32)
///     }
/// }
///
/// # async fn run(
/// #     mut client: steam_client::client::SteamClient<steam_client::client::LoggedOn<steam_client::client::User>>,
/// # ) -> Result<(), steam_client::errors::ClientError> {
/// let mut request = CMsgClientGetAppOwnershipTicket::new();
/// request.set_app_id(440);
/// let ticket = client.call_job::<GetAppOwnershipTicket>(request).await?.ticket;
/// # Ok(())
/// # }
/// ```
pub trait JobMessage: 'static {
    /// The request's message.
    const REQUEST: EMsg;
    /// The message answering it.
    const RESPONSE: EMsg;
    /// The request's body.
    type Request: Message;
    /// The response's body.
    type Response: Message;

    /// The result Steam gave in the response's body, as the `EResult`'s number, for messages that carry one (often
    /// an `eresult` field): the call fails unless it is `OK` (1). `None`, the default, checks nothing.
    fn result(_response: &Self::Response) -> Option<i32> {
        None
    }
}

/// The request of a job message in [`messages`], which names it: `call_job_message(request)` sends it, with no
/// marker type to write.
pub trait JobRequest: Message {
    /// The job message this is the request of.
    type Job: JobMessage<Request = Self>;
}

/// The response to `R` requests.
pub type JobResponseOf<R> = <<R as JobRequest>::Job as JobMessage>::Response;

/// The job messages this crate knows, as marker types for [`JobMessage`].
pub mod messages {
    use super::*;

    /// `ClientRequestFreeLicense`: the free licenses of `appids`, such as free-to-play games, answered by
    /// `ClientRequestFreeLicenseResponse` with the apps and packages granted. An app that isn't free, or that the
    /// account already owns, is not granted: Steam still answers `OK`, without it. Steam sends the new license list
    /// (`ClientLicenseList`) as an event after a grant.
    #[derive(Debug, Clone, Copy)]
    pub enum RequestFreeLicense {}

    impl JobMessage for RequestFreeLicense {
        const REQUEST: EMsg = EMsg::ClientRequestFreeLicense;
        const RESPONSE: EMsg = EMsg::ClientRequestFreeLicenseResponse;
        type Request = CMsgClientRequestFreeLicense;
        type Response = CMsgClientRequestFreeLicenseResponse;

        fn result(response: &Self::Response) -> Option<i32> {
            // absent is `Fail`, the field's default
            Some(response.eresult() as i32)
        }
    }

    impl JobRequest for CMsgClientRequestFreeLicense {
        type Job = RequestFreeLicense;
    }
}

/// Job message `J`, as the protocol sends it.
pub(crate) struct Job<J>(PhantomData<fn() -> J>);

impl<J: JobMessage> Exchange for Job<J> {
    type Request = J::Request;
    type Response = J::Response;
    const RESPONSE: EMsg = J::RESPONSE;

    fn request(caller: &Caller, job_id: u64) -> (EMsg, CMsgProtoBufHeader) {
        let mut header = caller.header();
        header.set_jobid_source(job_id);
        (J::REQUEST, header)
    }

    fn response(packet: PacketMessage) -> Result<J::Response, ClientError> {
        let response = ClientMessage::<J::Response>::from_proto_packet(packet)?.body;
        match J::result(&response).map(eresult) {
            Some(result) if result != EResult::OK => Err(ClientError::JobMessage {
                request: J::REQUEST,
                result,
            }),
            _ => Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use steam_language_gen::MessageHeaderWrapper;

    use super::messages::RequestFreeLicense;
    use super::*;
    use crate::connection::tests::connection;
    use crate::connection::tests::soon;
    use crate::protocol::proto_message;
    use crate::protocol::Session;

    #[test]
    fn writes_the_request_with_its_job_and_the_session() {
        let session = Session {
            steam_id: steamid_parser::SteamID::from_steam64(76561197960287930),
            session_id: -3,
        };
        let (emsg, header) = Job::<RequestFreeLicense>::request(&Caller::Authed(session), 9);
        assert_eq!(emsg, EMsg::ClientRequestFreeLicense);
        assert_eq!(header.jobid_source(), 9);
        assert!(!header.has_target_job_name());
        assert_eq!(header.steamid(), 76561197960287930);
        assert_eq!(header.client_sessionid(), -3);
    }

    fn response(result: Option<EResult>) -> PacketMessage {
        let mut response = CMsgClientRequestFreeLicenseResponse::new();
        if let Some(result) = result {
            response.set_eresult(result as u32);
        }
        response.granted_appids = vec![440];
        let raw = proto_message(
            EMsg::ClientRequestFreeLicenseResponse,
            CMsgProtoBufHeader::new(),
            response,
        );
        PacketMessage::from_bytes(Bytes::from(raw)).unwrap()
    }

    #[test]
    fn checks_the_result_in_the_body() {
        let granted = Job::<RequestFreeLicense>::response(response(Some(EResult::OK))).unwrap();
        assert_eq!(granted.granted_appids, [440]);

        match Job::<RequestFreeLicense>::response(response(Some(EResult::RateLimitExceeded))) {
            Err(ClientError::JobMessage { request, result }) => {
                assert_eq!(request, EMsg::ClientRequestFreeLicense);
                assert_eq!(result, EResult::RateLimitExceeded);
            }
            other => panic!("expected a job message error, got {:?}", other),
        }
        // absent is `Fail`
        assert!(matches!(
            Job::<RequestFreeLicense>::response(response(None)),
            Err(ClientError::JobMessage {
                result: EResult::Fail,
                ..
            })
        ));
    }

    #[test]
    fn checks_nothing_without_a_result() {
        enum NoResult {}

        impl JobMessage for NoResult {
            const REQUEST: EMsg = EMsg::ClientRequestFreeLicense;
            const RESPONSE: EMsg = EMsg::ClientRequestFreeLicenseResponse;
            type Request = CMsgClientRequestFreeLicense;
            type Response = CMsgClientRequestFreeLicenseResponse;
        }

        let response = Job::<NoResult>::response(response(None)).unwrap();
        assert_eq!(response.granted_appids, [440]);
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_its_answer_or_times_out() {
        let (mut connection, mut cm) = connection();
        let server = tokio::spawn(async move {
            let call = cm.read().await;
            assert_eq!(call.emsg, EMsg::ClientRequestFreeLicense);
            let mut response = CMsgClientRequestFreeLicenseResponse::new();
            response.set_eresult(EResult::OK as u32);
            response.granted_packageids = vec![1];
            let mut header = CMsgProtoBufHeader::new();
            header.set_jobid_target(call.header.jobid_source());
            cm.send(proto_message(EMsg::ClientRequestFreeLicenseResponse, header, response))
                .await;
            // never answered
            cm.read().await;
            cm
        });

        let mut request = CMsgClientRequestFreeLicense::new();
        request.appids = vec![440];
        let granted = connection
            .call_job::<RequestFreeLicense>(request.clone())
            .await
            .unwrap();
        assert_eq!(granted.granted_packageids, [1]);
        assert!(matches!(
            connection.call_job::<RequestFreeLicense>(request).await,
            Err(ClientError::Timeout(EMsg::ClientRequestFreeLicenseResponse))
        ));
        soon(server).await.unwrap();
    }

    #[test]
    fn a_response_without_a_protobuf_header_is_an_error() {
        use steam_language_gen::generated::headers::ExtendedMessageHeader;
        use steam_language_gen::MessageHeaderExt;

        let mut raw = vec![];
        MessageHeaderWrapper::Ext(ExtendedMessageHeader::create())
            .write(EMsg::ClientRequestFreeLicenseResponse, &mut raw);
        let packet = PacketMessage::from_bytes(Bytes::from(raw)).unwrap();
        assert!(Job::<RequestFreeLicense>::response(packet).is_err());
    }
}

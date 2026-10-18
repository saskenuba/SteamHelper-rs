//! What Steam sends that no call waits for: messages it pushes, such as friend messages and notifications, and what
//! happens to the connection.
//!
//! Read them with [`Events::recv`](crate::handle::Events::recv) from a spawned client, or with
//! [`SteamClient::next_event`](crate::client::SteamClient::next_event) without one.

use std::fmt;

use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;
use steam_protobuf::protobuf::Message;

use crate::errors::ClientError;
use crate::messages::message::ClientMessage;
use crate::messages::packet::PacketMessage;
use crate::service::ServiceNotification;

/// Something that happened that no call waited for.
///
/// A spawned client never drops `LoggedOff`, `Disconnected` or `Reconnected`, nor the count of a `Lagged`: only
/// messages go when they aren't read in time (see [`Events`](crate::handle::Events)).
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// A message from Steam: one it pushed, or a response to a call nobody waits for anymore.
    Message(RawMessage),
    /// Steam ended the session: it logged the client off, or (spawned) refused to log it on again.
    ///
    /// For a spawned client, this is for good, and the last event: when Steam logs off in a way worth retrying, such
    /// as `TryAnotherCM`, the client connects again instead, and the event is [`Event::Disconnected`].
    LoggedOff(EResult),
    /// This many messages were dropped here because they weren't read in time. The events after it are the newer
    /// ones.
    Lagged(u64),
    /// A spawned client lost its connection, or its session, and connects again. Calls waiting failed with
    /// [`ClientError::Dropped`], and so do calls made until [`Event::Reconnected`].
    Disconnected {
        /// Why.
        reason: DisconnectReason,
    },
    /// A spawned client logged on again after [`Event::Disconnected`].
    Reconnected,
}

/// Why a spawned client connects again: see [`Event::Disconnected`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The connection closed or failed.
    ConnectionLost,
    /// Steam logged the client off in a way worth retrying, such as `TryAnotherCM` or `ServiceUnavailable`.
    LoggedOff(EResult),
    /// Calls kept timing out, so the connection was taken for lost (one Steam drops silently, with no TCP reset, can
    /// otherwise take minutes to fail).
    Unresponsive,
}

impl Event {
    /// The notification of `N` this event carries, if it is one. See [`RawMessage::notification`].
    pub fn notification<N: ServiceNotification>(&self) -> Option<Result<N::Notification, ClientError>> {
        match self {
            Self::Message(message) => message.notification::<N>(),
            _ => None,
        }
    }
}

/// A message from Steam, its body not parsed yet. Holds a slice of what was read, not a copy.
#[derive(Clone)]
pub struct RawMessage {
    emsg: EMsg,
    /// The whole message: its header, then its body.
    packet: Bytes,
}

impl RawMessage {
    pub(crate) fn new(emsg: EMsg, packet: Bytes) -> Self {
        Self { emsg, packet }
    }

    /// The kind of message.
    pub fn emsg(&self) -> EMsg {
        self.emsg
    }

    /// Parses the protobuf body, as `M`.
    pub fn parse_proto<M: Message>(&self) -> Result<M, ClientError> {
        Ok(ClientMessage::<M>::from_proto_packet(self.parse()?)?.body)
    }

    /// The service method Steam called, as `Service.Method#1`, if this is a service notification
    /// ([`EMsg::ServiceMethod`]).
    pub fn service_method(&self) -> Option<String> {
        if self.emsg != EMsg::ServiceMethod {
            return None;
        }
        match self.parse().ok()?.header() {
            MessageHeaderWrapper::Proto(header) => Some(header.target_job_name().to_string()),
            _ => None,
        }
    }

    /// The notification of `N`, if this is one: Steam calling `N::METHOD` on the client, as an
    /// [`EMsg::ServiceMethod`] with the method on its header.
    ///
    /// ```no_run
    /// # fn run(message: steam_client::events::RawMessage) {
    /// use steam_client::service::notifications::IncomingMessage;
    ///
    /// if let Some(Ok(message)) = message.notification::<IncomingMessage>() {
    ///     println!("{} says {}", message.steamid_friend(), message.message());
    /// }
    /// # }
    /// ```
    pub fn notification<N: ServiceNotification>(&self) -> Option<Result<N::Notification, ClientError>> {
        if self.service_method()? != N::METHOD {
            return None;
        }
        Some(self.parse_proto())
    }

    /// The message with its header parsed.
    pub(crate) fn parse(&self) -> Result<PacketMessage, MessageError> {
        PacketMessage::from_bytes(self.packet.clone())
    }
}

impl fmt::Debug for RawMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawMessage")
            .field("emsg", &self.emsg)
            .field("len", &self.packet.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::protobufs::steammessages_clientserver_login::CMsgClientHello;
    use steam_protobuf::protobufs::steammessages_friendmessages_steamclient::CFriendMessages_IncomingMessage_Notification;

    use super::*;
    use crate::protocol::proto_message;
    use crate::service::notifications::IncomingMessage;
    use crate::service::notifications::NotificationsReceived;

    fn message(emsg: EMsg, header: CMsgProtoBufHeader, body: impl Message) -> Event {
        Event::Message(RawMessage::new(emsg, proto_message(emsg, header, body).into()))
    }

    /// A friend's message, as Steam calls `FriendMessagesClient.IncomingMessage` on the client.
    fn incoming_message(method: &str) -> Event {
        let mut header = CMsgProtoBufHeader::new();
        header.set_target_job_name(method.to_string());
        let mut notification = CFriendMessages_IncomingMessage_Notification::new();
        notification.set_steamid_friend(76561197960287930);
        notification.set_message("hi".to_string());
        message(EMsg::ServiceMethod, header, notification)
    }

    #[test]
    fn reads_notifications_of_their_method_only() {
        let event = incoming_message("FriendMessagesClient.IncomingMessage#1");
        let notification = event.notification::<IncomingMessage>().unwrap().unwrap();
        assert_eq!(notification.steamid_friend(), 76561197960287930);
        assert_eq!(notification.message(), "hi");
        assert!(event.notification::<NotificationsReceived>().is_none());
        match &event {
            Event::Message(message) => {
                assert_eq!(message.emsg(), EMsg::ServiceMethod);
                assert_eq!(
                    message.service_method().as_deref(),
                    Some("FriendMessagesClient.IncomingMessage#1")
                );
            }
            event => panic!("expected a message, got {:?}", event),
        }

        // another version is another method
        assert!(incoming_message("FriendMessagesClient.IncomingMessage#2")
            .notification::<IncomingMessage>()
            .is_none());
    }

    #[test]
    fn other_messages_are_no_notifications() {
        let mut header = CMsgProtoBufHeader::new();
        header.set_target_job_name("FriendMessagesClient.IncomingMessage#1".to_string());
        let hello = message(EMsg::ClientHello, header, CMsgClientHello::new());
        assert!(hello.notification::<IncomingMessage>().is_none());
        assert!(Event::LoggedOff(EResult::OK)
            .notification::<IncomingMessage>()
            .is_none());

        // a service method with an extended header has no method name
        let extended = RawMessage::new(EMsg::ServiceMethod, Bytes::from_static(&[0xF6, 0x05, 0, 0]));
        assert!(extended.service_method().is_none());
    }

    #[test]
    fn a_body_that_cant_be_parsed_is_an_error() {
        let mut header = CMsgProtoBufHeader::new();
        header.set_target_job_name("FriendMessagesClient.IncomingMessage#1".to_string());
        let mut packet = proto_message(EMsg::ServiceMethod, header, CMsgClientHello::new());
        packet.extend_from_slice(&[0xFF, 0xFF]);
        let event = Event::Message(RawMessage::new(EMsg::ServiceMethod, packet.into()));
        assert!(matches!(event.notification::<IncomingMessage>(), Some(Err(_))));
    }

    #[test]
    fn debug_shows_the_kind_and_length() {
        let event = incoming_message("FriendMessagesClient.IncomingMessage#1");
        let debug = format!("{:?}", event);
        assert!(
            debug.starts_with("Message(RawMessage { emsg: ServiceMethod, len: "),
            "{}",
            debug
        );
    }
}

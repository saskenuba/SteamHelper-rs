use bytes::{Buf, BytesMut};
use steam_language_gen::{
    generated::{
        enums::EMsg,
        headers::{ExtendedMessageHeader, StandardMessageHeader},
    },
    DeserializableBytes, HasJobId, MessageHeaderExt, MessageHeaderWrapper, SerializableBytes,
};
use steam_protobuf::{steam::steammessages_base::CMsgProtoBufHeader, Message};

use crate::messages::MessageKind;

/// Represents a simple unified interface into client messages received directly from the socket.
/// This is contrasted with [IClientMsg] in that this interface is packet body agnostic
/// and allows simple access into its header and underlying data.
///
/// Messages built by `PacketMessage` should be abstracted from the user.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PacketMessage {
    emsg: EMsg,
    header: MessageHeaderWrapper,
    data: Vec<u8>,
}

impl MessageKind for PacketMessage {
    /// Returns underlying message data.
    fn payload(&self) -> &[u8] {
        &self.data
    }
}

impl PacketMessage {
    /// Returns (source_job_id, target_job_id)
    pub fn jobs_ids(&self) -> (u64, u64) {
        (self.header.source(), self.header.target())
    }

    /// Returns underlying EMsg.
    pub fn emsg(&self) -> EMsg {
        self.emsg
    }

    /// Returns the underlying MessageHeaderWrapper.
    ///
    /// Internally cloned. Very cheap.
    pub fn header(&self) -> MessageHeaderWrapper {
        self.header.clone()
    }

    /// This classify the socket message as:
    /// - Standard message (EncryptRequest, EncryptResponse, EncryptResult)
    /// - Protobuf message
    /// - Extended message (extended header)
    /// We need to recover TargetJobID and SourceJobID from every header, that is why we have the
    /// PacketMsg on SteamKit. They are the same but exists for each header type.
    /// [raw_message_bytes] are the raw message bytes coming after Steam's identifier bytes.
    ///
    /// Reference: https://github.com/SteamRE/SteamKit/blob/58562fcc6f6972181615a6d1ff98103b06f0e33f/SteamKit2/SteamKit2/Steam/CMClient.cs#L448
    pub fn from_raw_bytes(raw_message_bytes: &[u8]) -> PacketMessage {
        let emsg = EMsg::from_raw_message(raw_message_bytes).unwrap();
        let raw_data = EMsg::strip_message(raw_message_bytes);

        let (header, body) = match emsg {
            EMsg::ChannelEncryptRequest | EMsg::ChannelEncryptResponse | EMsg::ChannelEncryptResult => {
                let (header, body) = StandardMessageHeader::split_from_bytes(raw_data);
                let header = StandardMessageHeader::from_bytes(header);
                println!("Found a Standard Header.");
                println!("Header bytes: {:?} Body bytes: {:?}", header, body);
                (MessageHeaderWrapper::Std(header), body)
            }
            // We can only check with the raw bytes, with the EMsg still inside
            _ if EMsg::is_protobuf(raw_message_bytes) => {
                println!("Found a Protobuf Header.");
                let (header, body) = CMsgProtoBufHeader::split_from_bytes(raw_data);
                let header = CMsgProtoBufHeader::parse_from_bytes(header).unwrap();
                (MessageHeaderWrapper::Proto(header), body)
            }
            _ => {
                let (header, body) = ExtendedMessageHeader::split_from_bytes(raw_data);
                let header = ExtendedMessageHeader::from_bytes(header);
                println!("Found a Extended Header.");
                (MessageHeaderWrapper::Ext(header), body)
            }
        };

        println!("Packet Message is: {:?}, {:?}, {:?}", &emsg, &header, body);

        PacketMessage {
            emsg,
            header,
            data: body.to_vec(),
        }
    }
}

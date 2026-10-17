use bytes::Buf;
use bytes::Bytes;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::HasJobId;
use steam_language_gen::MessageError;
use steam_language_gen::MessageHeaderWrapper;

use crate::messages::MessageKind;

/// Represents a simple unified interface into client messages received directly from the socket.
/// This is contrasted with [IClientMsg] in that this interface is packet body agnostic
/// and allows simple access into its header and underlying data.
///
/// Messages built by `PacketMessage` should be abstracted from the user.
///
/// The body is a slice of the received frame, not a copy.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PacketMessage {
    emsg: EMsg,
    header: MessageHeaderWrapper,
    body: Bytes,
}

impl MessageKind for PacketMessage {
    /// Returns underlying message data.
    fn payload(&self) -> &[u8] {
        &self.body
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
    pub fn header(&self) -> &MessageHeaderWrapper {
        &self.header
    }

    /// Returns the EMsg, the header and the body, which is still a slice of the frame.
    pub fn into_parts(self) -> (EMsg, MessageHeaderWrapper, Bytes) {
        (self.emsg, self.header, self.body)
    }

    /// This classify the socket message as:
    /// - Standard message (EncryptRequest, EncryptResponse, EncryptResult)
    /// - Protobuf message
    /// - Extended message (extended header)
    ///
    /// We need to recover TargetJobID and SourceJobID from every header, that is why we have the
    /// PacketMsg on SteamKit. They are the same but exists for each header type.
    /// [raw] are the raw message bytes, starting with Steam's identifier bytes. The body keeps pointing into them.
    ///
    /// Reference: https://github.com/SteamRE/SteamKit/blob/58562fcc6f6972181615a6d1ff98103b06f0e33f/SteamKit2/SteamKit2/Steam/CMClient.cs#L448
    pub fn from_bytes(mut raw: Bytes) -> Result<PacketMessage, MessageError> {
        let (emsg, header, body) = MessageHeaderWrapper::parse(&raw)?;
        let header_len = raw.len() - body.len();
        raw.advance(header_len);

        Ok(PacketMessage {
            emsg,
            header,
            body: raw,
        })
    }

    /// Like [`PacketMessage::from_bytes`], copying `raw`.
    #[cfg(test)]
    pub fn from_raw_bytes(raw: &[u8]) -> Result<PacketMessage, MessageError> {
        Self::from_bytes(Bytes::copy_from_slice(raw))
    }
}

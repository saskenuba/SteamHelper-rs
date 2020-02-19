use steam_language_gen::{MessageHeader, MessageHeaderExt, SerializableBytes, DeserializableBytes};
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::headers::{ExtendedMessageHeader, StandardMessageHeader};

/// Represents a simple unified interface into client messages received from the network.
/// This is contrasted with [IClientMsg] in that this interface is packet body agnostic
/// and only allows simple access into the header. This interface is also immutable, and the underlying
/// data cannot be modified.
#[derive(Debug)]
pub(crate) struct PacketMessage<'a> {
    emsg: EMsg,
    target_job_id: u64,
    source_job_id: u64,
    data: &'a [u8],
}

impl<'a> PacketMessage<'a> {
    /// Returns underlying message data.
    pub(crate) fn data(&self) -> &'a [u8] {
        &self.data
    }
    /// Returns (source_job_id, target_job_id)
    pub(crate) fn jobs_ids(&self) -> (&u64, &u64) {
        (&self.source_job_id, &self.target_job_id)
    }
    /// Returns underlying EMsg.
    pub(crate) fn emsg(&self) -> &EMsg { &self.emsg }

    /// This classify the message as:
    /// - Standard message (EncryptRequest, EncryptResponse, EncryptResult)
    /// - Protobuf message
    /// - Extended message (extended header)
    /// We need to recover TargetJobID and SourceJobID from every header, that is why we have the
    /// PacketMsg on SteamKit. They are the same but for each header type.
    /// [raw_message_data] are the bytes after the magic bytes received from connection stream.
    /// This _should_ be used by the main client to classify the messages from the raw bytes.
    /// Reference: https://github.com/SteamRE/SteamKit/blob/58562fcc6f6972181615a6d1ff98103b06f0e33f/SteamKit2/SteamKit2/Steam/CMClient.cs#L448
    pub(crate) fn from_rawdata(raw_message_bytes: &[u8]) -> PacketMessage {
        // should do error checking in case emsg not valid
        let extracted_emsg = EMsg::from_raw_message(raw_message_bytes).unwrap();
        let raw_message_data = EMsg::strip_message(raw_message_bytes);
        let target_job_id;
        let source_job_id;
        let body_bytes;

        match extracted_emsg {
            EMsg::ChannelEncryptRequest | EMsg::ChannelEncryptResponse | EMsg::ChannelEncryptResult => {
                debug!("Found a Standard Header.");
                let (header, body) = StandardMessageHeader::split_from_bytes(raw_message_data);
                trace!("Header bytes: {:?} Body bytes: {:?}", header, body);
                let header = StandardMessageHeader::from_bytes(header);
                target_job_id = header.target_job_id;
                source_job_id = header.source_job_id;
                body_bytes = body;
            }
            _ => {
                if EMsg::is_protobuf(raw_message_data) {
                    debug!("Found a Protobuf Header.");
                    unimplemented!();
                } else {
                    debug!("Found a Extended Header.");
                    let (header, body) = ExtendedMessageHeader::split_from_bytes(raw_message_data);
                    let header = ExtendedMessageHeader::from_bytes(header);
                    target_job_id = header.target_job_id;
                    source_job_id = header.source_job_id;
                    body_bytes = body;
                }
            }
        }

        let packet_message = PacketMessage {
            emsg: extracted_emsg,
            target_job_id,
            source_job_id,
            data: body_bytes,
        };
        trace!("Packet Message is: {:?}", packet_message);

        packet_message
    }
}

#[cfg(test)]
mod tests {
    use protobuf::Message;

    use steam_protobuf::steam::steammessages_clientserver_login::CMsgClientHeartBeat;

    #[test]
    fn test_proto() {
        let oi = CMsgClientHeartBeat::new();
        let teste = oi.write_to_bytes().unwrap();
        // println!("protobuf: {:#?}", teste);
    }
}


// [7:29 PM] Globi::<!>: @Martin an other trait object based approach would be to have an additional
// trait with your desired behavior that you would implement generically for every Message<B, H>
// with the serialize bounds, and return a Box<dyn MessageTrait> from the function


// Globi::<!>: Otherwise for the trait object approach you can define
//type DynamicBody = Box<dyn SerializableMessageBody>;
//type DynamicHeader = Box<dyn SerializableMessageHeader>;
//impl SerializableMessageBody for DynamicBody { ... }
//impl SerializableMessageHeader for DynamicHeader { ... }
// and use Message<DynamicBody, DynamicHeader> as your return type

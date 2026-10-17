use derive_new::new;
use serde::Deserialize;
use serde::Serialize;
use steam_language_gen_derive::MsgHeader;
use steam_protobuf::protobuf::Message;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;

use crate::emsg::EMsg;
use crate::HasJobId;
use crate::MessageError;
use crate::MessageHeaderExt;
use crate::SerializableBytes;

// add protobuf
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MessageHeaders {
    Standard,
    Extended,
}

// add protobuf matching
impl MessageHeaders {
    pub fn header_from_emsg(emsg: EMsg) -> Option<MessageHeaders> {
        match emsg {
            EMsg::ChannelEncryptRequest | EMsg::ChannelEncryptResponse | EMsg::ChannelEncryptResult => {
                Some(MessageHeaders::Standard)
            }
            _ => Some(MessageHeaders::Extended),
        }
    }
}

#[derive(new, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, MsgHeader)]
pub struct StandardMessageHeader {
    #[new(value = "u64::MAX")]
    pub target_job_id: u64,
    #[new(value = "u64::MAX")]
    pub source_job_id: u64,
}

impl MessageHeaderExt for CMsgProtoBufHeader {
    fn create() -> Self {
        Self::new()
    }

    /// The header is prefixed by its length, as a u32.
    fn parse(data: &[u8]) -> Result<(Self, &[u8]), MessageError> {
        let too_short = |needed| MessageError::TooShort {
            needed,
            got: data.len(),
        };

        if data.len() < 4 {
            return Err(too_short(4));
        }
        let (length, rest) = data.split_at(4);
        let length = u32::from_le_bytes([length[0], length[1], length[2], length[3]]) as usize;
        if rest.len() < length {
            return Err(too_short(4 + length));
        }

        let (header, rest) = rest.split_at(length);
        Ok((Self::parse_from_bytes(header)?, rest))
    }
}

impl HasJobId for CMsgProtoBufHeader {
    fn set_target(&mut self, new_target: u64) {
        self.set_jobid_target(new_target);
    }

    fn set_source(&mut self, new_source: u64) {
        self.set_jobid_source(new_source);
    }

    fn source(&self) -> u64 {
        self.jobid_source()
    }

    fn target(&self) -> u64 {
        self.jobid_target()
    }
}

#[derive(new, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, MsgHeader)]
pub struct ExtendedMessageHeader {
    /// Size of the header with its `EMsg`.
    #[new(value = "36")]
    header_size: u8,
    #[new(value = "2")]
    header_version: u16,
    #[new(value = "u64::MAX")]
    pub target_job_id: u64,
    #[new(value = "u64::MAX")]
    pub source_job_id: u64,
    #[new(value = "239")]
    header_canary: u8,
    #[new(value = "0")]
    steam_id: u64,
    #[new(value = "0")]
    session_id: i32,
}

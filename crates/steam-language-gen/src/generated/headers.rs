use derive_new::new;
use serde::Deserialize;
use serde::Serialize;
use steam_language_gen_derive::MsgHeader;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;

use crate::generated::enums::EMsg;
use crate::DeserializableBytes;
use crate::HasJobId;
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
    #[new(value = "std::u64::MAX")]
    pub target_job_id: u64,
    #[new(value = "std::u64::MAX")]
    pub source_job_id: u64,
}

impl MessageHeaderExt for CMsgProtoBufHeader {
    fn create() -> Self {
        Self::new()
    }

    fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]) {
        let size = std::mem::size_of::<Self>();
        (&data[..size], &data[size..])
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
    #[new(value = "32")]
    header_size: u8,
    #[new(value = "2")]
    header_version: u16,
    #[new(value = "std::u64::MAX")]
    pub target_job_id: u64,
    #[new(value = "std::u64::MAX")]
    pub source_job_id: u64,
    #[new(value = "239")]
    header_canary: u8,
    #[new(value = "0")]
    steam_id: u64,
    #[new(value = "0")]
    session_id: i32,
}

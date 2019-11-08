#[macro_use]
use num::FromPrimitive;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use steam_protobuf::steam::steammessages_base::CMsgProtoBufHeader;

use super::enums::{EMsg, EResult, EUniverse};

pub(crate) trait SerializableMessageHeader {
    fn new() -> Self;
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(packet_data: &[u8]) -> Self;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct StandardMessageHeader {
    emsg: EMsg,
    target_job_id: u64,
    source_job_id: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExtendedMessageHeader {
    emsg: EMsg,
    header_size: u8,
    header_version: u16,
    target_job_id: u64,
    source_job_id: u64,
    header_canary: u8,
    steam_id: u64,
    session_id: i32,
}

struct GCProtobufMessageHeader {
    emsg: EMsg,
    header_length: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct GCMessageHeader {
    header_version: u16,
    target_job_id: u64,
    source_job_id: u64,
}

#[derive(Debug)]
pub(crate) struct ProtobufMessageHeader {
    header_length: i32,
    proto_header: CMsgProtoBufHeader,
}

impl SerializableMessageHeader for ProtobufMessageHeader {
    fn new() -> Self {
        let mut proto_message = Self { header_length: 0, proto_header: Default::default() };

        proto_message.header_length = proto_message.proto_header.get_cached_size() as i32;
        proto_message
    }

    fn serialize(&self) -> Vec<u8> {
        unimplemented!()
    }

    fn deserialize(data: &[u8]) -> Self {
        unimplemented!()
    }
}

impl Default for ExtendedMessageHeader {
    fn default() -> Self {
        Self {
            emsg: EMsg::Invalid,
            header_size: 36,
            header_version: 2,
            target_job_id: std::u64::MAX,
            source_job_id: std::u64::MAX,
            header_canary: 239,
            steam_id: 0,
            session_id: 0,
        }
    }
}

impl SerializableMessageHeader for ExtendedMessageHeader {
    fn new() -> Self {
        Default::default()
    }

    fn serialize(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }

    fn deserialize(packet_data: &[u8]) -> Self {
        let decoded: Self = bincode::deserialize(packet_data).unwrap();
        decoded
    }
}

impl Default for StandardMessageHeader {
    fn default() -> Self {
        Self { emsg: EMsg::Invalid, target_job_id: std::u64::MAX, source_job_id: std::u64::MAX }
    }
}

impl SerializableMessageHeader for StandardMessageHeader {
    fn new() -> Self {
        Default::default()
    }

    fn serialize(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }

    fn deserialize(packet_data: &[u8]) -> Self {
        unimplemented!()
    }
}

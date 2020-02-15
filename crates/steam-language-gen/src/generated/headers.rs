use serde::{Deserialize, Serialize};

use steam_protobuf::steam::steammessages_base::CMsgProtoBufHeader;

pub trait SerializableMessageHeader {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(packet_data: &[u8]) -> Self where Self: Sized;
    fn strip_as_bytes(data: &[u8]) -> (&[u8], &[u8]);
}

#[derive(new, Debug, Serialize, Deserialize, PartialEq, Eq, MsgHeader)]
pub struct StandardMessageHeader {
    #[new(value = "std::u64::MAX")]
    pub target_job_id: u64,
    #[new(value = "std::u64::MAX")]
    pub source_job_id: u64,
}


#[derive(new, Debug, Serialize, Deserialize, PartialEq, Eq, MsgHeader)]
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

struct GCProtobufMessageHeader {
    header_length: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct GCMessageHeader {
    header_version: u16,
    target_job_id: u64,
    source_job_id: u64,
}

#[derive(Debug)]
pub struct ProtobufMessageHeader {
    header_length: i32,
    proto_header: CMsgProtoBufHeader,
}

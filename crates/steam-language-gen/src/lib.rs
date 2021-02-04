#![allow(dead_code, non_upper_case_globals, non_camel_case_types)]

#[macro_use]
extern crate steam_language_gen_derive;

use enum_dispatch::enum_dispatch;
pub use num_traits::FromPrimitive;
use serde::Serialize;

use crate::generated::headers::{ExtendedMessageHeader, StandardMessageHeader};
use steam_protobuf::steam::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::Message;

pub mod generated;
#[cfg(feature = "generator")]
pub mod generator;
#[cfg(feature = "generator")]
pub mod parser;

#[enum_dispatch(HasJobId)]
#[derive(Clone, Debug, PartialEq, Serialize)]
/// This wraps our headers so we can be generic over them over a Msg type.
pub enum MessageHeaderWrapper {
    Std(StandardMessageHeader),
    Proto(CMsgProtoBufHeader),
    Ext(ExtendedMessageHeader),
}

impl MessageHeaderWrapper {
    pub fn proto_header(&self) -> Option<&CMsgProtoBufHeader> {
        match self {
            MessageHeaderWrapper::Proto(me) => Some(me),
            _ => None,
        }
    }
}

/// Every implementation has to implement bincode::serialize and deserialize
pub trait SerializableBytes: Send {
    fn to_bytes(&self) -> Vec<u8>;
}

impl<T> SerializableBytes for T
where
    T: Message,
{
    fn to_bytes(&self) -> Vec<u8> {
        self.write_to_bytes().unwrap()
    }
}

impl<T> DeserializableBytes for T
where
    T: Message,
{
    fn from_bytes(packet_data: &[u8]) -> Self {
        T::parse_from_bytes(packet_data).unwrap()
    }
}

impl<T> MessageBodyExt for T
where
    T: Message,
{
    fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]) {
        let size = std::mem::size_of::<Self>();
        (&data[..size], &data[size..])
    }
}

/// delegate serialization to inner type
impl SerializableBytes for MessageHeaderWrapper {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            MessageHeaderWrapper::Std(hdr) => hdr.to_bytes(),
            MessageHeaderWrapper::Ext(hdr) => hdr.to_bytes(),
            MessageHeaderWrapper::Proto(hdr) => hdr.write_to_bytes().expect("Error writing protobuf"),
        }
    }
}

pub trait DeserializableBytes {
    fn from_bytes(packet_data: &[u8]) -> Self;
}

#[enum_dispatch]
pub trait HasJobId {
    fn set_target(&mut self, new_target: u64);
    fn set_source(&mut self, new_source: u64);
    fn source(&self) -> u64;
    fn target(&self) -> u64;
}

// facilities around headers
pub trait MessageHeaderExt {
    fn create() -> Self;
    /// Returns header on the left, rest on the right
    fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]);
}

pub trait MessageBodyExt {
    fn split_from_bytes(data: &[u8]) -> (&[u8], &[u8]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    File,
    Head,
    Type,
    Member,
}

#[derive(PartialEq, Eq)]
pub struct Token<'a> {
    value: String,
    default: Option<&'a str>,
}

impl<'a> Token<'a> {
    fn get_value(&self) -> &str {
        &self.value
    }
    fn get_default(&self) -> Option<&'a str> {
        self.default
    }
}

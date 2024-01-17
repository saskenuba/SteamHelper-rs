#![allow(dead_code, non_upper_case_globals, non_camel_case_types)]

#[macro_use]
extern crate steam_language_gen_derive;

use enum_dispatch::enum_dispatch;
use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
use steam_protobuf::ProtobufSerialize;

use crate::generated::headers::ExtendedMessageHeader;
use crate::generated::headers::StandardMessageHeader;

pub mod generated;
#[cfg(feature = "generator")]
pub mod generator;
#[cfg(feature = "generator")]
pub mod parser;

/// This wraps our headers so we can be generic over them over a Msg type.
#[enum_dispatch(HasJobId)]
#[derive(Clone, Debug, PartialEq)]
pub enum MessageHeaderWrapper {
    Std(StandardMessageHeader),
    Proto(CMsgProtoBufHeader),
    Ext(ExtendedMessageHeader),
}

/// Every implementation has to implement bincode::serialize and deserialize
pub trait SerializableBytes: Send {
    fn to_bytes(&self) -> Vec<u8>;
}

impl<T> SerializableBytes for T
where
    T: ProtobufSerialize,
{
    fn to_bytes(&self) -> Vec<u8> {
        self.to_bytes().unwrap()
    }
}

/// delegate serialization to inner type
impl SerializableBytes for MessageHeaderWrapper {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            MessageHeaderWrapper::Std(hdr) => hdr.to_bytes(),
            MessageHeaderWrapper::Ext(hdr) => hdr.to_bytes(),
            MessageHeaderWrapper::Proto(hdr) => SerializableBytes::to_bytes(hdr),
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
    /// Returns header on the left, rest on the right
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

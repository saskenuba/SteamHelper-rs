use arrayref::array_ref;
use num_traits::FromPrimitive;

use crate::generated::enums::EMsg;

pub mod messages;
pub mod enums;
pub mod headers;


const PROTOMASK: u32 = 0x8000_0000;
const EMSGMASK: u32 = !PROTOMASK;

impl EMsg {
    /// Creates an `EMsg` from a uint32
    pub fn from_uint(message: u32) -> Self {
        match EMsg::from_u32(message) {
            Some(value) => value,
            None => panic!("ABORT"),
        }
    }

    /// Creates an `EMsg` from a raw data packet.
    pub fn from_raw_message(message: &[u8]) -> Result<Self, EMsgError> {
        // an error should be throw if the message doesnt have 4 bytes of length
        let extracted_varint: u32 = Self::extract_varint(message);

        match EMsg::from_u32(extracted_varint) {
            Some(value) => Ok(value),
            None => Err(EMsgError::ValueNotFound("A value was not found.")),
        }
    }

    /// Strips protobuf message flag out and and returns it
    pub fn strip_protobuf_flag(message: u32) -> u32 {
        message & PROTOMASK
    }

    /// Strips Emsg and returns
    pub fn strip_message(message: &[u8]) -> &[u8] {
        &message[4..]
    }

    /// Checks if a message is flagged as a protobuf
    /// We can only check with the varint on it
    pub fn is_protobuf(message: &[u8]) -> bool {
        Self::strip_protobuf_flag(Self::extract_varint(message)) > 0
    }

    /// Extract varint from data
    pub fn extract_varint(message: &[u8]) -> u32 {
        u32::from_le_bytes(*array_ref!(message, 0, 4))
    }
}

#[derive(Debug)]
pub enum EMsgError {
    MessageNotLongEnough(&'static str),
    ValueNotFound(&'static str),
}


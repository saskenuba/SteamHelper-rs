//! Steam's protocol language: enums, message ids and messages, generated from
//! [SteamKit](https://github.com/SteamRE/SteamKit)'s SteamLanguage files.
//!
//! Each part is behind a feature, so a crate only builds what it uses:
//!
//! | Feature | Contents | Extra dependencies |
//! |---|---|---|
//! | none | [`enums`]: `EResult`, `EAccountType`, `EUniverse`, `ETradeOfferState`, ... | none |
//! | `emsg` | `emsg`: `EMsg` message ids, and helpers to read them from packets | `arrayref` |
//! | `messages` | `headers`, `messages` and their traits. Enables `emsg` | `steam-protobuf`, `bincode` |
//!
//! The `messages` feature is **unstable**: it is not covered by semver and can change in minor releases.
//! [`enums`] and `emsg` follow semver.

#![allow(dead_code, non_upper_case_globals, non_camel_case_types)]

#[cfg(feature = "emsg")]
pub mod emsg;
pub mod enums;
#[cfg(feature = "messages")]
pub mod headers;
#[cfg(feature = "messages")]
pub mod messages;
#[cfg(all(test, feature = "messages"))]
mod tests;

#[cfg(feature = "messages")]
pub use traits::*;

#[cfg(feature = "messages")]
mod traits {
    use enum_dispatch::enum_dispatch;
    use steam_protobuf::protobuf;
    use steam_protobuf::protobuf::Message;
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::ProtobufSerialize;

    use crate::emsg::EMsg;
    use crate::emsg::EMsgError;
    use crate::emsg::PROTOMASK;
    use crate::headers::ExtendedMessageHeader;
    use crate::headers::MessageHeaders;
    use crate::headers::StandardMessageHeader;

    /// Why a message could not be parsed.
    #[derive(Debug, thiserror::Error)]
    #[non_exhaustive]
    pub enum MessageError {
        #[error(transparent)]
        EMsg(#[from] EMsgError),
        #[error("expected at least {needed} bytes, got {got}")]
        TooShort { needed: usize, got: usize },
        #[error(transparent)]
        Bincode(#[from] bincode::Error),
        #[error(transparent)]
        Protobuf(#[from] protobuf::Error),
    }

    /// This wraps our headers so we can be generic over them over a Msg type.
    #[enum_dispatch(HasJobId)]
    #[derive(Clone, Debug, PartialEq)]
    pub enum MessageHeaderWrapper {
        Std(StandardMessageHeader),
        Proto(CMsgProtoBufHeader),
        Ext(ExtendedMessageHeader),
    }

    impl MessageHeaderWrapper {
        /// Parses a raw packet: its `EMsg`, then the header its kind uses. Returns them with the body.
        ///
        /// `ChannelEncrypt*` messages use the standard header, messages flagged as protobuf a
        /// [`CMsgProtoBufHeader`], and every other message the extended header.
        pub fn parse(packet: &[u8]) -> Result<(EMsg, Self, &[u8]), MessageError> {
            let (emsg, is_protobuf, data) = EMsg::split_raw(packet)?;

            let (header, body) = match MessageHeaders::header_from_emsg(emsg) {
                Some(MessageHeaders::Standard) => {
                    let (header, body) = StandardMessageHeader::parse(data)?;
                    (Self::Std(header), body)
                }
                _ if is_protobuf => {
                    let (header, body) = CMsgProtoBufHeader::parse(data)?;
                    (Self::Proto(header), body)
                }
                _ => {
                    let (header, body) = ExtendedMessageHeader::parse(data)?;
                    (Self::Ext(header), body)
                }
            };
            Ok((emsg, header, body))
        }

        /// Writes `emsg` and this header as they go on the wire.
        ///
        /// A protobuf header flags the `EMsg` as protobuf, and is prefixed by its length as a u32.
        pub fn write(&self, emsg: EMsg, out: &mut Vec<u8>) {
            match self {
                Self::Std(header) => {
                    out.extend_from_slice(&(emsg as u32).to_le_bytes());
                    out.extend(header.to_bytes());
                }
                Self::Ext(header) => {
                    out.extend_from_slice(&(emsg as u32).to_le_bytes());
                    out.extend(header.to_bytes());
                }
                Self::Proto(header) => {
                    // only fails on missing required fields, and the header has none
                    let header = header
                        .write_to_bytes()
                        .expect("CMsgProtoBufHeader has no required fields");
                    out.extend_from_slice(&(emsg as u32 | PROTOMASK).to_le_bytes());
                    out.extend_from_slice(&(header.len() as u32).to_le_bytes());
                    out.extend(header);
                }
            }
        }
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

    #[enum_dispatch]
    pub trait HasJobId {
        fn set_target(&mut self, new_target: u64);
        fn set_source(&mut self, new_source: u64);
        fn source(&self) -> u64;
        fn target(&self) -> u64;
    }

    // facilities around headers
    pub trait MessageHeaderExt: Sized {
        fn create() -> Self;
        /// Parses the header at the start of `data`, which comes after the `EMsg`. Returns it with the rest.
        fn parse(data: &[u8]) -> Result<(Self, &[u8]), MessageError>;
    }

    pub trait MessageBodyExt: Sized {
        /// Parses the body at the start of `data`, which comes after the header. Returns it with the rest, its
        /// payload.
        fn parse(data: &[u8]) -> Result<(Self, &[u8]), MessageError>;
    }
}

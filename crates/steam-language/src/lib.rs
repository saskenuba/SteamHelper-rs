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

#[cfg(feature = "messages")]
pub use traits::*;

#[cfg(feature = "messages")]
mod traits {
    use enum_dispatch::enum_dispatch;
    use steam_protobuf::protobufs::steammessages_base::CMsgProtoBufHeader;
    use steam_protobuf::ProtobufSerialize;

    use crate::headers::ExtendedMessageHeader;
    use crate::headers::StandardMessageHeader;

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
}

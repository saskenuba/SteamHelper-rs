//! Generator of Steam's protocol language from SteamKit's SteamLanguage files.
//!
//! The generated enums, message ids and messages now live in [`steam_language`]. They are re-exported here under
//! their previous paths.

#![allow(dead_code, non_upper_case_globals, non_camel_case_types)]

pub use steam_language::{
    DeserializableBytes, HasJobId, MessageBodyExt, MessageHeaderExt, MessageHeaderWrapper, SerializableBytes,
};

pub mod generated;
#[cfg(feature = "generator")]
pub mod generator;
#[cfg(feature = "generator")]
pub mod parser;

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

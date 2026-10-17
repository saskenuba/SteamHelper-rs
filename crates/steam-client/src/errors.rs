//! Errors of the CM connection and of the client on top of it.

use std::io;

use steam_crypto::CryptoError;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::MessageError;
use thiserror::Error;

/// Errors of [`SteamClient`](crate::client::SteamClient).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientError {
    /// The CM server list could not be fetched.
    #[error("Failed to fetch the CM server list: {0}")]
    ServerList(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// None of the CM servers tried accepted a connection.
    #[error("Could not connect to any CM server.")]
    NoServerAvailable,

    /// The CM server closed the connection.
    #[error("Connection with Steam CM server was dropped.")]
    Dropped,

    /// Steam did not answer in time.
    #[error("Timed out waiting for {0:?}.")]
    Timeout(EMsg),

    /// Steam sent another message than the one the protocol expects at this point.
    #[error("Expected {expected:?}, got {got:?}.")]
    UnexpectedMessage {
        /// The message the protocol expects.
        expected: EMsg,
        /// The message received.
        got: EMsg,
    },

    /// The CM server is in a universe whose public key we don't have.
    #[error("Universe {0:?} is not supported, only the public universe is.")]
    UnsupportedUniverse(EUniverse),

    /// Steam refused the channel encryption.
    #[error("Steam refused the channel encryption: {0:?}.")]
    EncryptionFailed(EResult),

    /// Steam refused the logon.
    #[error("Steam refused the logon: {result:?} (extended: {extended:?}).")]
    LogonFailed {
        /// Why the logon failed.
        result: EResult,
        /// More detail, when Steam sends it.
        extended: Option<EResult>,
    },

    /// Steam logged the client off.
    #[error("Steam logged the client off: {0:?}.")]
    LoggedOff(EResult),

    /// A message Steam sent could not be parsed.
    #[error(transparent)]
    Message(#[from] MessageError),

    /// The session key could not be generated or encrypted.
    #[error(transparent)]
    Crypto(#[from] CryptoError),

    /// The connection failed below the message layer.
    #[error(transparent)]
    Packet(#[from] PacketError),
}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        Self::Packet(error.into())
    }
}

/// Errors framing, encrypting or decrypting the packets on the socket.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PacketError {
    /// A frame did not start with the `VT01` magic.
    #[error("Received a malformed packet from the socket.")]
    Malformed,

    /// An encrypted frame could not be decrypted. The connection can't be used anymore.
    #[error("Failed to decrypt a packet: {0}")]
    Decrypt(#[from] CryptoError),

    /// Reading or writing the socket failed.
    #[error(transparent)]
    IoError(#[from] io::Error),
}

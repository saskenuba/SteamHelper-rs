//! Errors of the CM connection and of the client on top of it.

use std::io;

use steam_cryptography::CryptoError;
use steam_language_gen::generated::enums::EMsg;
use steam_language_gen::generated::enums::EResult;
use steam_language_gen::generated::enums::EUniverse;
use steam_language_gen::MessageError;
use steam_totp::error::TotpError;
use steamid_parser::SteamID;
use thiserror::Error;

use crate::auth::Confirmation;
#[cfg(feature = "cs2")]
use crate::gc::cs2::InspectLinkError;
#[cfg(feature = "cs2")]
use crate::gc::cs2::ItemDetailsError;

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

    /// A spawned client stopped: it logged off, or Steam ended its session for good (its last events say why).
    #[error("The client stopped: it logged off, or its session ended for good.")]
    Closed,

    /// A service method failed, such as `Authentication.BeginAuthSessionViaCredentials` with `InvalidPassword`.
    #[error("{method} failed: {result:?} {message}")]
    ServiceMethod {
        /// The method, as `Service.Method#1`.
        method: &'static str,
        /// Why it failed.
        result: EResult,
        /// Steam's explanation, often empty.
        message: String,
    },

    /// Steam Guard asks for a confirmation the logon can't give, such as an email code to a logon with a shared
    /// secret. Begin an [`AuthSession`](crate::auth::AuthSession) to answer it.
    #[error("Steam Guard asks for one of {0:?}, which this logon can't give.")]
    SteamGuard(Vec<Confirmation>),

    /// A token isn't one Steam issues.
    #[error("Invalid token: {0}.")]
    InvalidToken(String),

    /// The password could not be encrypted.
    #[error("Could not encrypt the password: {0}.")]
    PasswordEncryption(&'static str),

    /// A Steam Guard code could not be generated.
    #[error(transparent)]
    Totp(Box<TotpError>),

    /// A game coordinator did not send the message of this type (the game's own numbering) in time, such as CS2's
    /// `ClientWelcome` (4004) or its answer to an inspect link (9157).
    #[error("Timed out waiting for the game coordinator's message {0}.")]
    GcTimeout(u32),

    /// A game coordinator refused the session, such as CS2's `ClientLogonFatalError`.
    #[error("The game coordinator refused the session: {message} (code {code}).")]
    GcRefused {
        /// The game's own error code.
        code: u32,
        /// The game coordinator's explanation, often empty.
        message: String,
    },

    /// A request about a player needs an individual account in the public universe, such as CS2's profiles.
    #[error("{0} is not an individual account.")]
    NotIndividual(SteamID),

    /// A string isn't a CS2 inspect link.
    #[cfg(feature = "cs2")]
    #[error(transparent)]
    InspectLink(#[from] InspectLinkError),

    /// The details of a CS2 item could not be read.
    #[cfg(feature = "cs2")]
    #[error(transparent)]
    ItemDetails(#[from] ItemDetailsError),

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

impl From<TotpError> for ClientError {
    fn from(error: TotpError) -> Self {
        Self::Totp(Box::new(error))
    }
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

#[cfg(test)]
mod tests {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::*;

    #[test]
    fn boxes_the_errors_of_other_crates() {
        let time = UNIX_EPOCH.duration_since(SystemTime::now()).unwrap_err();
        assert!(matches!(ClientError::from(TotpError::Time(time)), ClientError::Totp(_)));
    }
}

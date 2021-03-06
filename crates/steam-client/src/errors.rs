use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Connection with Steam CM server was dropped.")]
    Dropped,

    #[error(transparent)]
    IoError(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum PacketError {
    #[error("Received a malformed packet from the socket.")]
    Malformed,

    #[error(transparent)]
    IoError(#[from] io::Error),
}

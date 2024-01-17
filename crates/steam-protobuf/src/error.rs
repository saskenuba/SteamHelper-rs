use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtobufError {
    #[error("`{0}`")]
    EncodeError(String),
    #[error("`{0}`")]
    DecodeError(String),
}

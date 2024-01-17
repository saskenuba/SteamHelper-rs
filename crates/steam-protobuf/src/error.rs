use protobuf_json_mapping::ParseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtobufError {
    #[error("`{0}`")]
    EncodeError(String),
    #[error(transparent)]
    DecodeError(#[from] ParseError),
}

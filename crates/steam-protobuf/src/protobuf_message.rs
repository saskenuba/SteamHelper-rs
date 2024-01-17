use protobuf::MessageDyn;
use protobuf::MessageFull;

use crate::error::ProtobufError;

pub trait ProtobufSerialize: MessageDyn {
    fn to_bytes(&self) -> Result<Vec<u8>, ProtobufError>;
    fn to_json(&self) -> Result<String, ProtobufError>;
}

pub trait ProtobufDeserialize {
    type Output: MessageFull;

    fn from_json(message: &str) -> Result<Self::Output, ProtobufError>;

    fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self::Output, ProtobufError>;
}

impl<M> ProtobufDeserialize for M
where
    M: MessageFull,
{
    type Output = Self;

    fn from_json(message: &str) -> Result<Self::Output, ProtobufError> {
        protobuf_json_mapping::parse_from_str(message).map_err(|e| ProtobufError::DecodeError(e.to_string()))
    }

    fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self::Output, ProtobufError> {
        M::parse_from_bytes(bytes.as_ref()).map_err(|e| ProtobufError::DecodeError(e.to_string()))
    }
}

impl<T> ProtobufSerialize for T
where
    T: MessageFull,
{
    fn to_bytes(&self) -> Result<Vec<u8>, ProtobufError> {
        self.write_to_bytes()
            .map_err(|e| ProtobufError::EncodeError(e.to_string()))
    }
    fn to_json(&self) -> Result<String, ProtobufError> {
        protobuf_json_mapping::print_to_string(self).map_err(|e| ProtobufError::EncodeError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobufs::steammessages_twofactor_steamclient::CTwoFactor_AddAuthenticator_Request;

    #[test]
    fn serialize_stuff() {
        let mut request = CTwoFactor_AddAuthenticator_Request::new();
        request.set_steamid(79841115858);
        let res = request.to_json().unwrap();
        assert_eq!(res, r#"{"steamid": "79841115858"}"#);
    }
    #[test]
    fn deserialize_stuff() {
        let response = CTwoFactor_AddAuthenticator_Request::from_json(r#"{"steamid": 79841115858}"#).unwrap();
        let mut expected = CTwoFactor_AddAuthenticator_Request::new();
        expected.set_steamid(79841115858);
        assert_eq!(response, expected);
    }
}

use protobuf::Message;

use crate::error::ProtobufError;

pub trait ProtobufSerialize: Message {
    fn to_bytes(&self) -> Result<Vec<u8>, ProtobufError>;
}

pub trait ProtobufDeserialize {
    type Output: Message;

    fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self::Output, ProtobufError>;
}

impl<M> ProtobufDeserialize for M
where
    M: Message,
{
    type Output = Self;

    fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self::Output, ProtobufError> {
        M::parse_from_bytes(bytes.as_ref()).map_err(|e| ProtobufError::DecodeError(e.to_string()))
    }
}

impl<T> ProtobufSerialize for T
where
    T: Message,
{
    fn to_bytes(&self) -> Result<Vec<u8>, ProtobufError> {
        self.write_to_bytes()
            .map_err(|e| ProtobufError::EncodeError(e.to_string()))
    }
}

#[cfg(all(test, feature = "steammessages_twofactor_steamclient"))]
mod tests {
    use super::*;
    use crate::protobufs::steammessages_twofactor_steamclient::CTwoFactor_AddAuthenticator_Request;

    #[test]
    fn roundtrip_bytes() {
        let mut request = CTwoFactor_AddAuthenticator_Request::new();
        request.set_steamid(79841115858);
        let bytes = request.to_bytes().unwrap();
        assert_eq!(bytes, [0x09, 0xd2, 0xbe, 0xe6, 0x96, 0x12, 0x00, 0x00, 0x00]);
        assert_eq!(CTwoFactor_AddAuthenticator_Request::from_bytes(bytes).unwrap(), request);
    }
}

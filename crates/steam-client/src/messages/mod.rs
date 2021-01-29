pub mod codec;
pub mod encoded;
pub mod message;
pub mod packet;

pub(crate) trait MessageKind {
    fn payload(&self) -> &[u8];
}

pub mod codec;
pub mod encoded;
pub mod message;
pub(crate) mod multi;
pub mod packet;

pub(crate) trait MessageKind {
    fn payload(&self) -> &[u8];
}

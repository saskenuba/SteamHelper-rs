use std::any::Any;

use steam_protobuf::Message;

pub mod codec;
pub mod encoded;
pub mod message;
pub mod packet;

pub(crate) trait MessageKind {
    fn set_payload(&mut self, payload: &[u8]);

    fn payload(&self) -> &[u8];
}

pub(crate) trait ProtoMsgBox: Message {
    /// Avoid some typing!
    fn boxed(self) -> Box<Self>;

    /// Box message and upcast it to Any
    fn boxed_any(self) -> Box<dyn Any>;
}

pub(crate) trait ProtoRecover: Any {
    fn recover<T: 'static>(self) -> Box<T>;
}

// FIXME: downcast and unwrap box?
impl ProtoRecover for Box<dyn Any> {
    fn recover<T: 'static>(self) -> Box<T> {
        self.downcast::<T>()
            .expect("This SHOULD NOT fail. Simply because we should know the type we are handling.")
    }
}

impl<T: Message> ProtoMsgBox for T {
    fn boxed(self) -> Box<Self> {
        Box::new(self)
    }

    fn boxed_any(self) -> Box<dyn Any> {
        self.boxed().into_any()
    }
}

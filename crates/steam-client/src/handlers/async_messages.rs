use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};

use futures::task::AtomicWaker;
use pin_project::pin_project;

use crate::errors::MessageError;
use crate::handlers::dispatcher::DispatcherMap;
use crate::messages::{ProtoMsgBox, ProtoRecover};

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
pub(crate) struct AsyncResponseInner<'a, T> {
    #[pin]
    dispatcher: &'a DispatcherMap,
    source_job_id: u64,
    message: PhantomData<T>,
}

impl<'a, T> AsyncResponseInner<'a, T> {
    pub fn new(dispatcher: &'a DispatcherMap) -> AsyncResponseInner<'a, T> {
        let source_job_id = dispatcher.current_job_id.fetch_add(1, Ordering::SeqCst);

        Self {
            dispatcher,
            source_job_id,
            message: Default::default(),
        }
    }
}

// FIXME: Add a timeout
impl<'a, T: 'static> Future for AsyncResponseInner<'a, T> {
    type Output = Result<T, MessageError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let source_job_id = self.source_job_id;

        let this = self.project();
        let dispatcher: Pin<&mut &DispatcherMap> = this.dispatcher;
        let waker = Box::new(AtomicWaker::new());

        waker.register(cx.waker());
        {
            dispatcher.tracked_jobids_wakers.lock().insert(source_job_id, waker);
        }

        let message = {
            match dispatcher.tracked_messages.lock().remove(&source_job_id) {
                None => return Poll::Ready(Err(MessageError::Timeout)),
                Some(message) => message,
            }
        };
        let final_message = message.recover::<T>();

        Poll::Ready(Ok(*final_message))
    }
}

impl<'a, T> AsyncResponseInner<'a, T> {}

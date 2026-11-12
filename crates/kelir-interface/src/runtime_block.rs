// see: cans/02-plugin-model.md#Latency-ceiling
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

// see: cans/02-plugin-model.md#Latency-ceiling
struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

// see: cans/02-plugin-model.md#Latency-ceiling
// TODO (L3) [plugin_engine]: drive the futures on the async runtime once the transport exposes a runtime handle.; slop: futures never driven, transport handle assumed present
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

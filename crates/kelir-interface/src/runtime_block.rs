// see: cans/02-plugin-model.md#Latency-ceiling
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};

// see: cans/02-plugin-model.md#Latency-ceiling
static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();

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
pub fn block_on<F: Future>(future: F) -> F::Output {
    if let Some(runtime) = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
    {
        return runtime.block_on(future);
    }
    park_block_on(future)
}

// see: cans/02-plugin-model.md#Latency-ceiling
fn park_block_on<F: Future>(future: F) -> F::Output {
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

// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Per-ring-timeouts
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use crate::errno_types::Errno;
use crate::plugin_model::PluginError;
use crate::protocol_wire::Action;

// see: cans/02-plugin-model.md#Latency-ceiling
pub const DEFAULT_WORKER_THREADS: usize = 8;
// see: cans/02-plugin-model.md#Per-ring-timeouts
pub const SYNC_HOOK_TIMEOUT_MS: u64 = 5;
// see: cans/02-plugin-model.md#Per-ring-timeouts
pub const ASYNC_POLICY_HOOK_TIMEOUT_MS: u64 = 200;

// see: cans/02-plugin-model.md#Latency-ceiling
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerPoolConfig {
    pub worker_threads: usize,
    pub allow_async_worker_yield: bool,
    pub queue_capacity: usize,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            worker_threads: DEFAULT_WORKER_THREADS,
            allow_async_worker_yield: true,
            queue_capacity: 16384,
        }
    }
}

impl WorkerPoolConfig {
    // see: cans/01-architecture.md#Embedded-profile
    pub fn from_driver_policy(policy: &crate::driver_interface::DriverPolicy) -> Self {
        Self {
            worker_threads: policy.ffi_worker_threads,
            allow_async_worker_yield: policy.ffi_allow_async_worker_yield,
            queue_capacity: policy.ffi_queue_capacity,
        }
    }
}

// see: cans/02-plugin-model.md#Per-ring-timeouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookTimeouts {
    pub sync_ms: u64,
    pub async_policy_ms: u64,
}

impl Default for HookTimeouts {
    fn default() -> Self {
        Self {
            sync_ms: SYNC_HOOK_TIMEOUT_MS,
            async_policy_ms: ASYNC_POLICY_HOOK_TIMEOUT_MS,
        }
    }
}

// see: cans/02-plugin-model.md#Per-ring-timeouts
pub fn hook_timeout_action() -> Action {
    Action::Drop {
        errno: Errno::Ewouldblock,
        reason: "hook timeout".to_string(),
    }
}

// see: cans/02-plugin-model.md#Latency-ceiling
pub struct AsyncWorkerPool {
    sender: Option<mpsc::Sender<Box<dyn FnOnce() + Send + 'static>>>,
    depth: Arc<AtomicUsize>,
    capacity: usize,
}

impl AsyncWorkerPool {
    // see: cans/02-plugin-model.md#Latency-ceiling
    pub fn new(config: &WorkerPoolConfig) -> Self {
        let (sender, receiver) = mpsc::channel::<Box<dyn FnOnce() + Send + 'static>>();
        let receiver = Arc::new(std::sync::Mutex::new(receiver));
        let depth = Arc::new(AtomicUsize::new(0));
        for index in 0..config.worker_threads {
            let receiver = Arc::clone(&receiver);
            let depth = Arc::clone(&depth);
            let _ = std::thread::Builder::new()
                .name(format!("kelir-worker-{index}"))
                .spawn(move || {
                    loop {
                        let task = match receiver.lock() {
                            Ok(guard) => guard.recv(),
                            Err(_) => return,
                        };
                        let Ok(task) = task else { return };
                        task();
                        depth.fetch_sub(1, Ordering::AcqRel);
                    }
                });
        }
        Self {
            sender: Some(sender),
            depth,
            capacity: config.queue_capacity,
        }
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    pub fn submit(&self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), PluginError> {
        let Some(sender) = self.sender.as_ref() else {
            return Err(PluginError::Halt("async worker pool is shut down".into()));
        };
        if self.depth.load(Ordering::Acquire) >= self.capacity {
            return Err(PluginError::FfiQueueSaturated {
                depth: self.depth.load(Ordering::Acquire),
            });
        }
        self.depth.fetch_add(1, Ordering::AcqRel);
        sender.send(task).map_err(|_| {
            self.depth.fetch_sub(1, Ordering::AcqRel);
            PluginError::Halt("async worker pool receiver dropped".into())
        })
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    pub fn depth(&self) -> usize {
        self.depth.load(Ordering::Acquire)
    }

    // see: cans/02-plugin-model.md#Latency-ceiling
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Drop for AsyncWorkerPool {
    fn drop(&mut self) {
        self.sender.take();
    }
}

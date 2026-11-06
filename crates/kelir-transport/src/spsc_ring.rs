// see: cans/01-architecture.md#Transport-engine-abstraction
use std::sync::atomic::AtomicU32;

use crate::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducerCursor(pub u32);

impl ProducerCursor {
    pub fn advance(&mut self, len: u32) {
        self.0 = self.0.wrapping_add(len);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumerCursor(pub u32);

impl ConsumerCursor {
    pub fn advance(&mut self, len: u32) {
        self.0 = self.0.wrapping_add(len);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingState {
    pub producer: ProducerCursor,
    pub consumer: ConsumerCursor,
    pub capacity: u32,
}

impl RingState {
    pub fn new(capacity: u32) -> Self {
        Self {
            producer: ProducerCursor(0),
            consumer: ConsumerCursor(0),
            capacity,
        }
    }

    pub fn used(&self) -> u32 {
        self.producer.0.wrapping_sub(self.consumer.0)
    }

    pub fn free(&self) -> u32 {
        self.capacity.saturating_sub(self.used())
    }
}

pub struct RingHeader {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    pub capacity: u32,
}

pub struct SpscRingTransport {
    pub header: *mut RingHeader,
    pub buffer: *mut u8,
    pub state: RingState,
}

// SAFETY: single-producer/single-consumer ring; header and buffer are mapped once and owned exclusively by the transport pair.
unsafe impl Send for SpscRingTransport {}

impl SpscRingTransport {
    pub fn new(header: *mut RingHeader, buffer: *mut u8, capacity: u32) -> Self {
        Self {
            header,
            buffer,
            state: RingState::new(capacity),
        }
    }
}

impl Transport for SpscRingTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = (buf, &self.state);
        // TODO: memory-fence synchronization + ring drain (L3).
        Err(crate::unsupported(
            "spsc ring read: lock-free SPSC drain with memory-fence synchronization",
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = (buf, &self.state);
        // TODO: memory-fence synchronization + ring enqueue (L3).
        Err(crate::unsupported(
            "spsc ring write: lock-free SPSC enqueue with memory-fence synchronization",
        ))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        let _ = &self.state;
        // TODO: ring flush (L3).
        Err(crate::unsupported("spsc ring flush"))
    }
}

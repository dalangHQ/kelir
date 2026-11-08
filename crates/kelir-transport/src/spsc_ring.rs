// see: cans/01-architecture.md#Transport-engine-abstraction
use std::sync::atomic::{AtomicU32, Ordering};

use crate::Transport;

pub struct RingHeader {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    pub capacity: u32,
}

pub struct SpscRingTransport {
    pub header: *mut RingHeader,
    pub buffer: *mut u8,
}

// SAFETY: single-producer/single-consumer ring; header and buffer are mapped once and owned exclusively by the transport pair.
unsafe impl Send for SpscRingTransport {}

impl SpscRingTransport {
    pub fn new(header: *mut RingHeader, buffer: *mut u8) -> Self {
        Self { header, buffer }
    }

    // SAFETY: header points at the ring header mapped once before sharing; capacity is written once at setup and never mutated after.
    fn header_ref(&self) -> &RingHeader {
        unsafe { &*self.header }
    }

    pub fn used(&self) -> u32 {
        let header = self.header_ref();
        header
            .head
            .load(Ordering::Acquire)
            .wrapping_sub(header.tail.load(Ordering::Acquire))
    }

    pub fn free(&self) -> u32 {
        self.header_ref().capacity.saturating_sub(self.used())
    }
}

// see: cans/01-architecture.md#Transport-engine-abstraction
pub struct OwnedSpscRing {
    header: Box<RingHeader>,
    buffer: Box<[u8]>,
    capacity: u32,
}

impl OwnedSpscRing {
    pub fn new(capacity: u32) -> Self {
        Self {
            header: Box::new(RingHeader {
                head: AtomicU32::new(0),
                tail: AtomicU32::new(0),
                capacity,
            }),
            buffer: vec![0u8; capacity as usize].into_boxed_slice(),
            capacity,
        }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn transport(&mut self) -> SpscRingTransport {
        SpscRingTransport::new(
            self.header.as_mut() as *mut RingHeader,
            self.buffer.as_mut_ptr(),
        )
    }
}

impl Transport for SpscRingTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: memory-fence synchronization + ring drain (L3).
        Err(crate::unsupported(
            "spsc ring read: lock-free SPSC drain with memory-fence synchronization",
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: memory-fence synchronization + ring enqueue (L3).
        Err(crate::unsupported(
            "spsc ring write: lock-free SPSC enqueue with memory-fence synchronization",
        ))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: ring flush (L3).
        Err(crate::unsupported("spsc ring flush"))
    }
}

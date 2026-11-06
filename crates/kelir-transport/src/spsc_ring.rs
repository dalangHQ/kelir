use std::sync::atomic::AtomicU32;

use crate::StreamTransport;

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
}

impl StreamTransport for SpscRingTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: memory-fence synchronization + ring drain (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "spsc ring read (L3)",
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: memory-fence synchronization + ring enqueue (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "spsc ring write (L3)",
        ))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: ring flush (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "spsc ring flush (L3)",
        ))
    }
}

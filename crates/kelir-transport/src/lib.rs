// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/artifacts/diagrams/four-ring-bus.txt#virtio-shm
// see: cans/artifacts/diagrams/wire-plane.txt#USERSPACE-SMOLTCP-INGRESS-RING
#[path = "fallback_transport.flow.rs"]
pub mod fallback_transport;
#[path = "host_bind.run.rs"]
pub mod host_bind;
#[path = "spsc_ring.flow.rs"]
pub mod spsc_ring;

pub trait Transport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;
    fn flush(&mut self) -> Result<(), std::io::Error>;
}

pub use Transport as StreamTransport;

// see: cans/08-lifecycle.md#Detached-lifecycle
#[derive(Debug)]
pub struct MemoryHandoff {
    fd: std::os::unix::io::OwnedFd,
    len: u64,
}

impl MemoryHandoff {
    pub fn new(fd: std::os::unix::io::OwnedFd, len: u64) -> Self {
        Self { fd, len }
    }

    pub fn fd(&self) -> &std::os::unix::io::OwnedFd {
        &self.fd
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub(crate) fn unsupported(op: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("{op} pending (L3)"),
    )
}

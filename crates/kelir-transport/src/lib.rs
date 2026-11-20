// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/artifacts/diagrams/four-ring-bus.txt#virtio-shm
// see: cans/artifacts/diagrams/wire-plane.txt#USERSPACE-SMOLTCP-INGRESS-RING
#[path = "fallback_transport.flow.rs"]
pub mod fallback_transport;
#[path = "host_bind.flow.rs"]
pub mod host_bind;
#[path = "host_bind.run.rs"]
pub mod host_bind_run;
#[path = "memory_handoff.flow.rs"]
pub mod memory_handoff;
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

pub(crate) fn unsupported(op: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("{op} pending (L3)"),
    )
}

// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/artifacts/diagrams/four-ring-bus.txt
// see: cans/artifacts/diagrams/wire-plane.txt
pub mod fallback;
pub mod spsc_ring;

pub trait Transport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;
    fn flush(&mut self) -> Result<(), std::io::Error>;
}

pub use Transport as StreamTransport;

pub(crate) fn unsupported(op: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("{op} pending (L3)"),
    )
}

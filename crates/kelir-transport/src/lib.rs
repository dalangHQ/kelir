// see: cans/01-architecture.md#Transport-engine-abstraction
pub mod fallback;
pub mod spsc_ring;

pub trait StreamTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;
    fn flush(&mut self) -> Result<(), std::io::Error>;
}

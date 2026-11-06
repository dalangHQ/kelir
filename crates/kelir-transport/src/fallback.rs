#![forbid(unsafe_code)]

use crate::StreamTransport;

pub struct VsockFallbackTransport {
    pub fd: i32,
}

impl VsockFallbackTransport {
    pub fn new(fd: i32) -> Self {
        Self { fd }
    }
}

impl StreamTransport for VsockFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: AF_VSOCK read (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "vsock fallback read (L3)",
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: AF_VSOCK write (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "vsock fallback write (L3)",
        ))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: AF_VSOCK flush (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "vsock fallback flush (L3)",
        ))
    }
}

pub struct NamedPipeFallbackTransport {
    pub path: String,
}

impl NamedPipeFallbackTransport {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl StreamTransport for NamedPipeFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: named-pipe read (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "named-pipe fallback read (L3)",
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: named-pipe write (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "named-pipe fallback write (L3)",
        ))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: named-pipe flush (L3).
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "named-pipe fallback flush (L3)",
        ))
    }
}

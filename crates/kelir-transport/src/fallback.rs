#![forbid(unsafe_code)]

// see: cans/01-architecture.md#Transport-engine-abstraction
use crate::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsockAddress {
    pub cid: u32,
    pub port: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsockConfig {
    pub address: VsockAddress,
}

pub struct VsockFallbackTransport {
    pub fd: i32,
}

impl VsockFallbackTransport {
    pub fn new(fd: i32) -> Self {
        Self { fd }
    }

    pub fn connect(config: VsockConfig) -> Result<Self, std::io::Error> {
        // TODO (L3): socket(AF_VSOCK) + connect to config.address.
        let _ = config;
        Err(crate::unsupported("vsock fallback connect"))
    }
}

impl Transport for VsockFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: AF_VSOCK read (L3).
        Err(crate::unsupported("vsock fallback read"))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: AF_VSOCK write (L3).
        Err(crate::unsupported("vsock fallback write"))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: AF_VSOCK flush (L3).
        Err(crate::unsupported("vsock fallback flush"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPipeConfig {
    pub path: String,
}

pub struct NamedPipeFallbackTransport {
    pub path: String,
}

impl NamedPipeFallbackTransport {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn open(config: NamedPipeConfig) -> Result<Self, std::io::Error> {
        // TODO (L3): open the Windows named pipe at config.path.
        let _ = config;
        Err(crate::unsupported("named-pipe fallback open"))
    }
}

impl Transport for NamedPipeFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: named-pipe read (L3).
        Err(crate::unsupported("named-pipe fallback read"))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO: named-pipe write (L3).
        Err(crate::unsupported("named-pipe fallback write"))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO: named-pipe flush (L3).
        Err(crate::unsupported("named-pipe fallback flush"))
    }
}

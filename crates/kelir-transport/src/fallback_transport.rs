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
        // TODO (L3) [pm_13_vsock_fallback_connect_fails_closed]: socket(AF_VSOCK) + connect to config.address; slop: connect Ok on dead CID, half-open transport keeps fd
        let _ = config;
        Err(crate::unsupported("vsock fallback connect"))
    }
}

impl Transport for VsockFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: AF_VSOCK read; slop: read returns ring bytes re-framed, vsock framing diverges
        Err(crate::unsupported("vsock fallback read"))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: AF_VSOCK write; slop: write alters header for socket, SPSC/vsock bytes differ
        Err(crate::unsupported("vsock fallback write"))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: AF_VSOCK flush; slop: flush claims sent, bytes still buffered without carrier
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
        // TODO (L3) [vr_13_transport_named_pipe_windows_parity]: open the Windows named pipe at config.path; slop: pipe open Ok without backpressure, frames drop when full
        let _ = config;
        Err(crate::unsupported("named-pipe fallback open"))
    }
}

impl Transport for NamedPipeFallbackTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO (L3) [vr_13_transport_named_pipe_windows_parity]: named-pipe read; slop: read drops excess frames instead of parking sender
        Err(crate::unsupported("named-pipe fallback read"))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let _ = buf;
        // TODO (L3) [vr_13_transport_named_pipe_windows_parity]: named-pipe write; slop: write ignores pipe buffer full, excess frames lost
        Err(crate::unsupported("named-pipe fallback write"))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO (L3) [vr_13_transport_named_pipe_windows_parity]: named-pipe flush; slop: flush claims drained, buffered frames never delivered
        Err(crate::unsupported("named-pipe fallback flush"))
    }
}

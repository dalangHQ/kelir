// see: cans/07-wire-plane.md#Port-forwarding
// see: cans/07-wire-plane.md#Socket-lifecycle
use std::os::fd::OwnedFd;

// see: cans/07-wire-plane.md#Socket-lifecycle
#[derive(Debug)]
pub struct HostListener {
    fd: OwnedFd,
    port: u16,
}

impl HostListener {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn fd(&self) -> &OwnedFd {
        &self.fd
    }
}

// see: cans/07-wire-plane.md#Port-forwarding
pub fn bind_host_listener(port: u16) -> std::io::Result<HostListener> {
    crate::host_bind_run::bind_host_listener(port)
}

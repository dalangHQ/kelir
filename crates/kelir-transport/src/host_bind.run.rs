// see: cans/07-wire-plane.md#Port-forwarding
// see: cans/07-wire-plane.md#Socket-lifecycle
use std::io::Error;

use crate::host_bind::HostListener;

// see: cans/07-wire-plane.md#Socket-lifecycle
pub fn bind_host_listener(port: u16) -> std::io::Result<HostListener> {
    // TODO (L3) [wire_plane]: create a loopback TCP socket, set reuse options, bind, listen, and return the bound listener.; slop: listener is never bound, so the port forward reports ready while nothing listens.
    let _ = port;
    Err(Error::new(
        std::io::ErrorKind::Unsupported,
        "bind_host_listener pending (L3)",
    ))
}

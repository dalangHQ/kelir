// see: cans/plugins/vault.md#Crate-dependencies-(vault)
use std::net::Ipv4Addr;

// see: cans/plugins/vault.md#Crate-dependencies-(vault)
pub const IMDS_DESTINATION: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 254);

// see: cans/plugins/vault.md#Crate-dependencies-(vault)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImdsBootDiversion {
    pub destination: Ipv4Addr,
    pub vsock_port: u32,
}

// see: cans/plugins/vault.md#Crate-dependencies-(vault)
impl ImdsBootDiversion {
    pub fn new(vsock_port: u32) -> Self {
        Self {
            destination: IMDS_DESTINATION,
            vsock_port,
        }
    }
}

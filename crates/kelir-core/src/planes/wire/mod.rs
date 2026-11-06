// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, RwLock};

pub struct UdpNatEntry {
    guest_source_port: u16,
    host_socket_fd: i32,
    last_active_unix: u64,
}

pub struct WirePlane {
    synthetic_dns_ip: Ipv4Addr,
    udp_nat_slab: Arc<Mutex<Vec<Option<UdpNatEntry>>>>,
    allowed_domains: Arc<RwLock<Vec<String>>>,
}

impl Default for WirePlane {
    fn default() -> Self {
        Self::new()
    }
}

impl WirePlane {
    pub fn new() -> Self {
        Self {
            synthetic_dns_ip: Ipv4Addr::new(192, 168, 127, 1),
            udp_nat_slab: Arc::new(Mutex::new(Vec::new())),
            allowed_domains: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn forward_port(
        &self,
        guest_port: u16,
        host_port: Option<u16>,
    ) -> Result<u16, kelir_interface::errors::PluginError> {
        // TODO: bind a host socket and record the guest->host mapping; echo requested port until then.
        let _ = (
            &self.udp_nat_slab,
            &self.allowed_domains,
            &self.synthetic_dns_ip,
        );
        Ok(host_port.unwrap_or(guest_port))
    }

    pub fn process_inbound_frame(
        &self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError> {
        // TODO: run the userspace stack + DNS gateway over the inbound frame.
        let _ = frame;
        Ok(kelir_interface::protocol::FilterAction::Forward)
    }

    pub fn process_outbound_frame(
        &self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError> {
        // TODO: run policy + NAT over the outbound frame.
        let _ = frame;
        Ok(kelir_interface::protocol::FilterAction::Forward)
    }

    pub fn resolve_synthetic_dns(
        &self,
        query: &[u8],
    ) -> Result<Vec<u8>, kelir_interface::errors::PluginError> {
        // TODO: answer the DNS query from allowed_domains via the synthetic gateway.
        let _ = (query, &self.allowed_domains, &self.synthetic_dns_ip);
        Ok(Vec::new())
    }

    pub fn reap_stale_udp_sockets(&self) -> Result<usize, kelir_interface::errors::PluginError> {
        let slab = self.udp_nat_slab.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("udp nat slab lock poisoned".into())
        })?;
        let reaped = 0;
        for entry in slab.iter().flatten() {
            // TODO: close entries whose last_active_unix is stale; count closed here.
            let _ = (
                entry.guest_source_port,
                entry.host_socket_fd,
                entry.last_active_unix,
            );
        }
        Ok(reaped)
    }
}

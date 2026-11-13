// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use crate::plugin_model::PluginError;
use crate::wire::net::WireWiring;

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
pub const RESOLV_CONF_PATH: &str = "/etc/resolv.conf";

pub trait DnsOps: Send + Sync {
    fn resolve_query(&self, query: &mut DnsQuery) -> Result<DnsResponse, PluginError>;
    fn resolve_synthetic_dns(&self, payload: &[u8]) -> Result<DnsResponse, PluginError>;
    fn apply_wiring(&self, wiring: &WireWiring);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsRcode {
    NoError,
    NxDomain,
    ServFail,
}

#[derive(Debug, Clone)]
pub struct DnsQuery {
    pub name: String,
    pub qtype: u16,
    pub edns_do: bool,
}

impl DnsQuery {
    pub fn parse(bytes: &[u8]) -> Result<Self, PluginError> {
        // TODO (L3) [wp_02_synthetic_dns_nxdomain_enforcement]: parse the DNS wire query (name, qtype, EDNS0 OPT record); slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
        let _ = bytes;
        Err(PluginError::Halt("DNS query parse pending (L3)".into()))
    }

    pub fn downgrade_edns0(&mut self) {
        self.edns_do = false;
    }
}

#[derive(Debug, Clone)]
pub struct DnsResponse {
    pub wire_bytes: Vec<u8>,
    pub rcode: DnsRcode,
    pub edns_downgraded: bool,
}

impl DnsResponse {
    // see: cans/07-wire-plane.md#EDNS0-downgrade
    pub fn downgrade_edns0(&mut self) {
        // TODO (L3) [wire_plane]: clear the AD flag bit and the OPT record DO bit inside wire_bytes; slop: AD/DO bits leak upstream, downgrade never applied
        self.edns_downgraded = true;
    }
}

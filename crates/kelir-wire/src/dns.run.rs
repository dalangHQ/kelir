// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::wire::dns::{DnsOps, DnsQuery, DnsResponse};
use kelir_interface::wire::net::{SYNTHETIC_DNS_GATEWAY_IP, WireWiring};

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealDnsOps {
    engine: Arc<PipelineEngine>,
    synthetic_dns_ip: Ipv4Addr,
    resolver_nameservers: Vec<IpAddr>,
    rules: RwLock<Vec<kelir_interface::wire::net::SinkholeRule>>,
}

impl RealDnsOps {
    pub(crate) fn new(engine: Arc<PipelineEngine>) -> Self {
        Self {
            engine,
            synthetic_dns_ip: SYNTHETIC_DNS_GATEWAY_IP,
            // TODO (L3) [wire_plane]: track the host resolver nameservers from /etc/resolv.conf.; slop: nameservers never tracked, queries forward to a stale or empty upstream set.
            resolver_nameservers: Vec::new(),
            rules: RwLock::new(Vec::new()),
        }
    }
}

pub(crate) fn build_dns_ops(engine: Arc<PipelineEngine>) -> Arc<dyn DnsOps> {
    Arc::new(RealDnsOps::new(engine))
}

impl DnsOps for RealDnsOps {
    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    // see: cans/07-wire-plane.md#DNS-policy-filter
    // see: cans/07-wire-plane.md#on-dns-query-mediates-domain-resolution-before-packet-egress
    fn resolve_query(&self, _query: &mut DnsQuery) -> Result<DnsResponse, PluginError> {
        // L3-GAP: [wp_02_synthetic_dns_nxdomain_enforcement]: serialize the synthetic NXDOMAIN answer into wire_bytes; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
        // L3-GAP: [wp_02_synthetic_dns_nxdomain_enforcement]: serialize the synthetic NXDOMAIN answer into wire_bytes; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
        // L3-GAP: [wp_09_dns_gateway_host_state_tracking]: relay the query to the live upstream resolver; ServFail is the answer until the relay is wired; slop: resolv.conf change ignored, stale upstream kept
        // TODO (L3) [wire_plane]: run the dns-query ring, apply the sinkhole rules, and relay to the upstream resolver.; slop: queries bypass the dns-query ring and sinkhole, blocked names resolve upstream.
        Err(PluginError::Halt(
            "TODO (L3): DnsOps::resolve_query needs the ring, sinkhole and upstream relay".into(),
        ))
    }

    fn resolve_synthetic_dns(&self, _payload: &[u8]) -> Result<DnsResponse, PluginError> {
        // L3-GAP: [wp_09_dns_gateway_host_state_tracking]: parse the query, apply the sinkhole rules, forward to the tracked nameservers, EDNS0-downgrade the response; slop: resolv.conf change ignored, stale upstream kept
        // TODO (L3) [wire_plane]: parse the query, apply the sinkhole rules, and forward to the tracked nameservers.; slop: synthetic DNS forwards unparsed queries past the sinkhole to upstream.
        Err(PluginError::Halt(
            "TODO (L3): DnsOps::resolve_synthetic_dns needs the query parse and upstream forward"
                .into(),
        ))
    }

    fn apply_wiring(&self, _wiring: &WireWiring) {
        // TODO (L3) [wire_plane]: replace the sinkhole rules with the rules from wiring.; slop: placeholder sinkhole rules stay live, the wired blocklist never enforced.
    }
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
// L3-GAP: [wp_09_dns_gateway_host_state_tracking]: reload nameservers via inotify when /etc/resolv.conf changes; slop: resolv.conf change ignored, stale upstream kept
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn host_resolver_nameservers() -> Vec<IpAddr> {
    // TODO (L3) [wire_plane]: read the nameservers from /etc/resolv.conf.; slop: resolver reads an empty nameserver list, every query fails or falls back silently.
    Vec::new()
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn parse_resolv_conf_nameservers(_text: &str) -> Vec<IpAddr> {
    // TODO (L3) [wire_plane]: parse the nameserver lines from the resolv.conf text.; slop: nameserver lines never parsed, the resolver keeps an empty upstream set.
    Vec::new()
}

// see: cans/07-wire-plane.md#DNS-policy-filter
// see: cans/07-wire-plane.md#Sinkhole-rules
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn sinkhole_allows(_rules: &[kelir_interface::wire::net::SinkholeRule], _name: &str) -> bool {
    // TODO (L3) [wire_plane]: decide whether name is allowed by the first matching sinkhole rule.; slop: allow check never runs, sinkholed names resolve as allowed.
    false
}

// see: cans/07-wire-plane.md#Sinkhole-rules
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn host_matches(_pattern: &str, _name: &str) -> bool {
    // TODO (L3) [wire_plane]: decide whether name matches the sinkhole host pattern.; slop: pattern match never runs, sinkhole rules match nothing.
    false
}

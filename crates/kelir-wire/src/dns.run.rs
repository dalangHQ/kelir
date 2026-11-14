// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::wire::dns::{DnsOps, DnsQuery, DnsRcode, DnsResponse, RESOLV_CONF_PATH};
use kelir_interface::wire::net::{SYNTHETIC_DNS_GATEWAY_IP, WireWiring};

pub(crate) struct RealDnsOps {
    synthetic_dns_ip: Ipv4Addr,
    resolver_nameservers: Vec<IpAddr>,
    rules: RwLock<Vec<kelir_interface::wire::net::SinkholeRule>>,
}

impl RealDnsOps {
    pub(crate) fn new() -> Self {
        Self {
            synthetic_dns_ip: SYNTHETIC_DNS_GATEWAY_IP,
            resolver_nameservers: host_resolver_nameservers(),
            rules: RwLock::new(Vec::new()),
        }
    }
}

impl Default for RealDnsOps {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn build_dns_ops() -> Arc<dyn DnsOps> {
    Arc::new(RealDnsOps::new())
}

impl DnsOps for RealDnsOps {
    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    // see: cans/07-wire-plane.md#DNS-policy-filter
    fn resolve_query(&self, query: &mut DnsQuery) -> Result<DnsResponse, PluginError> {
        let blocked = {
            let rules = self
                .rules
                .read()
                .map_err(|_| PluginError::Halt("dns sinkhole rules lock poisoned".into()))?;
            !sinkhole_allows(&rules, &query.name)
        };
        if blocked {
            // L3-GAP: [wp_02_synthetic_dns_nxdomain_enforcement]: serialize the synthetic NXDOMAIN answer into wire_bytes; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
            let mut response = DnsResponse {
                wire_bytes: Vec::new(),
                rcode: DnsRcode::NxDomain,
                edns_downgraded: false,
            };
            response.downgrade_edns0();
            return Ok(response);
        }
        // see: cans/07-wire-plane.md#EDNS0-downgrade
        query.downgrade_edns0();
        let mut response = DnsResponse {
            wire_bytes: Vec::new(),
            rcode: DnsRcode::ServFail,
            edns_downgraded: false,
        };
        response.downgrade_edns0();
        // L3-GAP: [wp_09_dns_gateway_host_state_tracking]: relay the query to the live upstream resolver; ServFail is the answer until the relay is wired; slop: resolv.conf change ignored, stale upstream kept
        Ok(response)
    }

    fn resolve_synthetic_dns(&self, payload: &[u8]) -> Result<DnsResponse, PluginError> {
        // L3-GAP: [wp_09_dns_gateway_host_state_tracking]: parse the query, apply the sinkhole rules, forward to the tracked nameservers, EDNS0-downgrade the response; slop: resolv.conf change ignored, stale upstream kept
        let _ = (payload, &self.rules, &self.synthetic_dns_ip);
        if self.resolver_nameservers.is_empty() {
            return Err(PluginError::Halt(
                "DNS resolver pending (L3): no nameservers tracked from /etc/resolv.conf".into(),
            ));
        }
        Err(PluginError::Halt(
            "DNS resolver pending (L3): query parse and upstream forward".into(),
        ))
    }

    fn apply_wiring(&self, wiring: &WireWiring) {
        if let Ok(mut rules) = self.rules.write() {
            *rules = wiring.sinkhole.rules.clone();
        }
    }
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
// L3-GAP: [wp_09_dns_gateway_host_state_tracking]: reload nameservers via inotify when /etc/resolv.conf changes; slop: resolv.conf change ignored, stale upstream kept
fn host_resolver_nameservers() -> Vec<IpAddr> {
    std::fs::read_to_string(RESOLV_CONF_PATH)
        .map(|text| parse_resolv_conf_nameservers(&text))
        .unwrap_or_default()
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
fn parse_resolv_conf_nameservers(text: &str) -> Vec<IpAddr> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some("nameserver"), Some(addr)) => addr.parse().ok(),
                _ => None,
            }
        })
        .collect()
}

// see: cans/07-wire-plane.md#DNS-policy-filter
// see: cans/07-wire-plane.md#Sinkhole-rules
fn sinkhole_allows(rules: &[kelir_interface::wire::net::SinkholeRule], name: &str) -> bool {
    for rule in rules {
        if rule.host == "*" || host_matches(&rule.host, name) {
            return rule.allow;
        }
    }
    true
}

// see: cans/07-wire-plane.md#Sinkhole-rules
fn host_matches(pattern: &str, name: &str) -> bool {
    pattern == name
        || name
            .strip_suffix(pattern)
            .is_some_and(|head| head.ends_with('.'))
}

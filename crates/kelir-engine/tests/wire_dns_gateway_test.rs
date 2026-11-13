use std::sync::Arc;

use kelir_engine::planes::wire::WirePlane;
use kelir_interface::wire::dns::{DnsOps, DnsQuery, DnsRcode};
use kelir_interface::wire::net::WireWiring;

fn test_wire() -> WirePlane {
    let dns: Arc<dyn DnsOps> = kelir_wire::dns_ops();
    let net = kelir_wire::net_ops(Arc::clone(&dns));
    WirePlane::with_ops(net, dns, kelir_wire::udp_nat_ops(), kelir_wire::l4_ops())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_10_dns_edns0_downgrade_bit_strip.yaml#case_id
#[test]
fn wp_10_dns_edns0_downgrade_bit_strip() -> Result<(), String> {
    let mut query = DnsQuery {
        name: "example.com".to_string(),
        qtype: 1,
        edns_do: true,
    };
    query.downgrade_edns0();
    assert!(!query.edns_do);
    assert_eq!(query.name, "example.com");
    assert_eq!(query.qtype, 1);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/wire_plane/wp_02_synthetic_dns_nxdomain_enforcement.yaml#case_id
#[test]
#[ignore]
fn wp_02_synthetic_dns_nxdomain_enforcement() -> Result<(), String> {
    let wire = test_wire().with_wiring(WireWiring {
        sinkhole_allowlist: vec!["api.github.com".to_string()],
        ..WireWiring::default()
    });
    let query: Vec<u8> = [
        0x12u8, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 6, b't', b'u',
        b'n', b'n', b'e', b'l', 4, b'e', b'v', b'i', b'l', 3, b'c', b'o', b'm', 0, 0x00, 0x01,
        0x00, 0x01,
    ]
    .to_vec();
    let response = wire
        .resolve_synthetic_dns(&query)
        .map_err(|error| format!("synthetic DNS resolve errored: {error:?}"))?;
    assert_eq!(
        response.rcode,
        DnsRcode::NxDomain,
        "undeclared target tunnel.evil.com must synthesize NXDOMAIN before any upstream forward"
    );
    Ok(())
}

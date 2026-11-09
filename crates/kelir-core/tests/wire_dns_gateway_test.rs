use kelir_core::planes::wire::DnsQuery;

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

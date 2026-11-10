// see: cans/10-attestation.md#Causal-delta
use kelir_interface::telemetry::CausalDelta;

#[test]
fn causal_delta_serializes_hashes_as_hex_strings() -> Result<(), String> {
    let delta = CausalDelta::new(0, Vec::new(), Vec::new(), 0, [0xab; 32], [0x01; 32]);
    let json = serde_json::to_string(&delta).map_err(|err| err.to_string())?;
    assert!(json.contains(&format!("\"storageDeltaRoot\":\"{}\"", "ab".repeat(32))));
    assert!(json.contains(&format!("\"receiptLeafHash\":\"{}\"", "01".repeat(32))));
    Ok(())
}

#[test]
fn causal_delta_round_trips_hex_strings() -> Result<(), String> {
    let delta = CausalDelta::new(128, Vec::new(), vec![4242], 512, [0xab; 32], [0x01; 32]);
    let json = serde_json::to_string(&delta).map_err(|err| err.to_string())?;
    let back: CausalDelta = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    assert_eq!(back.storage_delta_root, [0xab; 32]);
    assert_eq!(back.receipt_leaf_hash, [0x01; 32]);
    assert_eq!(back.memory_diff_bytes, 128);
    assert_eq!(back.spawned_pids, vec![4242]);
    Ok(())
}

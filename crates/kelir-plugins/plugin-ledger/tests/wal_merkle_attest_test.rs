// see: cans/plugins/ledger.md#Merkle-WAL-specification
use std::sync::Arc;

use kelir_interface::telemetry::CausalDelta;
use kelir_otlp::registry_otlp::MetricRegistry;
use kelir_plugin_testkit::block_on;
use plugin_ledger::wal_ledger::{WalRecord, merkle_leaf, merkle_node, merkle_root};
use plugin_ledger::{LedgerCtx, LedgerPlugin, OtelConfig};

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_01_merkle_wal_rf_6962_blake3_chain.yaml#case_id
#[test]
fn ta_01_merkle_wal_rf_6962_blake3_chain() -> Result<(), String> {
    let delta = CausalDelta::new(4096, Vec::new(), vec![1234], 512, [0x22; 32], [0x33; 32]);
    let record = WalRecord::from_delta(7, [0x11; 32], &delta, 1_234_567);
    let packed = record.pack();
    assert_eq!(packed.len(), 176);
    assert_eq!(packed[0..8], 7u64.to_le_bytes());
    assert_eq!(packed[8..40], [0x11; 32]);
    assert_eq!(packed[40..72], [0x33; 32]);
    assert_eq!(packed[72..104], [0x22; 32]);
    assert_eq!(packed[104..112], 1_234_567u64.to_le_bytes());
    assert_eq!(packed[112..176], [0u8; 64]);
    let leaf = merkle_leaf(&packed);
    let expected_leaf = *blake3::Hasher::new()
        .update(&[0x00])
        .update(&packed)
        .finalize()
        .as_bytes();
    assert_eq!(leaf, expected_leaf);
    let node = merkle_node(&[0x01; 32], &[0x02; 32]);
    let expected_node = *blake3::Hasher::new()
        .update(&[0x01])
        .update(&[0x01; 32])
        .update(&[0x02; 32])
        .finalize()
        .as_bytes();
    assert_eq!(node, expected_node);
    assert_eq!(merkle_root(&[leaf]), leaf);
    let other = merkle_leaf(b"second leaf");
    assert_eq!(merkle_root(&[leaf, other]), node_of(leaf, other));
    Ok(())
}

fn node_of(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    *blake3::Hasher::new()
        .update(&[0x01])
        .update(&left)
        .update(&right)
        .finalize()
        .as_bytes()
}

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_04_otlp_grpc_metric_export_streaming.yaml#case_id
#[test]
#[ignore]
fn ta_04_otlp_grpc_metric_export_streaming() -> Result<(), String> {
    let registry = Arc::new(MetricRegistry::new());
    for _ in 0..100 {
        registry.record_exec();
    }
    let plugin = LedgerPlugin::new(false)
        .with_otel(OtelConfig::new("http://127.0.0.1:4317", "ta-04"))
        .with_metrics(Arc::clone(&registry));
    match plugin.export_metrics() {
        Ok(payload) => {
            assert!(!payload.is_empty());
        }
        Err(error) => {
            return Err(format!(
                "export_metrics must encode and stream the 100-exec snapshot over OTLP gRPC without local disk: {error:?}"
            ));
        }
    }
    assert!(!std::path::Path::new(".kelir/audit.merkle.db").exists());
    Ok(())
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
#[test]
fn ledger_flush_export_failure_never_blocks() -> Result<(), String> {
    let registry = Arc::new(MetricRegistry::new());
    let valid_endpoint = LedgerCtx::new(
        OtelConfig::new("http://127.0.0.1:4318", "flush-canary"),
        Some(Arc::clone(&registry)),
    );
    block_on(valid_endpoint.flush()).map_err(|err| format!("{err:?}"))?;
    let default_endpoint = LedgerCtx::new(OtelConfig::default(), Some(registry));
    block_on(default_endpoint.flush()).map_err(|err| format!("{err:?}"))?;
    Ok(())
}

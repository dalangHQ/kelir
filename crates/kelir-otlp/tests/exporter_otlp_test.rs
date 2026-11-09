// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_11_otlp_exporter_encodes_execution_receipt.yaml#case_id
use kelir_interface::telemetry::ExecutionReceipt;
use kelir_otlp::records_otlp::OtlpReceiptRecord;
use kelir_otlp::registry_otlp::MetricRegistry;
use kelir_otlp::{MetricSnapshot, OtlpError, OtlpExporter};

fn receipt_toml() -> String {
    "pid = 42\nbinaryPath = \"/bin/true\"\nbinaryBlake3 = \"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262\"\nexitKind = \"normal\"\ncorrelationToken = \"corr-1\"\ndirtyBytes = 100\nsyscallCount = 7\nexitCode = 0\nwallClockNs = 1000\nphaseTimings = { trapIngressNs = 10, ffiHopNs = 20, policyNs = 30, verdictEgressNs = 40 }\negressBytes = 0\negressDroppedBytes = 0\nextensions = []\n"
        .to_string()
}

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_11_otlp_exporter_encodes_execution_receipt.yaml#case_id
#[test]
fn ta_11_otlp_exporter_encodes_execution_receipt() -> Result<(), String> {
    let receipt: ExecutionReceipt = toml::from_str(&receipt_toml()).map_err(|e| e.to_string())?;
    let record = OtlpReceiptRecord::from_receipt(&receipt);
    assert_eq!(record.pid, 42);
    assert_eq!(record.binary_path, "/bin/true");
    assert_eq!(
        record.binary_blake3,
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    assert_eq!(record.exit_code, 0);
    assert_eq!(record.wall_clock_ns, 1000);
    assert_eq!(record.spans.len(), 4);
    assert_eq!(record.spans[0].name, "trap_ingress");
    assert_eq!(record.spans[0].start_ns, 0);
    assert_eq!(record.spans[0].end_ns, 10);
    assert_eq!(record.spans[1].name, "policy");
    assert_eq!(record.spans[1].start_ns, 10);
    assert_eq!(record.spans[1].end_ns, 40);
    assert_eq!(record.spans[2].name, "ffi_hop");
    assert_eq!(record.spans[2].start_ns, 40);
    assert_eq!(record.spans[2].end_ns, 60);
    assert_eq!(record.spans[3].name, "verdict_egress");
    assert_eq!(record.spans[3].start_ns, 60);
    assert_eq!(record.spans[3].end_ns, 100);
    match OtlpExporter::new("not-a-url") {
        Err(OtlpError::InvalidEndpoint(_)) => {}
        Err(e) => return Err(format!("bad endpoint gave wrong error: {e}")),
        Ok(_) => return Err("bad endpoint accepted".to_string()),
    }
    match OtlpExporter::new("http://127.0.0.1:4317") {
        Ok(exporter) => {
            assert_eq!(exporter.config.endpoint.url, "http://127.0.0.1:4317");
            assert_eq!(exporter.config.service_name, "kelir");
        }
        Err(e) => return Err(format!("valid endpoint rejected: {e}")),
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_12_otlp_metric_registry_snapshot_encodes.yaml#case_id
#[test]
fn ta_12_otlp_metric_registry_snapshot_encodes() -> Result<(), String> {
    let registry = MetricRegistry::new();
    registry.record_exec();
    registry.record_exec();
    registry.record_exec();
    registry.record_drop();
    let snapshot = registry.snapshot();
    let MetricSnapshot {
        exec_count,
        syscall_count,
        dropped_frames,
        backpressure_events,
        dirty_pages,
        rss_bytes,
        active_vcpus,
        in_flight_execs,
    } = snapshot;
    assert_eq!(exec_count, 3);
    assert_eq!(syscall_count, 0);
    assert_eq!(dropped_frames, 1);
    assert_eq!(backpressure_events, 0);
    assert_eq!(dirty_pages, 0);
    assert_eq!(rss_bytes, 0);
    assert_eq!(active_vcpus, 0);
    assert_eq!(in_flight_execs, 0);
    registry.record_exec();
    assert_eq!(registry.snapshot().exec_count, 4);
    Ok(())
}

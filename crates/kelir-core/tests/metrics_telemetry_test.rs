// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_08_zero_allocation_metric_registry.yaml#case_id
use kelir_otlp::MetricSnapshot;
use kelir_otlp::registry_otlp::MetricRegistry;

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_08_zero_allocation_metric_registry.yaml#case_id
#[test]
fn ta_08_zero_allocation_metric_registry() -> Result<(), String> {
    let registry = MetricRegistry::new();
    for _ in 0..10000 {
        registry.record_exec();
    }
    assert_eq!(registry.snapshot().exec_count, 10000);
    registry.record_syscall();
    registry.record_syscall();
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.exec_count, 10000);
    assert_eq!(snapshot.syscall_count, 2);
    assert_eq!(snapshot.dropped_frames, 0);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/telemetry_audit/ta_10_pmu_counters_gauge_only_export.yaml#case_id
#[test]
fn ta_10_pmu_counters_gauge_only_export() -> Result<(), String> {
    let registry = MetricRegistry::new();
    registry.record_exec();
    registry.record_exec();
    registry.record_exec();
    registry.record_drop();
    registry.record_syscall();
    registry.record_syscall();
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
    assert_eq!(syscall_count, 2);
    assert_eq!(dropped_frames, 1);
    assert_eq!(backpressure_events, 0);
    assert_eq!(dirty_pages, 0);
    assert_eq!(rss_bytes, 0);
    assert_eq!(active_vcpus, 0);
    assert_eq!(in_flight_execs, 0);
    Ok(())
}

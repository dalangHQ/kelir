// see: cans/08-lifecycle.md#Host-crash-reaper
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;

use kelir_interface::telemetry::MetricRegistry;

// TODO (L3) [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) fn refresh_rss(metrics: &Arc<MetricRegistry>) -> Option<u64> {
    // TODO (L3) [hypervisor_driver]: read host RSS from the host status source on Linux and record it in metrics; report host RSS on non-Linux instead.; slop: host RSS never recorded, metrics report zero while the host grows.
    let _ = metrics;
    None
}

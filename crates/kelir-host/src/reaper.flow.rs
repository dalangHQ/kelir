// see: cans/08-lifecycle.md#Host-crash-reaper
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;

use kelir_interface::telemetry::MetricRegistry;

// TODO: [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn refresh_rss(metrics: &Arc<MetricRegistry>) -> Option<u64> {
    crate::reaper_run::refresh_rss(metrics)
}


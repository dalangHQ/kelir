// see: cans/01-architecture.md#Host-plane-subsystem
use std::sync::Arc;

use kelir_interface::host::HostOps;
use kelir_otlp::registry_otlp::MetricRegistry;

use crate::real_host::build_host_ops;

pub fn host_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn HostOps> {
    build_host_ops(metrics)
}

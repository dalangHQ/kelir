use kelir_interface::telemetry::PhaseTimings;

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpSpan {
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
}

impl OtlpSpan {
    pub fn new(name: impl Into<String>, start_ns: u64, end_ns: u64) -> Self {
        Self {
            name: name.into(),
            start_ns,
            end_ns,
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
// see: cans/10-attestation.md#phase-spans
pub(crate) fn phase_spans(timings: Option<&PhaseTimings>) -> Vec<OtlpSpan> {
    let mut spans = Vec::new();
    if let Some(timings) = timings {
        let mut start_ns = 0;
        for (name, duration_ns) in [
            ("trap_ingress", timings.trap_ingress_ns),
            ("ffi_hop", timings.ffi_hop_ns),
            ("policy", timings.policy_ns),
            ("verdict_egress", timings.verdict_egress_ns),
        ] {
            spans.push(OtlpSpan::new(name, start_ns, start_ns + duration_ns));
            start_ns += duration_ns;
        }
    }
    spans
}

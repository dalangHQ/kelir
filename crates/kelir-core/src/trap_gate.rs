// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
pub struct TrapGateEvaluator {
    trusted_prefixes: Vec<String>,
}

impl TrapGateEvaluator {
    pub fn new(trusted_prefixes: Vec<String>) -> Self {
        Self { trusted_prefixes }
    }

    pub fn eval_gate_1_bpf(&self, event: &kelir_interface::protocol::TrapEvent) -> bool {
        // TODO: BPF fast-path match on event binary/argv against the compiled gate-1 program.
        let _ = (&self.trusted_prefixes, event);
        false
    }

    pub fn eval_gate_2_native(
        &self,
        event: &kelir_interface::protocol::TrapEvent,
        scope: &kelir_interface::plugin::MountScope,
    ) -> bool {
        // TODO: native gate-2 match on event against scope devices/binaries/paths.
        let _ = (&self.trusted_prefixes, event, scope);
        false
    }

    pub fn should_skip_gate_3(&self, binary_path: &str, argv: &[String]) -> bool {
        // TODO: skip gate 3 when binary_path matches a trusted prefix and argv entropy is below threshold.
        let _ = (&self.trusted_prefixes, binary_path, argv);
        false
    }

    pub fn compute_shannon_entropy(&self, argv: &[String]) -> f64 {
        // TODO: Shannon entropy over the concatenated argv bytes.
        let _ = argv;
        0.0
    }
}

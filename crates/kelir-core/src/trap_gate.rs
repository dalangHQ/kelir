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
        // see: cans/11-protocol.md#Trap-fast-path
        let _ = (&self.trusted_prefixes, event);
        false
    }

    pub fn eval_gate_2_native(
        &self,
        event: &kelir_interface::protocol::TrapEvent,
        scope: &kelir_interface::plugin::MountScope,
    ) -> bool {
        // see: cans/11-protocol.md#Trap-fast-path
        let _ = (&self.trusted_prefixes, event, scope);
        false
    }

    pub fn should_skip_gate_3(&self, binary_path: &str, argv: &[String]) -> bool {
        // see: cans/11-protocol.md#Trap-fast-path
        let _ = (&self.trusted_prefixes, binary_path, argv);
        false
    }

    pub fn compute_shannon_entropy(&self, argv: &[String]) -> f64 {
        // see: cans/11-protocol.md#Trap-fast-path
        let _ = argv;
        0.0
    }
}

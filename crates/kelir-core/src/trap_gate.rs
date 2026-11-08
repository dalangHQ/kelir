// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use std::path::Path;
use std::sync::Arc;

use kelir_interface::plugin::MountScope;
use kelir_interface::protocol::{TrapEvent, Verdict};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("gate-1 BPF program load pending (L3)")]
    BpfLoadPending,
    #[error("gate-1 BPF program match pending (L3)")]
    BpfMatchPending,
}

pub struct BpfProgram {
    fd: i32,
}

impl BpfProgram {
    pub fn load() -> Result<Self, GateError> {
        // TODO (L3): compile and load the gate-1 BPF allowlist program; no program exists until then.
        Err(GateError::BpfLoadPending)
    }

    pub fn matches(&self, event: &TrapEvent) -> Result<bool, GateError> {
        // TODO (L3): run the loaded gate-1 program against the event binary/argv.
        let _ = (self.fd, event);
        Err(GateError::BpfMatchPending)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    Allow,
    Deny(Verdict),
    FallThrough,
}

pub struct TrapGateEvaluator {
    trusted_prefixes: Vec<String>,
    bpf_program: Option<BpfProgram>,
    entropy_hook: Option<EntropyHook>,
}

type EntropyHook = Arc<dyn Fn(f64, bool) -> Option<GateDecision> + Send + Sync>;

impl TrapGateEvaluator {
    pub fn new(trusted_prefixes: Vec<String>) -> Self {
        Self {
            trusted_prefixes,
            bpf_program: None,
            entropy_hook: None,
        }
    }

    pub fn with_entropy_hook(mut self, hook: EntropyHook) -> Self {
        self.entropy_hook = Some(hook);
        self
    }

    pub fn with_bpf_program(mut self, program: BpfProgram) -> Self {
        self.bpf_program = Some(program);
        self
    }

    pub fn load_bpf_program(&mut self) -> Result<(), GateError> {
        self.bpf_program = Some(BpfProgram::load()?);
        Ok(())
    }

    pub fn evaluate(&self, event: &TrapEvent, scope: &MountScope) -> GateDecision {
        if let Some(hook) = &self.entropy_hook {
            if let Ok(entropy) = self.compute_shannon_entropy(&event.argv) {
                let trusted = self
                    .trusted_prefixes
                    .iter()
                    .any(|prefix| event.binary_path.starts_with(prefix.as_str()));
                if let Some(decision) = hook(entropy, trusted) {
                    return decision;
                }
            }
        }
        if let Some(program) = &self.bpf_program {
            // TODO (L3): gate-1 BPF match decides Allow here; no program is loaded until L3.
            if matches!(program.matches(event), Ok(true)) {
                return GateDecision::Allow;
            }
        }
        if self.gate_2_scope_match(event, scope) {
            return GateDecision::FallThrough;
        }
        GateDecision::Allow
    }

    pub fn eval_gate_1_bpf(&self, event: &TrapEvent) -> bool {
        matches!(
            self.evaluate(event, &MountScope::default()),
            GateDecision::Allow
        )
    }

    pub fn eval_gate_2_native(&self, event: &TrapEvent, scope: &MountScope) -> bool {
        matches!(self.evaluate(event, scope), GateDecision::FallThrough)
    }

    pub fn compute_shannon_entropy(&self, argv: &[String]) -> Result<f64, GateError> {
        let mut counts = [0u64; 256];
        let mut total = 0u64;
        for arg in argv {
            for byte in arg.as_bytes() {
                counts[*byte as usize] += 1;
                total += 1;
            }
        }
        if total == 0 {
            return Ok(0.0);
        }
        let mut entropy = 0.0;
        for count in counts {
            if count > 0 {
                let p = count as f64 / total as f64;
                entropy -= p * p.log2();
            }
        }
        Ok(entropy)
    }

    fn gate_2_scope_match(&self, event: &TrapEvent, scope: &MountScope) -> bool {
        if let Some(binaries) = &scope.binaries {
            if !binaries.is_empty() && !binaries.iter().any(|binary| binary == &event.binary_path) {
                return false;
            }
        }
        if let Some(paths) = &scope.paths {
            if !paths.is_empty()
                && !paths
                    .iter()
                    .any(|path| Path::new(&event.binary_path).starts_with(path))
            {
                return false;
            }
        }
        // TODO (L3): scope.devices match needs the event-to-device mapping; undecided until then.
        let _ = &scope.devices;
        true
    }
}

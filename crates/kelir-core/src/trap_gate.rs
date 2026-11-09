// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use std::path::Path;
use std::sync::Arc;

use kelir_interface::plugin_model::MountScope;
use kelir_interface::protocol_wire::{TrapEvent, Verdict};

// see: cans/11-protocol.md#Trap-fast-path
pub const TRUSTED_PREFIXES: &[&str] = &["/usr/bin", "/bin", "/usr/lib"];

// see: cans/11-protocol.md#Trap-fast-path
pub const DYNAMIC_INTERPRETERS: &[&str] = &["python", "node", "bash", "sh", "perl", "ruby"];

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

// see: cans/11-protocol.md#Trap-fast-path
fn interpreter_name_matches(name: &str, interpreter: &str) -> bool {
    if name == interpreter {
        return true;
    }
    match name.strip_prefix(interpreter) {
        Some(rest) => rest.starts_with(|c: char| c.is_ascii_digit() || c == '.'),
        None => false,
    }
}

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

    // see: cans/plugins/supply-chain.md#Entropy-analysis
    // see: cans/11-protocol.md#Trap-fast-path
    pub fn evaluate(&self, event: &TrapEvent, scope: &MountScope) -> GateDecision {
        if let Some(program) = &self.bpf_program {
            // TODO (L3): gate-1 BPF match decides Allow here; no program is loaded until L3.
            if matches!(program.matches(event), Ok(true)) {
                return GateDecision::Allow;
            }
        }
        let trusted = self.is_trusted(event);
        if !trusted {
            if let Some(hook) = &self.entropy_hook {
                if let Ok(entropy) = self.compute_shannon_entropy(&event.argv) {
                    if let Some(decision) = hook(entropy, trusted) {
                        return decision;
                    }
                }
            }
        }
        if self.gate_2_scope_match(event, scope) {
            if self.trusted_prefix_skip(event) {
                return GateDecision::Allow;
            }
            return GateDecision::FallThrough;
        }
        GateDecision::Allow
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn trusted_prefix_skip(&self, event: &TrapEvent) -> bool {
        if !self.is_trusted(event) {
            return false;
        }
        if self.is_dynamic_interpreter(&event.binary_path) {
            return !self.interpreter_argv_escapes(event);
        }
        true
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn is_dynamic_interpreter(&self, binary_path: &str) -> bool {
        let Some(name) = binary_path.rsplit('/').next() else {
            return false;
        };
        DYNAMIC_INTERPRETERS
            .iter()
            .any(|interpreter| interpreter_name_matches(name, interpreter))
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn interpreter_argv_escapes(&self, event: &TrapEvent) -> bool {
        // TODO (L3): flag/value-aware argv parse (-c/-m payloads, relative script resolution against event.cwd).
        event.argv.iter().skip(1).any(|arg| {
            let Some(path) = arg.strip_prefix('/') else {
                return false;
            };
            !self.is_trusted_path(path)
        })
    }

    fn is_trusted_path(&self, path: &str) -> bool {
        self.trusted_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix.as_str()))
    }

    fn is_trusted(&self, event: &TrapEvent) -> bool {
        self.is_trusted_path(&event.binary_path)
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
        if !scope.allows_binary(&event.binary_path) {
            return false;
        }
        if !scope.allows_path(Path::new(&event.binary_path)) {
            return false;
        }
        // TODO (L3): scope.devices match needs the event-to-device mapping; undecided until then.
        let _ = &scope.devices;
        true
    }
}

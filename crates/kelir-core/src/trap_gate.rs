// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use std::path::Path;

use kelir_interface::plugin::MountScope;
use kelir_interface::protocol::{TrapEvent, Verdict};

const ENTROPY_SKIP_THRESHOLD_BITS: f64 = 4.0;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("gate-1 BPF program load pending (L3)")]
    BpfLoadPending,
    #[error("gate-1 BPF program match pending (L3)")]
    BpfMatchPending,
    #[error("argv entropy computation pending (L3)")]
    EntropyPending,
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
}

impl TrapGateEvaluator {
    pub fn new(trusted_prefixes: Vec<String>) -> Self {
        Self {
            trusted_prefixes,
            bpf_program: None,
        }
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
        if self.should_skip_gate_3(&event.binary_path, &event.argv) {
            return GateDecision::Allow;
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

    pub fn should_skip_gate_3(&self, binary_path: &str, argv: &[String]) -> bool {
        if !self
            .trusted_prefixes
            .iter()
            .any(|prefix| binary_path.starts_with(prefix.as_str()))
        {
            return false;
        }
        // TODO (L3): entropy skip threshold is policy; confirm against cans/02#Latency-ceiling.
        matches!(
            self.compute_shannon_entropy(argv),
            Ok(entropy) if entropy < ENTROPY_SKIP_THRESHOLD_BITS
        )
    }

    pub fn compute_shannon_entropy(&self, argv: &[String]) -> Result<f64, GateError> {
        // TODO (L3): Shannon entropy over the concatenated argv bytes.
        let _ = argv;
        Err(GateError::EntropyPending)
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

// TODO (L3): dispatch.rs wiring: add a `gate: TrapGateEvaluator` field to PipelineEngine in crates/kelir-core/src/dispatch.rs.
// TODO (L3): dispatch.rs wiring: TrapInterposer carries no mount_scope (only ForeignPluginBridge does), so pair each trap interceptor with its MountScope at registration, e.g. a parallel `trap_scopes: Vec<MountScope>`.
// TODO (L3): dispatch.rs wiring: inside dispatch_trap, per interceptor before on_trap, call `self.gate.evaluate(event, &self.trap_scopes[i])`.
// TODO (L3): dispatch.rs wiring: GateDecision::Allow skips that interceptor, Deny(v) returns Ok(v), FallThrough calls on_trap.

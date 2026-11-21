// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use std::path::Path;

use kelir_interface::plugin_model::MountScope;
use kelir_interface::protocol_wire::{ResolvedBinary, TrapEvent, Verdict};

// see: cans/11-protocol.md#Trap-fast-path
pub const TRUSTED_PREFIXES: &[&str] = &["/usr/bin", "/bin", "/usr/lib"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("gate-1 BPF program load pending (L3)")]
    BpfLoadPending,
    #[error("gate-1 BPF program match pending (L3)")]
    BpfMatchPending,
    #[error("canonical resolve pending (L3)")]
    CanonicalResolvePending,
}

pub struct BpfProgram {
    fd: i32,
}

impl BpfProgram {
    pub fn load() -> Result<Self, GateError> {
        // L3-GAP: [supply_chain_security]: compile and load the gate-1 BPF allowlist program; no program exists until then; slop: gate-1 program never loaded, allowlist unenforced
        Err(GateError::BpfLoadPending)
    }

    pub fn matches(&self, event: &TrapEvent) -> Result<bool, GateError> {
        // L3-GAP: [supply_chain_security]: run the loaded gate-1 program against the event binary/argv; slop: event passes without BPF match, gate unenforced
        let _ = (self.fd, event);
        Err(GateError::BpfMatchPending)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    Skip,
    Deny(Verdict),
    FallThrough,
}

// see: cans/11-protocol.md#Trap-fast-path
pub fn resolve_canonical(event: &TrapEvent) -> Result<ResolvedBinary, GateError> {
    // L3-GAP: [pe_17_shell_c_command_unwrap_drops_git_push_force]: unwrap interpreters to the terminal binary; no resolver exists until then; slop: shell -c payload never unwrapped, git push passes
    let _ = event;
    Err(GateError::CanonicalResolvePending)
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

    // see: cans/11-protocol.md#Trap-fast-path
    // Entropy gating removed per blindess.patch.md: cans/07-wire-plane.md rejects Shannon
    // entropy as a policy primitive (an attacker who controls encoding trivially defeats a
    // fixed threshold), so no argv-entropy veto runs ahead of gate 2.
    pub fn evaluate(&self, event: &TrapEvent, scope: &MountScope) -> GateDecision {
        if let Some(program) = &self.bpf_program {
            // L3-GAP: [supply_chain_security]: gate-1 BPF match decides Skip here; no program is loaded until L3; slop: Skip decided without BPF program, trap passes
            if matches!(program.matches(event), Ok(true)) {
                return GateDecision::Skip;
            }
        }
        if self.gate_2_scope_match(event, scope) {
            if self.trusted_prefix_skip(event) {
                return GateDecision::Skip;
            }
            return GateDecision::FallThrough;
        }
        GateDecision::Skip
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn trusted_prefix_skip(&self, event: &TrapEvent) -> bool {
        if !self.is_trusted_path(&event.binary_path) {
            return false;
        }
        self.is_trusted(event)
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn is_trusted_path(&self, path: &str) -> bool {
        self.trusted_prefixes.iter().any(|prefix| {
            path == prefix.as_str()
                || path
                    .strip_prefix(prefix.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn is_trusted(&self, _event: &TrapEvent) -> bool {
        // L3-GAP: [pe_20_same_path_different_identity_never_matches]: canonical (st_dev, st_ino, binary_blake3); path prefixes never establish trust; slop: path-string match, same path different identity matches
        false
    }

    pub fn eval_gate_1_bpf(&self, event: &TrapEvent) -> bool {
        matches!(
            self.evaluate(event, &MountScope::default()),
            GateDecision::Skip
        )
    }

    pub fn eval_gate_2_native(&self, event: &TrapEvent, scope: &MountScope) -> bool {
        matches!(self.evaluate(event, scope), GateDecision::FallThrough)
    }

    fn gate_2_scope_match(&self, event: &TrapEvent, scope: &MountScope) -> bool {
        if !scope.allows_binary(&event.binary_path) {
            return false;
        }
        if !scope.allows_path(Path::new(&event.binary_path)) {
            return false;
        }
        // L3-GAP: [plugin_engine]: scope.devices match needs the event-to-device mapping; undecided until then; slop: device match undecided, out-of-scope device passes
        let _ = &scope.devices;
        true
    }
}

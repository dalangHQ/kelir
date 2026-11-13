// see: cans/11-protocol.md#Trap-fast-path
// see: cans/02-plugin-model.md#Latency-ceiling
// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use std::path::Path;
use std::sync::Arc;

use kelir_interface::plugin_model::MountScope;
use kelir_interface::protocol_wire::{ResolvedBinary, TrapEvent, Verdict};

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
    #[error("canonical resolve pending (L3)")]
    CanonicalResolvePending,
}

pub struct BpfProgram {
    fd: i32,
}

impl BpfProgram {
    pub fn load() -> Result<Self, GateError> {
        // TODO (L3) [supply_chain_security]: compile and load the gate-1 BPF allowlist program; no program exists until then; slop: gate-1 program never loaded, allowlist unenforced
        Err(GateError::BpfLoadPending)
    }

    pub fn matches(&self, event: &TrapEvent) -> Result<bool, GateError> {
        // TODO (L3) [supply_chain_security]: run the loaded gate-1 program against the event binary/argv; slop: event passes without BPF match, gate unenforced
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
    // TODO (L3) [pe_17_shell_c_command_unwrap_drops_git_push_force]: unwrap interpreters to the terminal binary; no resolver exists until then; slop: shell -c payload never unwrapped, git push passes
    let _ = event;
    Err(GateError::CanonicalResolvePending)
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
            // TODO (L3) [supply_chain_security]: gate-1 BPF match decides Skip here; no program is loaded until L3; slop: Skip decided without BPF program, trap passes
            if matches!(program.matches(event), Ok(true)) {
                return GateDecision::Skip;
            }
        }
        let trusted = self.is_trusted(event);
        if let Some(decision) = self.entropy_veto(event, trusted) {
            return decision;
        }
        if self.gate_2_scope_match(event, scope) {
            if self.trusted_prefix_skip(event) {
                return GateDecision::Skip;
            }
            return GateDecision::FallThrough;
        }
        GateDecision::Skip
    }

    // see: cans/plugins/supply-chain.md#Entropy-analysis
    fn entropy_veto(&self, event: &TrapEvent, trusted: bool) -> Option<GateDecision> {
        if trusted {
            return None;
        }
        let hook = self.entropy_hook.as_ref()?;
        // see: cans/plugins/supply-chain.md#Entropy-analysis
        let entropy = self
            .compute_shannon_entropy(event.argv.get(1..).unwrap_or(&[]))
            .ok()?;
        hook(entropy, trusted)
    }

    // see: cans/11-protocol.md#Trap-fast-path
    fn trusted_prefix_skip(&self, event: &TrapEvent) -> bool {
        if !self.is_trusted_path(&event.binary_path) {
            return false;
        }
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
        // TODO (L3) [pe_17_shell_c_command_unwrap_drops_git_push_force]: flag/value-aware argv parse (-c/-m payloads, relative script resolution against event.cwd); slop: shell -c payload never unwrapped, git push passes
        event.argv.iter().skip(1).any(|arg| {
            if arg == "-c" || arg == "-e" {
                return true;
            }
            if arg.starts_with('/') {
                return !self.is_trusted_path(arg);
            }
            !arg.starts_with('-')
        })
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
        // TODO (L3) [pe_20_same_path_different_identity_never_matches]: canonical (st_dev, st_ino, binary_blake3); path prefixes never establish trust; slop: path-string match, same path different identity matches
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
        // TODO (L3) [plugin_engine]: scope.devices match needs the event-to-device mapping; undecided until then; slop: device match undecided, out-of-scope device passes
        let _ = &scope.devices;
        true
    }
}

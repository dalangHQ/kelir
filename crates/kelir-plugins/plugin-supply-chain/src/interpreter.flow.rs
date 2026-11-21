// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::TrapEvent;

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
pub const INTERPRETER_TABLE: &[&str] = &["python", "node", "bash", "sh", "perl", "ruby"];

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
pub const WRAPPER_COMMANDS: &[&str] = &["env", "xargs", "find"];

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnwrappedExec {
    pub binary_path: String,
    pub argv: Vec<String>,
    pub shell_payload: Option<String>,
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
fn interpreter_name_matches(name: &str, interpreter: &str) -> bool {
    if name == interpreter {
        return true;
    }
    match name.strip_prefix(interpreter) {
        Some(rest) => rest.starts_with(|c: char| c.is_ascii_digit() || c == '.'),
        None => false,
    }
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
pub fn is_interpreter(binary_path: &str) -> bool {
    let Some(name) = binary_path.rsplit('/').next() else {
        return false;
    };
    INTERPRETER_TABLE
        .iter()
        .any(|interpreter| interpreter_name_matches(name, interpreter))
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
fn strip_wrappers(argv: &[String]) -> Vec<String> {
    let mut rest: Vec<String> = argv.to_vec();
    while let Some(head) = rest.first() {
        match file_name(head) {
            "env" => {
                rest.remove(0);
                while rest
                    .first()
                    .is_some_and(|arg| arg == "-i" || arg.contains('='))
                {
                    rest.remove(0);
                }
            }
            "xargs" => {
                rest.remove(0);
                while rest.first().is_some_and(|arg| arg.starts_with('-')) {
                    rest.remove(0);
                }
            }
            "find" => {
                let Some(exec) = rest.iter().position(|arg| arg == "-exec") else {
                    return rest;
                };
                rest = rest[(exec + 1)..].to_vec();
                while rest.last().is_some_and(|arg| arg == ";" || arg == "+") {
                    rest.pop();
                }
            }
            _ => break,
        }
    }
    rest
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
fn capture_code_payload(argv: &[String]) -> Option<String> {
    argv.windows(2).find_map(|pair| {
        if pair[0] == "-c" || pair[0] == "-e" {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
// L3-GAP: [pe_17_shell_c_command_unwrap_drops_git_push_force]: flag/value-aware argv parse (-c/-m payloads, relative script resolution against event.cwd); slop: shell -c payload never unwrapped, git push passes
pub fn unwrap_interpreter(event: &TrapEvent) -> UnwrappedExec {
    if let Some(terminal) = event.interpreter_chain.last() {
        return UnwrappedExec {
            binary_path: terminal.path.clone(),
            argv: event.argv.clone(),
            shell_payload: event.shell_payload.clone(),
        };
    }
    let argv = strip_wrappers(&event.argv);
    let binary_path = argv
        .first()
        .cloned()
        .unwrap_or_else(|| event.binary_path.clone());
    let shell_payload = if is_interpreter(&binary_path) {
        capture_code_payload(&argv).or(event.shell_payload.clone())
    } else {
        event.shell_payload.clone()
    };
    UnwrappedExec {
        binary_path,
        argv,
        shell_payload,
    }
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
pub fn resolve_shebang(path: &str) -> Result<String, PluginError> {
    // TODO (L3) [pe_17_shell_c_command_unwrap_drops_git_push_force]: read the script shebang and resolve the interpreter path; slop: script interpreters never resolve from the shebang; wrapped execs pass without matching the policy row.
    Err(PluginError::Halt(format!(
        "shebang resolution pending (L3): {path}"
    )))
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
// see: blindess.patch.md#C: recursion depth cap for interpreter-chain unwrapping.
/// Maximum interpreter-chain unwrapping depth; chains deeper than this are
/// treated as obfuscation and fail closed (deny) instead of unwrapping further.
pub const INTERPRETER_UNWRAP_DEPTH_LIMIT: usize = 8;

// see: blindess.patch.md#C: recursive unwrapping must terminate; an attacker can
// nest interpreter wrappers (env xargs find env ... sh -c ...) arbitrarily deep.
/// Pure recursion guard for interpreter unwrapping. Returns `Ok` only when
/// `depth` is within the cap; deeper chains fail closed with a `Halt` so the
/// trap is denied rather than passed unmediated.
pub fn unwrap_depth_ok(depth: usize) -> Result<bool, PluginError> {
    if depth >= INTERPRETER_UNWRAP_DEPTH_LIMIT {
        // Fail closed: nested-wrapper chains beyond the cap are treated as
        // obfuscation and the exec is denied, never passed.
        Err(PluginError::Halt(format!(
            "interpreter unwrap depth {depth} exceeds cap {INTERPRETER_UNWRAP_DEPTH_LIMIT}"
        )))
    } else {
        Ok(true)
    }
}

// see: cans/plugins/supply-chain.md#Interpreter-unwrapping-and-payload-inspection
// L3-GAP: [supply_chain_security]: opaque shell AST fallback; when the shell payload
// cannot be parsed into a concrete AST, the fallback must drop to child execve traps
// (each spawned child is trapped and mediated individually) rather than trusting the
// unparsed payload; slop: opaque payloads pass unmediated because no AST row matched.
// TODO (L3) [supply_chain_security]: parse opaque shell payloads into an AST and, when
// parsing fails, fall back to mediating each child execve trap spawned by the shell; slop:
// opaque payloads pass unmediated, unparsable shell commands execute without child-trap mediation.

// see: blindess.patch.md#C: strip @scope/ prefixes for external advisory lookups.
/// Pure helper that strips an npm-style `@scope/` prefix from a package spec so
/// external advisory lookups (OSV/Snyk/Socket/deps.dev) query the bare package
/// name, which is what those registries index.
pub fn strip_private_scope(pkg: &str) -> String {
    // Only a leading scope is stripped: "@scope/pkg" -> "pkg"; a bare "pkg"
    // (or an already-stripped name) is returned unchanged.
    match pkg.strip_prefix('@') {
        Some(rest) => match rest.split_once('/') {
            Some((_scope, name)) => name.to_string(),
            None => pkg.to_string(),
        },
        None => pkg.to_string(),
    }
}

// see: blindess.patch.md#C: BusyBox applet argv[0] attestation check stub.
/// Pure BusyBox applet attestation stub: BusyBox dispatches applets by `argv[0]`
/// (e.g. `/bin/ls` -> busybox "ls"), so the invoked applet must be attested
/// against the resolved binary, not the symlinked name. The L2 shape returns
/// whether the applet is attestable from `argv[0]`; actual digest attestation is L3.
pub fn busybox_applet_attested(argv0: &str) -> bool {
    // Pure placeholder: with no attestation catalog at L2, an argv[0]-only
    // check cannot confirm the applet identity, so it fails closed.
    let _ = argv0;
    // TODO (L3) [supply_chain_security]: attest BusyBox applets by resolving argv[0] against the BusyBox applet table and the binary digest; slop: argv[0] is trusted as-is, a swapped busybox binary or spoofed applet name dispatches unattested.
    false
}

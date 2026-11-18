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

// see: cans/15-operations.md#Developer-CLI
#![forbid(unsafe_code)]

#[path = "attach_cli.flow.rs"]
pub mod attach_cli;
#[path = "attest_cli.flow.rs"]
pub mod attest_cli;
#[path = "audit_cli.flow.rs"]
pub mod audit_cli;
#[path = "budgets_cli.flow.rs"]
pub mod budgets_cli;
#[path = "bump_core_cli.flow.rs"]
pub mod bump_core_cli;
#[path = "cli_error.flow.rs"]
pub mod cli_error;
#[path = "dispatch.flow.rs"]
mod dispatch;
#[path = "doctor_cli.flow.rs"]
pub mod doctor_cli;
#[path = "emergency_cli.flow.rs"]
pub mod emergency_cli;
#[path = "fixtures_cli.flow.rs"]
pub mod fixtures_cli;
#[path = "graph_cli.flow.rs"]
pub mod graph_cli;
#[path = "layers_cli.flow.rs"]
pub mod layers_cli;
pub mod logs_cli;
pub mod manifest_cli;
#[path = "parity_cli.flow.rs"]
pub mod parity_cli;
pub mod registry_cli;
#[path = "report_cli.flow.rs"]
pub mod report_cli;
#[path = "rings_cli.flow.rs"]
pub mod rings_cli;
#[path = "scaffold_cli.flow.rs"]
pub mod scaffold_cli;
#[path = "setup_cli.flow.rs"]
pub mod setup_cli;
#[path = "spec_cli.flow.rs"]
pub mod spec_cli;
#[path = "vectors_cli.flow.rs"]
pub mod vectors_cli;

use crate::cli_error::CliError;

// see: cans/15-operations.md#Local-velocity
// KELIR_PLUGIN_CLI_ROOT overrides the baked-in workspace root so pe_01/pe_22
// can drive `check --layers` against a mock tree from a temp dir.
pub(crate) fn workspace_root() -> Result<std::path::PathBuf, CliError> {
    if let Some(root) = std::env::var_os("KELIR_PLUGIN_CLI_ROOT") {
        let root = std::path::PathBuf::from(root);
        if !root.as_os_str().is_empty() {
            return Ok(root);
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("kelir-plugin-cli: cannot locate the workspace root from the manifest dir".into())
        .map(std::path::Path::to_path_buf)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if let Err(err) = dispatch::dispatch(&args) {
        eprintln!("{err}");
        return err.exit_code();
    }
    std::process::ExitCode::SUCCESS
}

// see: cans/15-operations.md#Developer-CLI
#![forbid(unsafe_code)]

pub mod attach_cli;
#[path = "attest_cli.flow.rs"]
pub mod attest_cli;
#[path = "audit_cli.run.rs"]
pub mod audit_cli;
#[path = "budgets_cli.run.rs"]
pub mod budgets_cli;
#[path = "bump_core_cli.run.rs"]
pub mod bump_core_cli;
#[path = "cli_error.run.rs"]
pub mod cli_error;
#[path = "dispatch.flow.rs"]
mod dispatch;
#[path = "doctor_cli.run.rs"]
pub mod doctor_cli;
#[path = "fixtures_cli.run.rs"]
pub mod fixtures_cli;
#[path = "graph_cli.run.rs"]
pub mod graph_cli;
#[path = "layers_cli.run.rs"]
pub mod layers_cli;
pub mod logs_cli;
pub mod manifest_cli;
#[path = "parity_cli.run.rs"]
pub mod parity_cli;
pub mod registry_cli;
#[path = "rings_cli.run.rs"]
pub mod rings_cli;
pub mod scaffold_cli;
#[path = "setup_cli.flow.rs"]
pub mod setup_cli;
#[path = "spec_cli.run.rs"]
pub mod spec_cli;
#[path = "vectors_cli.run.rs"]
pub mod vectors_cli;

use crate::cli_error::CliError;

// see: cans/15-operations.md#Local-velocity
pub(crate) fn workspace_root() -> Result<std::path::PathBuf, CliError> {
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

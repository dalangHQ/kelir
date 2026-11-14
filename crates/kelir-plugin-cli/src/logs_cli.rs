// see: cans/15-operations.md#Developer-CLI
// see: cans/04-terminal-plane.md#Stream-replay
// L3-GAP: [terminal_plane]: tail the session output cursor from index.bin.; slop: tail Ok, index.bin cursor never advanced
use crate::cli_error::CliError;
pub fn run() -> Result<(), CliError> {
    Err("logs: not implemented (L3): tailing session output needs the index.bin cursor".into())
}

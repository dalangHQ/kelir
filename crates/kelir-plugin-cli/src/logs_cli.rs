// see: cans/15-operations.md#Developer-CLI
// see: cans/04-terminal-plane.md#Stream-replay
// TODO: [terminal_plane]: tail the session output cursor from index.bin.; slop: tail Ok, index.bin cursor never advanced (deferred to L3)
use crate::cli_error::CliError;
pub fn run() -> Result<(), CliError> {
 Err("logs: not implemented: tailing session output needs the index.bin cursor".into())
}

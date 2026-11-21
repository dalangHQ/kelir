// see: cans/15-operations.md#Subcommands
// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
use crate::cli_error::CliError;

// TODO (L3) [plugin_engine]: emit the auto-generated manifest artifact from the plugin AST.; slop: hand-written manifest passes, AST artifact never emitted
pub fn run() -> Result<(), CliError> {
    Err("manifest: not implemented: AST manifest extraction per cans/02-plugin-model.md#Auto-generated-manifest-contract".into())
}

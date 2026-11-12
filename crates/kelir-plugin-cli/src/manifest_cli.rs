// see: cans/15-operations.md#Subcommands
// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
// TODO (L3) [plugin_engine]: emit the auto-generated manifest artifact from the plugin AST.; slop: hand-written manifest passes, AST artifact never emitted
use crate::cli_error::CliError;
pub fn run() -> Result<(), CliError> {
    Err(
        "manifest: not implemented (L3): AST manifest extraction per cans/02-plugin-model.md#Auto-generated-manifest-contract"
            .into(),
    )
}

// see: cans/13-cli.md#Incident-and-emergency-commands
// see: cans/15-operations.md#Developer-CLI
use crate::cli_error::CliError;

// see: cans/13-cli.md#Incident-and-emergency-commands
// `kelir report <session>` emits a human-readable markdown summary of blocks,
// anomalies, and the timeline without requiring an external OTel/SIEM stack.
pub fn run(args: &[String]) -> Result<(), CliError> {
    let session = args.first().map(String::as_str).ok_or_else(|| {
        CliError::usage("report requires a session id argument: report <session>")
    })?;
    if session.is_empty() {
        return Err(CliError::usage(
            "report requires a non-empty session id argument",
        ));
    }
    if args.len() > 1 {
        return Err(CliError::usage(
            "report takes exactly one session id and no further flags",
        ));
    }
    // TODO (L3) [telemetry_audit]: aggregate the ExecutionReceipt and EventBus audit streams of session `<session>` into the executive markdown summary — blocks, anomalies, and timeline — without requiring an external OTel/SIEM stack; slop: report renders an empty summary, incident findings silently missing
    Err(format!(
        "report: markdown findings summary of session {session} needs the audit stream (L3)"
    ).into())
}

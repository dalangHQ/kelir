// see: cans/15-operations.md#Developer-CLI
use crate::cli_error::CliError;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRequest {
    pub session: String,
    pub json: bool,
}

pub fn parse_request(args: &[String]) -> Result<ReportRequest, CliError> {
    let mut session: Option<String> = None;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other if other.starts_with('-') => {
                return Err(CliError::usage(format!(
                    "report: unknown flag `{other}` (expected --json)"
                )));
            }
            other => {
                if session.is_some() {
                    return Err(CliError::usage(
                        "report takes exactly one session id: report <session> [--json]",
                    ));
                }
                session = Some(other.to_string());
            }
        }
    }
    let Some(session) = session else {
        return Err(CliError::usage(
            "report requires a session id: report <session> [--json]",
        ));
    };
    if session.trim().is_empty() {
        return Err(CliError::usage("report: session id must not be empty"));
    }
    Ok(ReportRequest { session, json })
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    let request = parse_request(args)?;
    // TODO: [telemetry_audit]: emit human-readable findings and timeline markdown from the session audit log.; slop: report Ok, findings never rendered from the audit log (deferred to L3)
    // TODO: [telemetry_audit]: render the session findings and timeline markdown from the ledger audit log; slop: report exits without findings, the audit log is never read (deferred to L3)
    Err(format!(
        "report: emitting findings and timeline markdown for session `{}`",
        request.session
    )
    .into())
}

// see: cans/13-cli.md#Incident-and-emergency-commands
// see: cans/08-lifecycle.md#Break-glass
use crate::cli_error::CliError;

// see: cans/08-lifecycle.md#Break-glass
// Fleet kill-switch halts active sessions and blocks new boots globally;
// break-glass requires MFA, reason, and expiry TTL.
pub fn run(args: &[String]) -> Result<(), CliError> {
    let command = args.first().map(String::as_str).ok_or_else(|| {
        CliError::usage("emergency requires a subcommand: kill-switch or break-glass")
    })?;
    match command {
        "kill-switch" => {
            if args.len() > 1 {
                return Err(CliError::usage(
                    "emergency kill-switch takes no further flags",
                ));
            }
            // TODO (L3) [telemetry_audit]: flip the global runtime halt state — halt every active session and reject new boots fleet-wide, binding the transition to an OCSF 1.x security-finding event; slop: kill-switch reported engaged, sessions keep running
            Err(
                "emergency kill-switch: global runtime halt control needs the fleet control plane (L3)"
                    .into(),
            )
        }
        "break-glass" => {
            // see: cans/08-lifecycle.md#Break-glass
            // Timed policy break-glass overrides require MFA, reason, and expiry TTL.
            if args.len() != 2 {
                return Err(CliError::usage(
                    "emergency break-glass takes exactly one TTL duration argument (e.g. 10m)",
                ));
            }
            let ttl = &args[1];
            if ttl.is_empty() {
                return Err(CliError::usage(
                    "emergency break-glass TTL must not be empty",
                ));
            }
            // TODO (L3) [telemetry_audit]: open the timed break-glass override — verify MFA (TOTP/WebAuthn/OIDC), record the operator reason, and arm the expiry TTL that auto-revokes the override; slop: override accepted without MFA, permanent bypass never expires
            Err(format!(
                "emergency break-glass: timed MFA-gated override of TTL {ttl} needs the auth surface (L3)"
            ).into())
        }
        other => Err(CliError::usage(format!(
            "emergency: unknown subcommand `{other}` (expected kill-switch or break-glass)"
        ))),
    }
}

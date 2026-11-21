// see: cans/08-lifecycle.md#Break-glass
use crate::cli_error::CliError;

// see: cans/08-lifecycle.md#Break-glass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyAction {
    KillSwitch,
    BreakGlass,
}

impl EmergencyAction {
    pub fn name(&self) -> &'static str {
        match self {
            EmergencyAction::KillSwitch => "kill-switch",
            EmergencyAction::BreakGlass => "break-glass",
        }
    }
}

// see: cans/08-lifecycle.md#Break-glass
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmergencyRequest {
    pub action: EmergencyAction,
    pub reason: Option<String>,
    pub ttl_seconds: Option<u64>,
}

pub fn parse_request(args: &[String]) -> Result<EmergencyRequest, CliError> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err(CliError::usage(
            "emergency requires an action: emergency <kill-switch|break-glass>",
        ));
    };
    let action = match action {
        "kill-switch" => EmergencyAction::KillSwitch,
        "break-glass" => EmergencyAction::BreakGlass,
        other => {
            return Err(CliError::usage(format!(
                "emergency: unknown action `{other}` (expected kill-switch or break-glass)"
            )));
        }
    };
    let mut reason: Option<String> = None;
    let mut ttl_seconds: Option<u64> = None;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--reason" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(CliError::usage(
                        "emergency break-glass: --reason requires a value",
                    ));
                };
                reason = Some(value.clone());
                index += 2;
            }
            "--ttl-seconds" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(CliError::usage(
                        "emergency break-glass: --ttl-seconds requires a value",
                    ));
                };
                ttl_seconds = Some(value.parse().map_err(|_| {
                    CliError::usage(format!(
                        "emergency break-glass: --ttl-seconds `{value}` is not a u64"
                    ))
                })?);
                index += 2;
            }
            other => {
                return Err(CliError::usage(format!(
                    "emergency: unknown argument `{other}` (expected --reason <text> --ttl-seconds <n> for break-glass)"
                )));
            }
        }
    }
    if action == EmergencyAction::BreakGlass {
        let Some(reason) = &reason else {
            return Err(CliError::usage(
                "emergency break-glass requires --reason <text>: a timed override without a reason never opens",
            ));
        };
        if reason.trim().is_empty() {
            return Err(CliError::usage(
                "emergency break-glass: --reason must not be empty",
            ));
        }
        match ttl_seconds {
            Some(0) | None => {
                return Err(CliError::usage(
                    "emergency break-glass requires --ttl-seconds <n> with n > 0: a timed override without an expiry never opens",
                ));
            }
            Some(_) => {}
        }
    }
    Ok(EmergencyRequest {
        action,
        reason,
        ttl_seconds,
    })
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    let request = parse_request(args)?;
    // TODO: [governance_tools]: halt active sessions and block new boots globally (kill-switch) or open a timed, MFA-gated override (break-glass).; slop: emergency Ok, global halt never engages and the override never expires (deferred to L3)
    // TODO: [governance_tools]: bind the kill-switch / break-glass request to the runtime halt state and the OCSF audit event; slop: emergency exits without halting, sessions keep running (deferred to L3)
    Err(format!("emergency: {}", request.action.name()).into())
}

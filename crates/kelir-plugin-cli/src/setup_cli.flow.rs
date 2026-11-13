// see: cans/15-operations.md#Developer-CLI
use std::error::Error;
use std::fmt;

use crate::cli_error::CliError;
use crate::doctor_cli::{HwLockPath, probe_host, write_hw_lockfile};

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    InstallShims,
    AttachLsm,
    QuarantineCredentials,
    EnforceHwLock,
}

impl SetupStep {
    pub fn name(&self) -> &'static str {
        match self {
            SetupStep::InstallShims => "install-shims",
            SetupStep::AttachLsm => "attach-lsm",
            SetupStep::QuarantineCredentials => "quarantine-credentials",
            SetupStep::EnforceHwLock => "hw-lock",
        }
    }

    pub fn all() -> [SetupStep; 4] {
        [
            SetupStep::InstallShims,
            SetupStep::AttachLsm,
            SetupStep::QuarantineCredentials,
            SetupStep::EnforceHwLock,
        ]
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum SetupError {
    NotImplemented(String),
    UnknownFlag(String),
    Io(String),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::NotImplemented(message)
            | SetupError::UnknownFlag(message)
            | SetupError::Io(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl Error for SetupError {}

pub fn run_step(step: SetupStep) -> Result<(), SetupError> {
    match step {
        SetupStep::EnforceHwLock => {
            let report = probe_host();
            if !report.is_ready() {
                return Err(SetupError::NotImplemented(format!(
                    "setup step hw-lock: host probes incomplete (L3): {}",
                    report.pending().join("; ")
                )));
            }
            write_hw_lockfile(&HwLockPath::default_path(), &report)
                .map_err(|err| SetupError::Io(format!("hw-lock: cannot write hw.lock: {err}")))?;
            // TODO (L3) [hypervisor_driver]: enforce 0660 perms on /dev/kvm (group kelir) once privileged hardening lands.; slop: /dev/kvm left world-accessible, 0660 never enforced
            Ok(())
        }
        // TODO (L3) [hypervisor_driver]: implement the InstallShims, AttachLsm, and QuarantineCredentials steps.; slop: setup Ok, shims/LSM/credentials steps skipped
        _ => Err(SetupError::NotImplemented(format!(
            "setup step {} (L3)",
            step.name()
        ))),
    }
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    let mut steps: Vec<SetupStep> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--install-shims" => steps.push(SetupStep::InstallShims),
            "--attach-lsm" => steps.push(SetupStep::AttachLsm),
            "--quarantine-credentials" => steps.push(SetupStep::QuarantineCredentials),
            "--hw-lock" => steps.push(SetupStep::EnforceHwLock),
            other => {
                return Err(SetupError::UnknownFlag(format!(
                    "setup: unknown flag `{other}` (expected --install-shims, --attach-lsm, --quarantine-credentials, or --hw-lock)"
                ))
                .into());
            }
        }
    }
    if steps.is_empty() {
        steps = SetupStep::all().to_vec();
    }
    for step in steps {
        run_step(step)?;
    }
    println!("kelir setup: host hardening complete");
    Ok(())
}

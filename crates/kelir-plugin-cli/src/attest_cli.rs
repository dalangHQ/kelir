// see: cans/15-operations.md#Developer-CLI
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestPlan {
    pub plugin_path: PathBuf,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum AttestError {
    MissingSign(String),
    MissingPluginPath(String),
    UnknownFlag(String),
}

impl fmt::Display for AttestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttestError::MissingSign(message)
            | AttestError::MissingPluginPath(message)
            | AttestError::UnknownFlag(message) => write!(f, "{message}"),
        }
    }
}

impl Error for AttestError {}

// see: cans/15-operations.md#Developer-CLI
pub fn parse_attest_args(args: &[String]) -> Result<AttestPlan, AttestError> {
    let mut sign = false;
    let mut plugin_path: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--sign" => sign = true,
            "--plugin-path" => {
                let value = iter.next().ok_or_else(|| {
                    AttestError::MissingPluginPath(
                        "attest: --plugin-path requires a value".to_string(),
                    )
                })?;
                plugin_path = Some(PathBuf::from(value));
            }
            other => {
                return Err(AttestError::UnknownFlag(format!(
                    "attest: unknown flag `{other}` (expected --sign or --plugin-path <repo>)"
                )));
            }
        }
    }
    if !sign {
        return Err(AttestError::MissingSign(
            "attest: requires --sign".to_string(),
        ));
    }
    let plugin_path = plugin_path.ok_or_else(|| {
        AttestError::MissingPluginPath("attest: requires --plugin-path <repo>".to_string())
    })?;
    Ok(AttestPlan { plugin_path })
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let _plan = parse_attest_args(args)?;
    // TODO (L3) [attestation_receipts]: attest --sign signs attestation.json for the plugin repo at the parsed path.; slop: unsigned attestation.json accepted, signature never verified
    Err("attest: signing subcommand (L3)".into())
}

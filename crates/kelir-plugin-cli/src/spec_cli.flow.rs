// see: cans/15-operations.md#Developer-CLI
use crate::cli_error::CliError;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSource {
    Toml,
    Yaml,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AotReport {
    pub path: PathBuf,
    pub source: ProfileSource,
    pub syntax_ok: bool,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum SpecError {
    MissingFile(String),
    UnsupportedExtension(String),
    Parse(String),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::MissingFile(message)
            | SpecError::UnsupportedExtension(message)
            | SpecError::Parse(message) => write!(f, "{message}"),
        }
    }
}

impl Error for SpecError {}

pub fn profile_source(path: &std::path::Path) -> Result<ProfileSource, SpecError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => Ok(ProfileSource::Toml),
        Some("yaml") | Some("yml") => Ok(ProfileSource::Yaml),
        Some(other) => Err(SpecError::UnsupportedExtension(format!(
            "spec: unsupported profile extension `.{other}` (expected .toml or .yaml)"
        ))),
        None => Err(SpecError::UnsupportedExtension(
            "spec: profile path has no extension (expected .toml or .yaml)".to_string(),
        )),
    }
}

pub fn validate_profile_aot(path: &str) -> Result<AotReport, SpecError> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(SpecError::MissingFile(format!(
            "spec: profile not found at {path}"
        )));
    }
    let source = profile_source(&path_buf)?;
    let text = std::fs::read_to_string(&path_buf)
        .map_err(|e| SpecError::Parse(format!("spec: cannot read {path}: {e}")))?;
    match source {
        ProfileSource::Toml => {
            toml::from_str::<toml::Value>(&text)
                .map_err(|e| SpecError::Parse(format!("spec: {path} is not valid TOML: {e}")))?;
        }
        ProfileSource::Yaml => {
            serde_yaml::from_str::<serde_yaml::Value>(&text)
                .map_err(|e| SpecError::Parse(format!("spec: {path} is not valid YAML: {e}")))?;
        }
    }
    // L3-GAP: [plugin_engine]: validate the TypeBox/Serde schema, acyclic extends, and monotonic limits without compiling code.; slop: cyclic extends accepted, schema validation never runs
    Ok(AotReport {
        path: path_buf,
        source,
        syntax_ok: true,
    })
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        Some("check") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            let report = validate_profile_aot(path)?;
            let source = match report.source {
                ProfileSource::Toml => "TOML",
                ProfileSource::Yaml => "YAML",
            };
            println!(
                "spec: {path} {source} syntax valid; schema, acyclic extends, and monotonic limits pending (L3)"
            );
            Ok(())
        }
        Some("export") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            // L3-GAP: [plugin_engine]: emit the canonical kelir.toml or kelir.yaml for the profile.; slop: hand-written toml passes, canonical profile never emitted
            Err(format!("spec export: canonical export of {path} (L3)").into())
        }
        // see: cans/13-cli.md#Policy-generation-commands
        // see: cans/15-operations.md#Spec-validation
        Some("generate") => {
            let session = args.get(1).map(String::as_str).ok_or_else(|| {
                CliError::usage("spec generate requires a session id argument")
            })?;
            if session.is_empty() {
                return Err(CliError::usage(
                    "spec generate requires a non-empty session id argument",
                ));
            }
            if args.len() > 2 {
                return Err(CliError::usage(
                    "spec generate takes exactly one session id and no further flags",
                ));
            }
            // TODO (L3) [plugin_engine]: compile the observed ExecutionReceipt and EventBus logs of session `<session>` into a starter kelir.toml — collapse path globs, /24 egress CIDRs, and observed MCP tool hashes above their thresholds, flag risky commands, then emit the profile-then-lock starter policy; slop: synthesized profile drifts from the observed session, unsafe commands pass as allowed, starter policy never emitted
            Err(format!(
                "spec generate: synthesizing a starter profile from the audit log of session {session} (L3)"
            ).into())
        }
        Some(other) => Err(CliError::usage(format!(
            "spec: unknown subcommand `{other}` (expected check, export, or generate)"
        ))),
        None => Err(CliError::usage(
            "spec requires a subcommand: check <file>, export <file>, or generate <session>",
        )),
    }
}

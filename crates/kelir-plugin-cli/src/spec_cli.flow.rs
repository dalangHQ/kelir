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
    // TODO: [plugin_engine]: validate the TypeBox/Serde schema, acyclic extends, and monotonic limits without compiling code.; slop: cyclic extends accepted, schema validation never runs (deferred to L3)
    Ok(AotReport {
        path: path_buf,
        source,
        syntax_ok: true,
    })
}

// see: cans/15-operations.md#Developer-CLI
pub fn generate_profile(session: &str) -> Result<(), CliError> {
    if session.trim().is_empty() {
        return Err(CliError::usage(
            "spec generate requires a session id: spec generate <session>",
        ));
    }
    // TODO: [plugin_engine]: synthesize a starter kelir.toml from the session ExecutionReceipt and EventBus logs (policygen).; slop: generate Ok, starter profile never synthesized from observed behavior (deferred to L3)
    // TODO: [plugin_engine]: compile observed ExecutionReceipt + EventBus logs for the session into a starter profile; slop: hand-written profile passes, observed behavior never compiled into one (deferred to L3)
    Err(
 format!("spec generate: synthesizing a starter profile from session `{session}`")
            .into(),
    )
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        Some("generate") => {
            let Some(session) = args.get(1).map(String::as_str) else {
                return Err(CliError::usage(
                    "spec generate requires a session id: spec generate <session>",
                ));
            };
            if args.len() > 2 {
                return Err(CliError::usage(format!(
                    "spec generate takes exactly one session id, got `{}`",
                    args[2]
                )));
            }
            generate_profile(session)
        }
        Some("check") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            let report = validate_profile_aot(path)?;
            let source = match report.source {
                ProfileSource::Toml => "TOML",
                ProfileSource::Yaml => "YAML",
            };
            println!(
 "spec: {path} {source} syntax valid; schema, acyclic extends, and monotonic limits pending"
            );
            Ok(())
        }
        Some("export") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            // TODO: [plugin_engine]: emit the canonical kelir.toml or kelir.yaml for the profile.; slop: hand-written toml passes, canonical profile never emitted (deferred to L3)
 Err(format!("spec export: canonical export of {path}").into())
        }
        Some(other) => Err(CliError::usage(format!(
            "spec: unknown subcommand `{other}` (expected check, export, or generate)"
        ))),
        None => Err(CliError::usage(
            "spec requires a subcommand: check <file>, export <file>, or generate <session>",
        )),
    }
}

// see: cans/15-operations.md#Developer-CLI
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
    pub acyclic: bool,
    pub monotonic_limits: bool,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum SpecError {
    MissingFile(String),
    UnsupportedExtension(String),
    NotImplemented(String),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::MissingFile(message)
            | SpecError::UnsupportedExtension(message)
            | SpecError::NotImplemented(message) => write!(f, "{message}"),
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
    let _source = profile_source(&path_buf)?;
    // TODO (L3): validate TypeBox/Serde schema, acyclic extends, and monotonic limits without compiling code.
    Err(SpecError::NotImplemented(
        "spec check AOT validation (L3)".to_string(),
    ))
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("check") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            let _report = validate_profile_aot(path)?;
            println!("spec: {path} AOT validation passed");
            Ok(())
        }
        Some("export") => {
            let path = args.get(1).map(String::as_str).unwrap_or("kelir.toml");
            // TODO (L3): emit the canonical kelir.toml or kelir.yaml for the profile.
            Err(format!("spec export: canonical export of {path} (L3)").into())
        }
        Some(other) => {
            Err(format!("spec: unknown subcommand `{other}` (expected check or export)").into())
        }
        None => Err("spec requires a subcommand: check <file> or export <file>".into()),
    }
}

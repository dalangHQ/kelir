// see: cans/15-operations.md#Developer-CLI
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use sha2::Digest;

use kelir_interface::hex_codec::encode_hex;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureEntry {
    pub name: String,
    pub path: String,
    pub blake3: Option<String>,
    pub sha256: Option<String>,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestMismatch {
    pub name: String,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for DigestMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fixture {}: digest mismatch: expected {}, got {}",
            self.name, self.expected, self.actual
        )
    }
}

impl Error for DigestMismatch {}

pub fn compare_digest(name: &str, expected: &str, actual: &str) -> Result<(), DigestMismatch> {
    if expected == actual {
        Ok(())
    } else {
        Err(DigestMismatch {
            name: name.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum FixtureError {
    Read(String),
    Parse(String),
    MissingFile(String),
    NoDigestPinned(String),
    DigestMismatch(String),
    HashUnavailable(String),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixtureError::Read(message)
            | FixtureError::Parse(message)
            | FixtureError::MissingFile(message)
            | FixtureError::NoDigestPinned(message)
            | FixtureError::DigestMismatch(message)
            | FixtureError::HashUnavailable(message) => write!(f, "{message}"),
        }
    }
}

impl Error for FixtureError {}

fn strip_quotes(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

pub fn parse_fixtures(text: &str) -> Result<Vec<FixtureEntry>, FixtureError> {
    let mut entries: Vec<FixtureEntry> = Vec::new();
    let mut current: Option<FixtureEntry> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            let section = &line[1..line.len() - 1];
            if let Some(name) = section.strip_prefix("fixtures.") {
                if name.is_empty() || name.contains('.') {
                    return Err(FixtureError::Parse(format!(
                        "fixtures: bad section `{line}`"
                    )));
                }
                current = Some(FixtureEntry {
                    name: name.to_string(),
                    path: String::new(),
                    blake3: None,
                    sha256: None,
                });
            }
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(value) = strip_quotes(value.trim()) else {
            continue;
        };
        match key.trim() {
            "path" => entry.path = value,
            "blake3" => entry.blake3 = Some(value),
            "sha256" => entry.sha256 = Some(value),
            _ => {}
        }
    }
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
    for entry in &entries {
        if entry.path.is_empty() {
            return Err(FixtureError::Parse(format!(
                "fixtures: entry `{}` pins no path",
                entry.name
            )));
        }
    }
    Ok(entries)
}

fn verify_entry(entry: &FixtureEntry) -> Result<(), FixtureError> {
    let bytes = fs::read(&entry.path).map_err(|e| {
        FixtureError::MissingFile(format!(
            "fixture {}: file missing at {}: {e}",
            entry.name, entry.path
        ))
    })?;
    if entry.blake3.is_none() && entry.sha256.is_none() {
        return Err(FixtureError::NoDigestPinned(format!(
            "fixture {}: pins no blake3 or sha256 digest",
            entry.name
        )));
    }
    if let Some(expected) = &entry.blake3 {
        let actual = encode_hex(blake3::hash(&bytes).as_bytes());
        compare_digest(&entry.name, expected, &actual)
            .map_err(|mismatch| FixtureError::DigestMismatch(mismatch.to_string()))?;
    }
    if let Some(expected) = &entry.sha256 {
        let mut hasher = sha2::Sha256::new();
        hasher.update(&bytes);
        let actual = encode_hex(&hasher.finalize());
        compare_digest(&entry.name, expected, &actual)
            .map_err(|mismatch| FixtureError::DigestMismatch(mismatch.to_string()))?;
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args.get(1).map(String::as_str).unwrap_or("fixtures.toml");
    match args.first().map(String::as_str) {
        Some("--verify") => {
            let text = fs::read_to_string(path)
                .map_err(|e| FixtureError::Read(format!("fixtures: cannot read {path}: {e}")))?;
            let entries = parse_fixtures(&text)?;
            for entry in &entries {
                verify_entry(entry)?;
            }
            println!("fixtures: {} fixture(s) verified in {path}", entries.len());
            Ok(())
        }
        Some("--fetch") => {
            let text = fs::read_to_string(path)
                .map_err(|e| FixtureError::Read(format!("fixtures: cannot read {path}: {e}")))?;
            let entries = parse_fixtures(&text)?;
            for entry in &entries {
                if !Path::new(&entry.path).exists() {
                    // TODO (L3): download the pinned fixture from its source registry, then verify its digest before mounting.
                    return Err(FixtureError::HashUnavailable(format!(
                        "fixtures fetch (L3): `{}` is absent at {}; network fetch pending",
                        entry.name, entry.path
                    ))
                    .into());
                }
                verify_entry(entry)?;
            }
            println!(
                "fixtures: {} fixture(s) present and digest-verified in {path}",
                entries.len()
            );
            Ok(())
        }
        Some(other) => {
            Err(format!("fixtures: unknown flag `{other}` (expected --verify or --fetch)").into())
        }
        None => Err("fixtures requires --verify or --fetch".into()),
    }
}

// see: cans/15-operations.md#Developer-CLI
use std::error::Error;
use std::fmt;
use std::path::Path;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreChannel {
    Released,
    Main,
}

impl CoreChannel {
    // see: cans/15-operations.md#Developer-CLI
    pub fn parse(value: &str) -> Result<Self, BumpCoreError> {
        match value {
            "released" => Ok(CoreChannel::Released),
            "main" => Ok(CoreChannel::Main),
            other => Err(BumpCoreError::UnknownChannel(format!(
                "bump-core: unknown channel `{other}` (expected released or main)"
            ))),
        }
    }

    // see: cans/15-operations.md#Developer-CLI
    pub fn as_str(&self) -> &'static str {
        match self {
            CoreChannel::Released => "released",
            CoreChannel::Main => "main",
        }
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelPin {
    pub channel: CoreChannel,
    pub kelir_version: String,
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum BumpCoreError {
    MissingChannel(String),
    MissingValue(String),
    UnknownFlag(String),
    UnknownChannel(String),
    Io(String),
}

impl fmt::Display for BumpCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BumpCoreError::MissingChannel(message)
            | BumpCoreError::MissingValue(message)
            | BumpCoreError::UnknownFlag(message)
            | BumpCoreError::UnknownChannel(message)
            | BumpCoreError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl Error for BumpCoreError {}

// see: cans/15-operations.md#Developer-CLI
pub fn resolve_pin(channel: CoreChannel) -> Result<ChannelPin, BumpCoreError> {
    Ok(ChannelPin {
        channel,
        kelir_version: kelir_core::KELIR_CORE_VERSION.into(),
    })
}

// see: cans/15-operations.md#Developer-CLI
pub fn write_pin(pin: &ChannelPin, path: &Path) -> Result<(), BumpCoreError> {
    let json = serde_json::json!({
        "channel": pin.channel.as_str(),
        "kelir_version": pin.kelir_version,
    });
    let text = serde_json::to_string_pretty(&json)
        .map_err(|e| BumpCoreError::Io(format!("bump-core: cannot encode pin: {e}")))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                BumpCoreError::Io(format!(
                    "bump-core: cannot create {}: {e}",
                    parent.display()
                ))
            })?;
        }
    }
    std::fs::write(path, text)
        .map_err(|e| BumpCoreError::Io(format!("bump-core: cannot write {}: {e}", path.display())))
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut channel: Option<CoreChannel> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--channel" => {
                let value = iter.next().ok_or_else(|| {
                    BumpCoreError::MissingValue(
                        "bump-core: --channel requires a value (released or main)".to_string(),
                    )
                })?;
                channel = Some(CoreChannel::parse(value)?);
            }
            other => {
                return Err(BumpCoreError::UnknownFlag(format!(
                    "bump-core: unknown flag `{other}` (expected --channel <released|main>)"
                ))
                .into());
            }
        }
    }
    let channel = channel.ok_or_else(|| {
        BumpCoreError::MissingChannel("bump-core: requires --channel <released|main>".to_string())
    })?;
    let pin = resolve_pin(channel)?;
    write_pin(
        &pin,
        &crate::workspace_root()?.join(".kelir/core-channel.json"),
    )?;
    println!(
        "bump-core: pinned channel {} at kelir {}",
        pin.channel.as_str(),
        pin.kelir_version
    );
    Ok(())
}

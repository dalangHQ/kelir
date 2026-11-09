// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/15-operations.md#Per-plugin-CI
use std::collections::BTreeSet;
use std::path::Path;

use crate::layers;

// see: cans/02-plugin-model.md#Segregated-ring-traits
pub const RING_TRAITS: &[&str] = &[
    "LifecycleParticipant",
    "TrapInterposer",
    "ConsoleInterposer",
    "FsInterposer",
    "NetInterposer",
    "HostInterposer",
    "VsockServiceHandler",
    "SystemTelemetryParticipant",
];

const KELIR_PLUGIN_IDENTITY: &str = "KelirPlugin";

pub fn declared_ring_mask(dir: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    layers::collect_rs(dir, &mut files)?;
    let mut mask = BTreeSet::new();
    for file in files {
        let text = std::fs::read_to_string(&file)?;
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("impl ") else {
                continue;
            };
            let Some((trait_name, _)) = rest.split_once(" for ") else {
                continue;
            };
            let trait_name = trait_name.trim();
            if trait_name == KELIR_PLUGIN_IDENTITY || RING_TRAITS.contains(&trait_name) {
                mask.insert(trait_name.to_string());
            }
        }
    }
    Ok(mask)
}

pub fn declared_plugin_name(dir: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    layers::collect_rs(dir, &mut files)?;
    for file in files {
        let text = std::fs::read_to_string(&file)?;
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if !line.trim().starts_with("fn name(&self) -> &'static str") {
                continue;
            }
            for lookahead in lines.by_ref().take(10) {
                if let Some(literal) = quoted_literal(lookahead) {
                    return Ok(Some(literal));
                }
            }
        }
    }
    Ok(None)
}

fn quoted_literal(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO (L3): VirtIO ring inspection subcommand skeleton.
    Ok(())
}

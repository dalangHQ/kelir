// see: cans/15-operations.md#CI-matrix
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

const REQUIRED_KEYS: &[&str] = &[
    "gate_1_us",
    "gate_2_us",
    "gate_3_us",
    "cold_boot_ms",
    "rss_purge_ms",
    "fast_boot_hydration_us",
    "idle_memory_rss_mb",
];

// see: cans/15-operations.md#CI-matrix
const SPEC_CEILINGS: &[(&str, i64)] = &[
    ("gate_2_us", 2),
    ("cold_boot_ms", 60),
    ("rss_purge_ms", 50),
    ("fast_boot_hydration_us", 500),
    ("idle_memory_rss_mb", 4),
];

#[derive(Deserialize)]
struct BudgetsToml {
    enforce: bool,
    latencies: BTreeMap<String, i64>,
}

pub fn check(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("budgets: cannot read {}: {e}", path.display()))?;
    let budgets: BudgetsToml = toml::from_str(&text).map_err(|e| {
        format!(
            "budgets: {} is not a valid budgets file: {e}",
            path.display()
        )
    })?;
    if !budgets.enforce {
        return Err("budgets: `enforce` must be true".into());
    }
    for key in REQUIRED_KEYS {
        let Some(value) = budgets.latencies.get(*key) else {
            return Err(format!("budgets: missing required key `{key}`").into());
        };
        if let Some((_, ceiling)) = SPEC_CEILINGS.iter().find(|(spec_key, _)| spec_key == key)
            && *value > *ceiling
        {
            return Err(format!(
                "budgets: `{key}` = {value} exceeds the spec ceiling {ceiling} (cans/15-operations.md#CI-matrix)"
            )
            .into());
        }
    }
    // TODO (L3): measured p95 enforcement rides the benchmark suites per cans/15-operations.md#CI-matrix.
    println!(
        "budgets: {} enforce=true, {} required key(s) present, {} spec ceiling(s) hold",
        path.display(),
        REQUIRED_KEYS.len(),
        SPEC_CEILINGS.len()
    );
    Ok(())
}

// see: cans/15-operations.md#CI-matrix
use std::fs;

const REQUIRED_KEYS: &[&str] = &[
    "gate_1_us",
    "gate_2_us",
    "gate_3_us",
    "cold_boot_ms",
    "rss_purge_ms",
    "fast_boot_hydration_us",
    "idle_memory_rss_mb",
];

pub fn check(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path).map_err(|e| format!("budgets: cannot read {path}: {e}"))?;
    let mut enforce = false;
    let mut found: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "enforce" {
            if value != "true" {
                return Err("budgets: `enforce` must be true".into());
            }
            enforce = true;
        } else if REQUIRED_KEYS.contains(&key) {
            value.parse::<i64>().map_err(|_| {
                format!("budgets: `{key}` must have an integer value, found `{value}`")
            })?;
            found.push(key);
        }
    }
    if !enforce {
        return Err("budgets: missing `enforce = true`".into());
    }
    for key in REQUIRED_KEYS {
        if !found.contains(key) {
            return Err(format!("budgets: missing required key `{key}`").into());
        }
    }
    Ok(())
}

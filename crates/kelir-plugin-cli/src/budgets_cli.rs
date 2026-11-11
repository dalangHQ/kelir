// see: cans/15-operations.md#CI-matrix
use std::fs;
use std::path::Path;

use kelir_testkit::perf_budgets::{Budgets, validate_scorecard_budgets};

// see: cans/15-operations.md#CI-matrix
const SPEC_CEILINGS: &[(&str, u64)] = &[
    ("gate_2_us", 2),
    ("cold_boot_ms", 60),
    ("rss_purge_ms", 50),
    ("fast_boot_hydration_us", 500),
    ("idle_memory_rss_mb", 4),
];

pub fn check(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("budgets: cannot read {}: {e}", path.display()))?;
    let budgets: Budgets = toml::from_str(&text)
        .map_err(|e| format!("budgets: {} is not valid TOML: {e}", path.display()))?;
    if !budgets.enforce {
        return Err("budgets: `enforce` must be true".into());
    }
    validate_scorecard_budgets(&budgets)?;
    for (key, ceiling) in SPEC_CEILINGS {
        let Some(value) = budgets.latencies.get(*key) else {
            return Err(format!("budgets: missing required key `{key}`").into());
        };
        if *value > *ceiling {
            return Err(format!(
                "budgets: `{key}` = {value} exceeds the spec ceiling {ceiling} (cans/15-operations.md#CI-matrix)"
            )
            .into());
        }
    }
    // TODO (L3): measured p95 enforcement rides the benchmark suites per cans/15-operations.md#CI-matrix.
    println!(
        "budgets: {} enforce=true, {} spec ceiling(s) hold",
        path.display(),
        SPEC_CEILINGS.len()
    );
    Ok(())
}

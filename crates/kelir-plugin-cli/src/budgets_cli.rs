// see: cans/15-operations.md#CI-matrix
use std::fs;

use kelir_testkit::perf_budgets::{Budgets, validate_scorecard_budgets};

pub fn check(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path).map_err(|e| format!("budgets: cannot read {path}: {e}"))?;
    let budgets: Budgets =
        toml::from_str(&text).map_err(|e| format!("budgets: {path} is not valid TOML: {e}"))?;
    if !budgets.enforce {
        return Err("budgets: `enforce` must be true".into());
    }
    validate_scorecard_budgets(&budgets)?;
    Ok(())
}

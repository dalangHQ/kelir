// see: cans/15-operations.md#Coverage-gates
use std::collections::BTreeMap;

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
pub struct Budgets {
    pub enforce: bool,
    pub latencies: BTreeMap<String, u64>,
    pub scorecard: ScorecardTables,
}

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
pub struct ScorecardTables {
    pub rules: Vec<ScorecardRule>,
}

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
pub struct ScorecardRule {
    pub id: String,
    pub rule: String,
    pub mechanism: String,
    pub status: String,
}

// see: cans/15-operations.md#Coverage-gates
pub fn validate_scorecard_budgets(budgets: &Budgets) -> Result<(), String> {
    if budgets.scorecard.rules.is_empty() {
        return Err("budgets: [[scorecard.rules]] is empty".into());
    }
    for rule in &budgets.scorecard.rules {
        if rule.id.is_empty()
            || rule.rule.is_empty()
            || rule.mechanism.is_empty()
            || rule.status.is_empty()
        {
            return Err(format!("budgets: rule `{}` has an empty field", rule.id));
        }
    }
    Ok(())
}

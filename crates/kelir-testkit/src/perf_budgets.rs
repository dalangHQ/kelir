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
    // TODO (L3) [governance_tools]: scorecard budget validation pending; slop: scorecard rules unvalidated
    let _ = budgets;
    Err("TODO (L3) [governance_tools]: scorecard budget validation pending; slop: scorecard rules unvalidated".into())
}

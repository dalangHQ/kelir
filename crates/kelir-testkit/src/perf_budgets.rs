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
    pub floors: Vec<ScorecardFloor>,
    pub rules: Vec<ScorecardRule>,
}

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
pub struct ScorecardFloor {
    pub scenario: String,
    pub budget: Option<String>,
    pub value: Option<u64>,
    pub unit: Option<String>,
    pub target: Option<String>,
    pub enforcement: String,
    pub status: String,
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
    if budgets.scorecard.floors.is_empty() {
        return Err("budgets: [[scorecard.floors]] is empty".into());
    }
    for floor in &budgets.scorecard.floors {
        let declared = usize::from(floor.budget.is_some())
            + usize::from(floor.value.is_some())
            + usize::from(floor.target.is_some());
        if declared != 1 {
            return Err(format!(
                "budgets: floor `{}` must declare exactly one of budget, value, or target",
                floor.scenario
            ));
        }
        if floor.target.is_none() && floor.unit.as_deref().is_none_or(str::is_empty) {
            return Err(format!(
                "budgets: floor `{}` must declare a unit",
                floor.scenario
            ));
        }
        if let Some(key) = &floor.budget {
            if !budgets.latencies.contains_key(key) {
                return Err(format!(
                    "budgets: floor `{}` references unknown [latencies] key `{}`",
                    floor.scenario, key
                ));
            }
        }
    }
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

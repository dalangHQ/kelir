// see: cans/15-operations.md#Coverage-gates
use crate::perf_budgets_contract::Budgets;

pub use crate::perf_budgets_contract::Budgets;

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

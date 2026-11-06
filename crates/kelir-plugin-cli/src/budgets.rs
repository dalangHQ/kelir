// see: cans/15-operations.md#CI-matrix
pub fn check(_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Assert latency and RSS budgets from perf/budgets.toml
    Ok(())
}

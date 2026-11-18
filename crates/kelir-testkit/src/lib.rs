// see: cans/_rules.yaml#slug
#![forbid(unsafe_code)]
pub mod case_map;
#[path = "perf_budgets.flow.rs"]
pub mod perf_budgets;
#[path = "perf_budgets.rs"]
pub mod perf_budgets_contract;
#[path = "spec_anchors.run.rs"]
pub mod spec_anchors;

pub use case_map::{CaseMap, HarnessTest, MapEntry};
pub use spec_anchors::AnchorIndex;

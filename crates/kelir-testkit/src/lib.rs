// see: cans/_rules.yaml#slug
#![forbid(unsafe_code)]
pub mod case_map;
pub mod perf_budgets;
#[path = "scenario_catalog.run.rs"]
pub mod scenario_catalog;
#[path = "spec_anchors.run.rs"]
pub mod spec_anchors;

pub use case_map::{CaseMap, HarnessTest, MapEntry};
pub use spec_anchors::AnchorIndex;

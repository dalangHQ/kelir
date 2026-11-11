// see: cans/_rules.yaml#slug
pub mod case_map;
pub mod perf_budgets;
pub mod scenario_catalog;
pub mod spec_anchors;

pub use case_map::{CaseMap, HarnessTest, MapEntry};
pub use spec_anchors::AnchorIndex;

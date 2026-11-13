#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Codebase-invariants
use regex::Regex;
use serde::Deserialize;

#[path = "levels.flow.rs"]
pub mod levels;
#[path = "policy.flow.rs"]
pub mod policy;
#[path = "structure.run.rs"]
pub mod structure;
#[path = "tree.run.rs"]
pub mod tree;

pub use levels::{crate_of, level_of};
pub use tree::{check_tree, read_allowed_levels, rs_files};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Level {
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForbidMode {
    Warn,
    Error,
}

#[derive(Debug)]
pub struct Rule {
    pub level: Level,
    pub forbid: bool,
    pub id: String,
    pub pattern: String,
    pub hint: Option<String>,
    pub regex: Regex,
}

#[derive(Debug)]
pub struct Policy {
    pub l2_max_delta_per_pr: u32,
    pub forbid_mode: ForbidMode,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default)]
pub struct CheckReport {
    pub files: usize,
    pub hard_fails: usize,
    pub fails: Vec<String>,
    pub warns: Vec<String>,
}

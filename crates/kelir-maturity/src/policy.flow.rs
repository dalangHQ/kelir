// see: cans/01-architecture.md#Codebase-invariants
use crate::{CheckReport, ForbidMode, Level, Policy, Rule};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct PolicyFile {
    #[serde(default)]
    budget: BudgetFile,
    #[serde(default)]
    policy: PolicyTableFile,
    #[serde(default)]
    level: BTreeMap<String, LevelRulesFile>,
}

#[derive(Debug, Default, Deserialize)]
struct PolicyTableFile {
    #[serde(default)]
    forbid_mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetFile {
    #[serde(default, rename = "L2_max_delta_per_pr")]
    l2_max_delta_per_pr: u32,
}

#[derive(Debug, Default, Deserialize)]
struct LevelRulesFile {
    #[serde(default)]
    forbid: Vec<RuleFile>,
    #[serde(default)]
    warn: Vec<RuleFile>,
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    id: String,
    pattern: String,
    #[serde(default)]
    hint: Option<String>,
}

impl Policy {
    pub fn from_toml(text: &str) -> Result<Policy, String> {
        let file: PolicyFile = match toml::from_str(text) {
            Ok(f) => f,
            Err(e) => return Err(e.to_string()),
        };
        let forbid_mode = match file.policy.forbid_mode.as_deref() {
            None => ForbidMode::Error,
            Some("error") => ForbidMode::Error,
            Some("warn") => ForbidMode::Warn,
            Some(other) => {
                return Err(format!(
                    "policy.forbid_mode: {other:?} (want \"warn\" or \"error\")"
                ))
            }
        };
        let mut rules = Vec::new();
        for (name, lr) in &file.level {
            let level = match Level::from_name(name) {
                Some(l) => l,
                None => continue,
            };
            for rf in &lr.forbid {
                rules.push(rule(level, true, rf)?);
            }
            for rf in &lr.warn {
                rules.push(rule(level, false, rf)?);
            }
        }
        Ok(Policy {
            l2_max_delta_per_pr: file.budget.l2_max_delta_per_pr,
            forbid_mode,
            rules,
        })
    }

    pub fn empty() -> Policy {
        Policy {
            l2_max_delta_per_pr: 0,
            forbid_mode: ForbidMode::Error,
            rules: Vec::new(),
        }
    }

    pub fn rules_for(&self, level: Level, forbid: bool) -> impl Iterator<Item = &Rule> {
        self.rules
            .iter()
            .filter(move |r| r.level == level && r.forbid == forbid)
    }
}

impl CheckReport {
    pub fn passed(&self) -> bool {
        self.fails.is_empty()
    }
}

fn rule(level: Level, forbid: bool, rf: &RuleFile) -> Result<Rule, String> {
    match Regex::new(&rf.pattern) {
        Ok(regex) => Ok(Rule {
            level,
            forbid,
            id: rf.id.clone(),
            pattern: rf.pattern.clone(),
            hint: rf.hint.clone(),
            regex,
        }),
        Err(e) => Err(format!("rule {}: {}", rf.id, e)),
    }
}

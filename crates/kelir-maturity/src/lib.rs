#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Codebase-invariants
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Level {
    L1,
    L2,
    L3,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::L1 => write!(f, "L1"),
            Level::L2 => write!(f, "L2"),
            Level::L3 => write!(f, "L3"),
        }
    }
}

impl Level {
    pub fn from_name(name: &str) -> Option<Level> {
        match name {
            "L1" => Some(Level::L1),
            "L2" => Some(Level::L2),
            "L3" => Some(Level::L3),
            _ => None,
        }
    }
}

pub fn level_of(path: &Path) -> Level {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => String::new(),
    };
    if name.ends_with(".flow.rs") {
        Level::L2
    } else if name.ends_with(".run.rs") {
        Level::L3
    } else {
        Level::L1
    }
}

pub fn crate_of(path: &Path) -> PathBuf {
    let mut dir = match path.parent() {
        Some(d) => d.to_path_buf(),
        None => return PathBuf::new(),
    };
    loop {
        if dir.join("Cargo.toml").is_file() {
            return dir;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => return PathBuf::new(),
        }
    }
}

pub fn read_allowed_levels(cargo_toml: &Path) -> Vec<Level> {
    let text = match fs::read_to_string(cargo_toml) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let array = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("kelir"))
        .and_then(|k| k.get("allowed_levels"))
        .and_then(|a| a.as_array());
    match array {
        Some(items) => items
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(Level::from_name)
            .collect(),
        None => Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
struct PolicyFile {
    #[serde(default)]
    budget: BudgetFile,
    #[serde(default)]
    level: BTreeMap<String, LevelRulesFile>,
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
    pub rules: Vec<Rule>,
}

impl Policy {
    pub fn from_toml(text: &str) -> Result<Policy, String> {
        let file: PolicyFile = match toml::from_str(text) {
            Ok(f) => f,
            Err(e) => return Err(e.to_string()),
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
            rules,
        })
    }

    pub fn empty() -> Policy {
        Policy {
            l2_max_delta_per_pr: 0,
            rules: Vec::new(),
        }
    }

    pub fn rules_for(&self, level: Level, forbid: bool) -> impl Iterator<Item = &Rule> {
        self.rules
            .iter()
            .filter(move |r| r.level == level && r.forbid == forbid)
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

#[derive(Debug, Default)]
pub struct CheckReport {
    pub files: usize,
    pub fails: Vec<String>,
    pub warns: Vec<String>,
}

impl CheckReport {
    pub fn passed(&self) -> bool {
        self.fails.is_empty()
    }
}

pub fn rs_files(root: &Path) -> Vec<PathBuf> {
    let base = {
        let crates = root.join("crates");
        if crates.is_dir() {
            crates
        } else {
            root.to_path_buf()
        }
    };
    let mut out = Vec::new();
    let walker = WalkDir::new(base)
        .into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && e.file_name() == "target"));
    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
    out.sort();
    out
}

pub fn check_tree(root: &Path, policy: &Policy) -> CheckReport {
    let mut report = CheckReport::default();
    for file in rs_files(root) {
        report.files += 1;
        let level = level_of(&file);
        let krate = crate_of(&file);
        if krate.as_os_str().is_empty() {
            report
                .fails
                .push(format!("{}: no enclosing crate", file.display()));
            continue;
        }
        let allowed = read_allowed_levels(&krate.join("Cargo.toml"));
        if !allowed.contains(&level) {
            report.fails.push(format!(
                "{}: {} not in allowed_levels {:?} of {}",
                file.display(),
                level,
                allowed,
                krate.display()
            ));
            continue;
        }
        let src = match fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) => {
                report
                    .fails
                    .push(format!("{}: cannot read: {}", file.display(), e));
                continue;
            }
        };
        for r in policy.rules_for(level, true) {
            for (n, line) in src.lines().enumerate() {
                if r.regex.is_match(line) {
                    report.warns.push(format!(
                        "{}:{}: {} (forbid, warn-only this wave)",
                        file.display(),
                        n + 1,
                        r.id
                    ));
                }
            }
        }
        for r in policy.rules_for(level, false) {
            for (n, line) in src.lines().enumerate() {
                if r.regex.is_match(line) {
                    report
                        .warns
                        .push(format!("{}:{}: {} (warn)", file.display(), n + 1, r.id));
                }
            }
        }
        if level == Level::L3 && src.contains("// TODO") {
            report.warns.push(format!(
                "{}: L3 carries // TODO (prepared stub, warns until L3 fills it)",
                file.display()
            ));
        }
    }
    report
}

// see: cans/01-architecture.md#Codebase-invariants
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of, structure};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const TODO_MARK: &str = concat!("/", "/ ", "TODO");
// see: cans/01-architecture.md#Codebase-invariants
const CONCERN_LEAK_MIN_RUN: usize = 3;

// see: cans/01-architecture.md#Codebase-invariants
const OS_TOKENS: [&str; 5] = ["libc", "std::fs", "std::net", "std::process", "std::os"];

// see: cans/01-architecture.md#Codebase-invariants
const NET_PURE_TYPES: [&str; 6] = [
    "Ipv4Addr",
    "Ipv6Addr",
    "IpAddr",
    "SocketAddr",
    "SocketAddrV4",
    "SocketAddrV6",
];

// see: cans/01-architecture.md#Codebase-invariants
const TOOLING_CRATES: [&str; 4] = [
    "kelir-maturity",
    "kelir-plugin-cli",
    "kelir-plugin-testkit",
    "kelir-testkit",
];

// see: cans/01-architecture.md#Codebase-invariants
pub fn is_tooling_file(path: &Path) -> bool {
    if is_bin_entry_point(path) {
        return true;
    }
    let krate = crate_of(path);
    let name = krate.file_name().and_then(|n| n.to_str()).unwrap_or("");
    TOOLING_CRATES.contains(&name)
}

// see: cans/01-architecture.md#Codebase-invariants
fn is_bin_entry_point(path: &Path) -> bool {
    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    parts.windows(2).any(|w| w[0] == "src" && w[1] == "bin")
        || parts.windows(2).any(|w| w[0] == "src" && w[1] == "main.rs")
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn is_import_only_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ") || trimmed.starts_with("pub use ")
}

// see: cans/01-architecture.md#Codebase-invariants
fn os_reach_exempt(file: &Path, line: &str, level: Level) -> bool {
    if is_tooling_file(file) {
        return true;
    }
    if is_named_profile_read(file, line) {
        return true;
    }
    if level == Level::L2 {
        return os_reach_carveout(line);
    }
    is_import_only_line(line) || os_call_free(line)
}

// see: cans/01-architecture.md#Codebase-invariants
fn is_named_profile_read(file: &Path, line: &str) -> bool {
    file.display()
        .to_string()
        .ends_with("crates/kelir/src/builder_sdk.flow.rs")
        && line
            .trim_start()
            .starts_with("let text = std::fs::read_to_string(&path)")
}

// see: cans/01-architecture.md#Codebase-invariants
fn os_call_free(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut matched: Option<&str> = None;
        for token in OS_TOKENS {
            if line[i..].starts_with(token) && is_word_boundary(line, i, i + token.len()) {
                matched = Some(token);
                break;
            }
        }
        let Some(token) = matched else {
            i += 1;
            continue;
        };
        let (_, end) = path_idents_after(line, i, token);
        if line[end..].trim_start().starts_with('(') {
            return false;
        }
        i += token.len();
    }
    true
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn os_reach_carveout(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut matched: Option<&str> = None;
        for token in OS_TOKENS {
            if line[i..].starts_with(token) && is_word_boundary(line, i, i + token.len()) {
                matched = Some(token);
                break;
            }
        }
        let Some(token) = matched else {
            i += 1;
            continue;
        };
        if !os_token_allowed(line, token, i) {
            return false;
        }
        i += token.len();
    }
    true
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn line_has_guest_io(line: &str) -> bool {
    line.contains(concat!("rusqlite", "::"))
        || line.contains(concat!("tokio", "::fs::"))
        || (OS_TOKENS.iter().any(|t| contains_word(line, t)) && !os_reach_carveout(line))
}

fn contains_word(line: &str, token: &str) -> bool {
    let mut start = 0;
    while let Some(off) = line[start..].find(token) {
        let i = start + off;
        if is_word_boundary(line, i, i + token.len()) {
            return true;
        }
        start = i + 1;
    }
    false
}

fn is_word_boundary(line: &str, start: usize, end: usize) -> bool {
    let before = start == 0 || !is_ident_byte(line.as_bytes()[start - 1]);
    let after = end >= line.len() || !is_ident_byte(line.as_bytes()[end]);
    before && after
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i + 1 < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'/' && bytes[i + 1] == b'/' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn os_token_allowed(line: &str, token: &str, at: usize) -> bool {
    if let Some(cs) = comment_start(line) {
        if at >= cs {
            return true;
        }
    }
    if is_import_only_line(line) {
        return true;
    }
    match token {
        "libc" => libc_use_allowed(line, at),
        "std::fs" => fs_use_allowed(line, at),
        "std::net" => net_use_allowed(line, at),
        "std::process" => process_use_allowed(line, at),
        "std::os" => os_path_call_free(line, at),
        _ => false,
    }
}

fn path_idents_after(line: &str, at: usize, token: &str) -> (Vec<String>, usize) {
    let mut idents = Vec::new();
    let mut i = at + token.len();
    let bytes = line.as_bytes();
    while i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b':' {
        i += 2;
        let start = i;
        while i < bytes.len() && is_ident_byte(bytes[i]) {
            i += 1;
        }
        if i == start {
            break;
        }
        idents.push(line[start..i].to_string());
    }
    (idents, i)
}

fn libc_use_allowed(line: &str, at: usize) -> bool {
    let (_, end) = path_idents_after(line, at, "libc");
    let rest = line[end..].trim_start();
    !rest.starts_with('(')
}

fn fs_use_allowed(line: &str, at: usize) -> bool {
    let (idents, end) = path_idents_after(line, at, "std::fs");
    let Some(call) = idents.first().map(String::as_str) else {
        return true;
    };
    match call {
        "create_dir_all" | "remove_dir_all" | "remove_dir" => {
            line[end..].trim_start().starts_with('(')
        }
        "read_to_string" => {
            line[end..].trim_start().starts_with('(') && lockfile_probe_arg(line, end)
        }
        "metadata" => {
            line[end..].trim_start().starts_with('(') && metadata_self_path_arg(line, end)
        }
        _ => false,
    }
}

fn metadata_self_path_arg(line: &str, end: usize) -> bool {
    let rest = line[end..].trim_start();
    let arg = rest.strip_prefix('(').unwrap_or("").trim_start();
    arg.starts_with("&self.")
}

fn lockfile_probe_arg(line: &str, end: usize) -> bool {
    let rest = line[end..].trim_start();
    let arg = rest.strip_prefix('(').unwrap_or("").trim_start();
    let mut ident = String::new();
    for b in arg.bytes() {
        if is_ident_byte(b) {
            ident.push(b as char);
        } else {
            break;
        }
    }
    !ident.is_empty()
        && ident
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'_' || b.is_ascii_digit())
        && (ident.contains("LOCKFILE") || ident.ends_with("_PATH"))
}

fn net_use_allowed(line: &str, at: usize) -> bool {
    let (idents, _) = path_idents_after(line, at, "std::net");
    match idents.first().map(String::as_str) {
        Some(name) => NET_PURE_TYPES.contains(&name),
        None => true,
    }
}

fn process_use_allowed(line: &str, at: usize) -> bool {
    let (idents, end) = path_idents_after(line, at, "std::process");
    match idents.first().map(String::as_str) {
        Some("id") => line[end..].trim_start().starts_with('('),
        Some("ExitCode") => true,
        Some(_) => false,
        None => true,
    }
}

fn os_path_call_free(line: &str, at: usize) -> bool {
    let (_, end) = path_idents_after(line, at, "std::os");
    let rest = line[end..].trim_start();
    !rest.starts_with('(')
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
    let walker = WalkDir::new(base).into_iter().filter_entry(|e| {
        !(e.file_type().is_dir()
            && matches!(e.file_name().to_str(), Some("target" | "tests" | "benches")))
    });
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
    let files = rs_files(root);
    for file in &files {
        report.files += 1;
        structure::check_file(file, policy, &mut report);
        let level = level_of(file);
        let krate = crate_of(file);
        if krate.as_os_str().is_empty() {
            report.hard_fails += 1;
            report
                .fails
                .push(format!("{}: no enclosing crate", file.display()));
            continue;
        }
        let allowed = read_allowed_levels(&krate.join("Cargo.toml"));
        if !allowed.contains(&level) {
            report.hard_fails += 1;
            report.fails.push(format!(
                "{}: {} not in allowed_levels {:?} of {}",
                file.display(),
                level,
                allowed,
                krate.display()
            ));
            continue;
        }
        let src = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                report.hard_fails += 1;
                report
                    .fails
                    .push(format!("{}: cannot read: {}", file.display(), e));
                continue;
            }
        };
        for r in policy.rules_for(level, true) {
            for (n, line) in src.lines().enumerate() {
                if r.regex.is_match(line) {
                    if r.id == "os-reach" && os_reach_exempt(file, line, level) {
                        continue;
                    }
                    if policy.forbid_mode == ForbidMode::Error {
                        let text = if let Some(h) = r.hint.as_deref().filter(|h| !h.is_empty()) {
                            format!("{}:{}: [forbid:{}] {}", file.display(), n + 1, r.id, h)
                        } else {
                            format!("{}:{}: [forbid:{}]", file.display(), n + 1, r.id)
                        };
                        report.fails.push(text);
                    } else {
                        report.warns.push(format!(
                            "{}:{}: {} (forbid, warn-mode rollout)",
                            file.display(),
                            n + 1,
                            r.id
                        ));
                    }
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
        if level == Level::L3 && src.contains(TODO_MARK) {
            report.warns.push(format!(
                "{}: L3 carries a TODO marker (prepared stub, warns until L3 fills it)",
                file.display()
            ));
        }
    }
    check_concern_leak(root, &files, policy, &mut report);
    report
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_concern_leak(root: &Path, files: &[PathBuf], policy: &Policy, report: &mut CheckReport) {
    let mut by_crate: BTreeMap<PathBuf, (usize, usize, Vec<String>)> = BTreeMap::new();
    for file in files {
        if !is_lib_file(file) {
            continue;
        }
        let krate = crate_of(file);
        if krate.as_os_str().is_empty() {
            continue;
        }
        if !read_allowed_levels(&krate.join("Cargo.toml")).contains(&Level::L3) {
            continue;
        }
        let entry = by_crate.entry(krate).or_default();
        match level_of(file) {
            Level::L2 => entry.0 += 1,
            Level::L3 => {
                entry.1 += 1;
                entry.2.push(
                    file.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
            }
            Level::L1 => {}
        }
    }
    for (krate, (flow, run, run_files)) in by_crate {
        if flow != 0 || run < CONCERN_LEAK_MIN_RUN {
            continue;
        }
        let at = krate
            .strip_prefix(root)
            .unwrap_or(&krate)
            .display()
            .to_string();
        let detail = format!(
            "concern leak: {run} .run.rs files with no .flow.rs seam ({})",
            run_files.join(", ")
        );
        if policy.forbid_mode == ForbidMode::Error {
            report
                .fails
                .push(format!("{at}: [concern-leak:C1] {detail}"));
        } else {
            report.warns.push(format!(
                "{at}: {detail} (concern-leak:C1, warn-mode rollout)"
            ));
        }
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn is_lib_file(path: &Path) -> bool {
    let mut under_src = false;
    for part in path.components() {
        match part.as_os_str().to_str() {
            Some("src") => under_src = true,
            Some("bin") if under_src => return false,
            _ => {}
        }
    }
    under_src
}

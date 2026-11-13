#![forbid(unsafe_code)]
// see: cans/15-operations.md#Developer-CLI
use kelir_maturity::{Level, Policy, check_tree, level_of, rs_files};
use regex::Regex;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn print_line(s: &str) {
    use std::io::Write;
    let mut lock = std::io::stdout().lock();
    match writeln!(lock, "{}", s) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(0),
        Err(_) => std::process::exit(1),
    }
}

fn usage() -> String {
    "usage: kelir-maturity check [--root <dir>] [--base <ref>] [--strict]\n       kelir-maturity list --level <L1|L2|L3> [--crate <name>]\n       kelir-maturity list --tag <slop-tag>\n       kelir-maturity emit --out <dir>"
        .to_string()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn load_policy(root: &Path) -> Policy {
    let mut dir = match root.canonicalize() {
        Ok(d) => d,
        Err(_) => root.to_path_buf(),
    };
    loop {
        let candidate = dir.join("perf").join("maturity-policy.toml");
        if let Ok(text) = fs::read_to_string(&candidate) {
            return match Policy::from_toml(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("policy {}: {}", candidate.display(), e);
                    Policy::empty()
                }
            };
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    eprintln!("no perf/maturity-policy.toml found; structural check only");
    Policy::empty()
}

fn crate_name(file: &Path, root: &Path) -> String {
    let krate = kelir_maturity::crate_of(file);
    let rel = krate.strip_prefix(root).unwrap_or(&krate);
    rel.components()
        .last()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn rel_path(file: &Path, root: &Path) -> String {
    match file.strip_prefix(root) {
        Ok(p) => p.display().to_string(),
        Err(_) => file.display().to_string(),
    }
}

fn cmd_check(args: &[String]) -> ExitCode {
    let root = flag(args, "--root")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let strict = args.iter().any(|a| a == "--strict");
    let policy = load_policy(&root);
    let mut report = check_tree(&root, &policy);
    if let Some(base) = flag(args, "--base") {
        match l2_delta_files(&root, &base) {
            Ok(delta) => {
                let max = policy.l2_max_delta_per_pr;
                if max > 0 && delta > max as u64 {
                    report.fails.push(format!(
                        "L2 delta budget exceeded: {} net new .flow.rs files vs {} (max {})",
                        delta, base, max
                    ));
                }
            }
            Err(e) => report.fails.push(format!("L2 delta: {e}")),
        }
    }
    for fail in report.fails.iter().chain(report.structural_fails.iter()) {
        print_line(&format!("FAIL {fail}"));
    }
    let mut warn_counts: BTreeMap<String, usize> = BTreeMap::new();
    for warn in &report.warns {
        let id = warn
            .split(": ")
            .nth(1)
            .unwrap_or("")
            .split(" (")
            .next()
            .unwrap_or("")
            .to_string();
        *warn_counts.entry(id).or_insert(0) += 1;
    }
    for (id, n) in &warn_counts {
        print_line(&format!("WARN {}: {} hits (content rule, warn-only this wave)", id, n));
    }
    if report.passed() {
        print_line(&format!(
            "OK: {} files, 0 placement failures, {} content warnings",
            report.files,
            report.warns.len()
        ));
        ExitCode::from(0)
    } else if strict {
        print_line(&format!(
            "FAIL: {} placement/content failures, {} structural failures in {} files (--strict)",
            report.fails.len(),
            report.structural_fails.len(),
            report.files
        ));
        ExitCode::from(1)
    } else if report.structural_fails.is_empty() {
        print_line(&format!(
            "FAIL: {} placement/content failures in {} files",
            report.fails.len(),
            report.files
        ));
        ExitCode::from(1)
    } else {
        print_line(&format!(
            "FAIL: {} structural failures in {} files",
            report.structural_fails.len(),
            report.files
        ));
        ExitCode::from(1)
    }
}

fn l2_delta_files(root: &Path, base: &str) -> Result<u64, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("--name-status")
        .arg(base)
        .arg("--")
        .arg("*.flow.rs")
        .output()
        .map_err(|e| format!("git diff failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff --name-status {base} failed (unresolvable ref?): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut net_new = 0u64;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.starts_with('A') {
            net_new += 1;
        } else if line.starts_with('D') {
            net_new = net_new.saturating_sub(1);
        }
    }
    Ok(net_new)
}

fn cmd_list(args: &[String], root: &Path) -> ExitCode {
    if let Some(tag) = flag(args, "--tag") {
        let needle = format!("[{}]", tag);
        let mut hits = 0;
        for file in rs_files(root) {
            if let Ok(src) = fs::read_to_string(&file) {
                for (n, line) in src.lines().enumerate() {
                    if line.contains(&needle) {
                        print_line(&format!(
                            "{}:{}: {}",
                            rel_path(&file, root),
                            n + 1,
                            line.trim()
                        ));
                        hits += 1;
                    }
                }
            }
        }
        if hits == 0 {
            print_line(&format!("no files carry tag [{}]", tag));
        }
        return ExitCode::from(0);
    }
    let level = match flag(args, "--level").as_deref().and_then(Level::from_name) {
        Some(l) => l,
        None => {
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    };
    let krate = flag(args, "--crate");
    for file in rs_files(root) {
        if level_of(&file) != level {
            continue;
        }
        if let Some(c) = &krate {
            if crate_name(&file, root) != *c {
                continue;
            }
        }
        print_line(&rel_path(&file, root));
    }
    ExitCode::from(0)
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn cmd_emit(args: &[String], root: &Path) -> ExitCode {
    let out = flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("maturity"));
    let files = rs_files(root);
    let mut by_level: BTreeMap<Level, Vec<String>> = BTreeMap::new();
    let mut by_crate: BTreeMap<String, BTreeMap<Level, usize>> = BTreeMap::new();
    let mut by_tag: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let tag_re = match Regex::new(r"\[([A-Za-z0-9_]+)\]") {
        Ok(r) => r,
        Err(_) => return ExitCode::from(1),
    };
    for file in &files {
        let level = level_of(file);
        let rel = rel_path(file, root);
        by_level.entry(level).or_default().push(rel.clone());
        by_crate
            .entry(crate_name(file, root))
            .or_default()
            .entry(level)
            .and_modify(|n| *n += 1)
            .or_insert(1);
        if let Ok(src) = fs::read_to_string(file) {
            for line in src.lines() {
                if line.contains("// TODO") {
                    for cap in tag_re.captures_iter(line) {
                        by_tag
                            .entry(cap[1].to_string())
                            .or_default()
                            .push(rel.clone());
                    }
                }
            }
        }
    }
    for list in by_tag.values_mut() {
        list.sort();
        list.dedup();
    }
    let mut counts = String::from("{\n  \"files\": ");
    counts.push_str(&files.len().to_string());
    counts.push_str(",\n  \"by_level\": {");
    for (i, level) in [Level::L1, Level::L2, Level::L3].iter().enumerate() {
        if i > 0 {
            counts.push(',');
        }
        counts.push_str(&format!(
            "\n    \"{}\": {}",
            level,
            by_level.get(level).map(|v| v.len()).unwrap_or(0)
        ));
    }
    counts.push_str("\n  },\n  \"by_crate\": {");
    for (i, (name, levels)) in by_crate.iter().enumerate() {
        if i > 0 {
            counts.push(',');
        }
        counts.push_str(&format!("\n    \"{}\": {{", json_escape(name)));
        for (j, level) in [Level::L1, Level::L2, Level::L3].iter().enumerate() {
            if j > 0 {
                counts.push(',');
            }
            counts.push_str(&format!(
                "\n      \"{}\": {}",
                level,
                levels.get(level).copied().unwrap_or(0)
            ));
        }
        counts.push_str("\n    }");
    }
    counts.push_str("\n  }\n}\n");
    let write = |path: &Path, text: &str| -> bool {
        if let Some(Err(_)) = path.parent().map(fs::create_dir_all) {
            return false;
        }
        fs::write(path, text).is_ok()
    };
    let mut ok = write(&out.join("counts.json"), &counts);
    for (level, list) in &by_level {
        let mut text = String::new();
        for f in list {
            text.push_str(f);
            text.push('\n');
        }
        ok &= write(&out.join("by-level").join(format!("{}.txt", level)), &text);
    }
    for (name, levels) in &by_crate {
        let mut text = String::new();
        for level in [Level::L1, Level::L2, Level::L3] {
            text.push_str(&format!(
                "{}: {}\n",
                level,
                levels.get(&level).copied().unwrap_or(0)
            ));
        }
        ok &= write(&out.join("by-crate").join(format!("{}.txt", name)), &text);
    }
    for (tag, list) in &by_tag {
        let mut text = String::new();
        for f in list {
            text.push_str(f);
            text.push('\n');
        }
        ok &= write(&out.join("by-tag").join(format!("{}.txt", tag)), &text);
    }
    if ok {
        print_line(&format!("wrote {} files to {}", files.len(), out.display()));
        ExitCode::from(0)
    } else {
        eprintln!("emit: failed writing under {}", out.display());
        ExitCode::from(1)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match args.first().map(|s| s.as_str()) {
        Some("check") => cmd_check(&args[1..]),
        Some("list") => cmd_list(&args[1..], &root),
        Some("emit") => cmd_emit(&args[1..], &root),
        _ => {
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

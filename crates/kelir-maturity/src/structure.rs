#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Codebase-invariants
use std::fs;
use std::path::Path;

use crate::{CheckReport, Level, crate_of, level_of, read_allowed_levels};

pub fn check_file(path: &Path, _policy: &crate::Policy, report: &mut CheckReport) {
    let Ok(src) = fs::read_to_string(path) else { return };
    let level = level_of(path);
    let lines = src.lines().count();
    if level == Level::L1 {
        let mut hits: Vec<&str> = Vec::new();
        for needle in [
            "async fn",
            ".await",
            "tokio::",
            "std::thread::spawn",
            "#[tokio::main]",
            "#[tokio::test]",
        ] {
            if src.contains(needle) {
                hits.push(needle);
            }
        }
        if (src.contains("Mutex<") || src.contains("RwLock<")) && has_mutating_impl(&src) {
            hits.push("Mutex</RwLock< with &mut self impls");
        }
        if !hits.is_empty() {
            report.structural_fails.push(format!(
                "{}: [structure:S1] L1 file carries runtime concern ({}); move to <stem>.flow.rs",
                path.display(),
                hits.join(", ")
            ));
        }
    }
    if is_aggregator(path) && lines > 120 && has_mutating_impl(&src) {
        report.structural_fails.push(format!(
            "{}: [structure:S2] lib.rs/mod.rs is not an aggregator ({} lines with mutating impls); extract the impl to <Type>.flow.rs",
            path.display(),
            lines
        ));
    }
    if level == Level::L2 && has_runtime_spawn(&src) {
        report.structural_fails.push(format!(
            "{}: [structure:S3] L2 file spawns a runtime; move to <stem>.run.rs (requires allowed_levels += L3)",
            path.display()
        ));
    }
    if level == Level::L2
        && crate_allows_l3(&crate_of(path))
        && has_runtime_concern(&src)
        && has_io_concern(&src)
    {
        report.structural_fails.push(format!(
            "{}: [structure:S4] .flow.rs carries I/O (std::fs/tokio::fs/libc/std::process) inside an L3-allowed crate; move to <stem>.run.rs",
            path.display()
        ));
    }
    if is_aggregator(path) && src.contains("async fn") && has_mutating_impl(&src) {
        report.structural_fails.push(format!(
            "{}: [structure:S5] lib.rs/mod.rs carries async impl bodies; extract to <impl>.flow.rs",
            path.display()
        ));
    }
}

fn has_runtime_concern(src: &str) -> bool {
    src.contains("async fn")
        || src.contains(".await")
        || src.contains("tokio::")
        || src.contains("std::thread::spawn")
        || src.contains("#[tokio::main]")
        || src.contains("#[tokio::test]")
}

fn has_runtime_spawn(src: &str) -> bool {
    src.contains("std::thread::spawn")
        || src.contains("#[tokio::main]")
        || src.contains("tokio::runtime::Runtime::new")
}

fn has_io_concern(src: &str) -> bool {
    src.contains("std::fs::")
        || src.contains("tokio::fs::")
        || src.contains("libc::")
        || src.contains("std::process::")
        || src.contains("rusqlite::")
}

fn has_mutating_impl(src: &str) -> bool {
    src.split("impl").skip(1).any(|block| block.contains("&mut self"))
}

fn is_aggregator(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("lib.rs" | "mod.rs")
    )
}

fn crate_allows_l3(crate_dir: &Path) -> bool {
    read_allowed_levels(&crate_dir.join("Cargo.toml")).contains(&Level::L3)
}

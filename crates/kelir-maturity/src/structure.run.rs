// see: cans/01-architecture.md#Codebase-invariants
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of, read_allowed_levels};
use std::fs;
use std::path::Path;

pub fn check_file(path: &Path, policy: &Policy, report: &mut CheckReport) {
    let Ok(src) = fs::read_to_string(path) else {
        return;
    };
    let level = level_of(path);
    let lines = src.lines().count();
    let at = path.display().to_string();
    if level == Level::L1 && has_runtime_concern(&src) {
        record(
            report,
            policy.forbid_mode,
            "S1",
            &at,
            "L1 file carries runtime concern (async fn/.await/tokio::/spawn); move to <stem>.flow.rs",
        );
    }
    if is_aggregator(path) && lines > 120 && has_mutating_impl(&src) {
        record(
            report,
            policy.forbid_mode,
            "S2",
            &format!("{at}:{lines}"),
            "lib.rs/mod.rs is not an aggregator; extract the impl to <Type>.flow.rs",
        );
    }
    if level == Level::L2
        && (src.contains("std::thread::spawn")
            || src.contains("#[tokio::main]")
            || src.contains("tokio::runtime::Runtime::new")
            || src.contains("tokio::runtime::Builder"))
    {
        record(
            report,
            policy.forbid_mode,
            "S3",
            &at,
            "L2 file spawns a runtime; move to <stem>.run.rs (requires allowed_levels += L3)",
        );
    }
    if level == Level::L2
        && crate_allows_l3(&crate_of(path))
        && has_runtime_concern(&src)
        && has_io_concern(&src)
    {
        record(
            report,
            policy.forbid_mode,
            "S4",
            &at,
            ".flow.rs carries I/O (std::fs/tokio::fs/libc/std::process) inside an L3-allowed crate; move to <stem>.run.rs",
        );
    }
    if is_aggregator(path) && src.contains("async fn") && has_mutating_impl(&src) {
        record(
            report,
            policy.forbid_mode,
            "S5",
            &at,
            "lib.rs/mod.rs carries async impl bodies; extract to <impl>.flow.rs",
        );
    }
}

fn record(report: &mut CheckReport, mode: ForbidMode, rule: &str, at: &str, detail: &str) {
    if mode == ForbidMode::Error {
        report
            .fails
            .push(format!("{at}: [structure:{rule}] {detail}"));
    } else {
        report.warns.push(format!(
            "{at}: {detail} (structure:{rule}, warn-mode rollout)"
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

fn has_io_concern(src: &str) -> bool {
    src.contains("std::fs::")
        || src.contains("tokio::fs::")
        || src.contains("libc::")
        || src.contains("std::process::")
        || src.contains("rusqlite::")
}

fn has_mutating_impl(src: &str) -> bool {
    src.split("impl").skip(1).any(|block| {
        let (_, body) = block.split_once('{').unwrap_or(("", ""));
        body.contains("&mut self")
    })
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

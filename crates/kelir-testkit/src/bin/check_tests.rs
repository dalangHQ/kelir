use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

fn main() -> ExitCode {
    let repo_root = match PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
    {
        Some(root) => root.to_path_buf(),
        None => {
            eprintln!("ERROR: Failed to locate workspace root");
            return ExitCode::FAILURE;
        }
    };

    let tests_dir = repo_root.join("crates");
    let mut violations = Vec::new();

    for entry in WalkDir::new(&tests_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs")
            && (path.to_string_lossy().contains("/tests/")
                || path.to_string_lossy().contains("/benches/"))
        {
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            lint_file(path, &content, &mut violations);
        }
    }

    if violations.is_empty() {
        println!("Anti-Slop Lint passed: zero violations across test suites.");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "ERROR: Anti-Slop Linter detected {} rule violations:",
            violations.len()
        );
        for v in violations {
            eprintln!("  {}", v);
        }
        ExitCode::FAILURE
    }
}

fn lint_file(path: &Path, content: &str, violations: &mut Vec<String>) {
    let rel_path = path.display().to_string();

    if rel_path.contains("/integration/")
        && (content.contains("mockall")
            || content.contains("MockVmmDriver")
            || content.contains("MockHostInterposer"))
    {
        violations.push(format!(
            "{}: [AS-01] Mocking hypervisor interfaces in integration tier is prohibited.",
            rel_path
        ));
    }

    for (line_idx, line) in content.lines().enumerate() {
        if (line.contains(".exec(")
            || line.contains(".rollback()")
            || line.contains("evaluate_trap"))
            && (line.contains(".is_ok()") || line.contains(".is_err()"))
        {
            violations.push(format!(
                "{}:{}: [AS-03] Naked .is_ok()/.is_err() on execution/rollback. Must match typed Verdict variant or ExitKind.",
                rel_path, line_idx + 1
            ));
        }
    }
    if rel_path.contains("/benches/") {
        if !content.contains("budgets.toml") {
            violations.push(format!(
                "{}: [AS-04B] Benchmark suite must enforce thresholds from perf/budgets.toml; AS-04A does not apply here.",
                rel_path
            ));
        }
    } else {
        for (line_idx, line) in content.lines().enumerate() {
            if line.contains("std::time::Instant")
                || line.contains("Instant::now()")
                || line.contains("SystemTime::now()")
                || line.contains("thread::sleep")
                || line.contains("thread_rng")
                || line.contains("rand::random")
            {
                violations.push(format!(
                    "{}:{}: [AS-04A] Wall-clock sampling, sleep, or unseeded randomness in deterministic test context. Pin the virtual clock or seed ChaCha20.",
                    rel_path, line_idx + 1
                ));
            }
        }
    }
}

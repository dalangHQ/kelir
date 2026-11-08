//! Static AST Linter enforcing Anti-Slop Laws AS-01 through AS-08 across tests/ directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

fn main() -> ExitCode {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to locate workspace root")
        .to_path_buf();

    let tests_dir = repo_root.join("crates");
    let mut violations = Vec::new();

    for entry in WalkDir::new(&tests_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs")
            && path.to_string_lossy().contains("/tests/")
        {
            let content = fs::read_to_string(path).unwrap_or_default();
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

    // AS-01: Banned Mocking in integration suites
    if rel_path.contains("/integration/") {
        if content.contains("mockall")
            || content.contains("MockVmmDriver")
            || content.contains("MockHostInterposer")
        {
            violations.push(format!(
                "{}: [AS-01] Mocking hypervisor interfaces in integration tier is prohibited.",
                rel_path
            ));
        }
    }

    // AS-03: Prohibit naked .is_ok() / .is_err() on exec, verdict, or rollback results
    for (line_idx, line) in content.lines().enumerate() {
        if line.contains(".exec(") || line.contains(".rollback()") || line.contains("evaluate_trap")
        {
            if line.contains(".is_ok()") || line.contains(".is_err()") {
                violations.push(format!(
                    "{}:{}: [AS-03] Naked .is_ok()/.is_err() on execution/rollback. Must match typed Verdict variant or ExitKind.",
                    rel_path, line_idx + 1
                ));
            }
        }
    }
}

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

struct Counts {
    rs_test_files: usize,
    rs_bench_files: usize,
    ts_test_files: usize,
    py_test_files: usize,
    ts_src_files: usize,
    py_src_files: usize,
    rs_src_files: usize,
    yaml_cases: usize,
    yaml_snippets: usize,
}

struct YamlCase {
    rel_path: String,
    rules: Vec<String>,
    snippets: Vec<(String, String)>,
    mitigation_laws: Vec<String>,
    hypervisor: Option<String>,
    has_emulated_opt_in: bool,
}

fn contains_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let needle = word.as_bytes();
    if needle.len() > bytes.len() {
        return false;
    }
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = 0;
    while start + needle.len() <= bytes.len() {
        if &bytes[start..start + needle.len()] == needle {
            let before_ok = start == 0 || !is_word(bytes[start - 1]);
            let after = start + needle.len();
            let after_ok = after >= bytes.len() || !is_word(bytes[after]);
            if before_ok && after_ok {
                return true;
            }
        }
        start += 1;
    }
    false
}

fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut i = open;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
        } else if in_double {
            if b == b'\\' {
                i += 1;
            } else if b == b'"' {
                in_double = false;
            }
        } else if b == b'\'' {
            in_single = true;
        } else if b == b'"' {
            in_double = true;
        } else if b == b'(' || b == b'[' || b == b'{' {
            depth += 1;
        } else if b == b')' || b == b']' || b == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn split_top_level(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
        } else if in_double {
            if b == b'\\' {
                i += 1;
            } else if b == b'"' {
                in_double = false;
            }
        } else if b == b'\'' {
            in_single = true;
        } else if b == b'"' {
            in_double = true;
        } else if b == b'(' || b == b'[' || b == b'{' {
            depth += 1;
        } else if b == b')' || b == b']' || b == b'}' {
            depth = depth.saturating_sub(1);
        } else if b == b',' && depth == 0 {
            parts.push(text[start..i].to_string());
            start = i + 1;
        }
        i += 1;
    }
    parts.push(text[start..].to_string());
    parts
}

fn root_ident(arg: &str) -> String {
    let trimmed = arg.trim_start_matches([' ', '&', '*', '(']);
    let mut ident = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(c);
        } else {
            break;
        }
    }
    ident
}

// see: cans/artifacts/test-plan/matrix.yaml#anti_slop_laws.AS-06
fn is_tautology(snippet: &str) -> bool {
    for macro_name in ["assert_eq!(", "assert_ne!("] {
        let mut search_from = 0usize;
        while let Some(pos) = snippet[search_from..].find(macro_name) {
            let open = search_from + pos + macro_name.len() - 1;
            let Some(close) = matching_paren(snippet, open) else {
                break;
            };
            let inner = &snippet[open + 1..close];
            let args = split_top_level(inner);
            if args.len() >= 2 {
                let root = root_ident(&args[0]);
                if !root.is_empty() && args[1].contains('(') && contains_word(&args[1], &root) {
                    return true;
                }
            }
            search_from = close + 1;
        }
    }
    let mut search_from = 0usize;
    while let Some(pos) = snippet[search_from..].find("assert!(") {
        let open = search_from + pos + "assert!".len();
        let Some(close) = matching_paren(snippet, open) else {
            break;
        };
        let inner = &snippet[open + 1..close];
        if let Some(eq) = inner.find("==") {
            let lhs = inner[..eq].trim();
            let rhs = inner[eq + 2..].trim();
            let root = root_ident(lhs);
            if !root.is_empty() && rhs.contains('(') && contains_word(rhs, &root) {
                return true;
            }
        }
        search_from = close + 1;
    }
    false
}

// see: cans/artifacts/test-plan/case-to-test-map.yaml#harness_tests
fn harness_exempt_files(repo_root: &Path) -> HashSet<String> {
    let mut exempt = HashSet::new();
    let map_path = repo_root.join("cans/artifacts/test-plan/case-to-test-map.yaml");
    let Ok(raw) = fs::read_to_string(&map_path) else {
        return exempt;
    };
    let mut in_harness = false;
    for line in raw.lines() {
        if line.starts_with("harness_tests:") {
            in_harness = true;
        } else if in_harness {
            if let Some(rest) = line.strip_prefix("- file:") {
                let file = rest.trim().trim_matches('"');
                if !file.is_empty() {
                    exempt.insert(file.to_string());
                }
            } else if !line.starts_with(' ') && !line.starts_with('-') {
                in_harness = false;
            }
        }
    }
    exempt
}

fn is_vendor_entry(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    name == "node_modules" || name == ".venv"
}

fn valid_rust_test_stem(stem: &str) -> bool {
    let Some(base) = stem.strip_suffix("_test") else {
        return false;
    };
    let base = ["_unit", "_integration", "_parity"]
        .iter()
        .find_map(|role| base.strip_suffix(*role))
        .unwrap_or(base);
    let tokens: Vec<&str> = base.split('_').filter(|t| !t.is_empty()).collect();
    tokens.len() >= 2
        && tokens.iter().all(|token| {
            token
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn valid_sdk_test_base(base: &str, sep: char) -> bool {
    let mut tokens: Vec<&str> = base.split(sep).filter(|t| !t.is_empty()).collect();
    if tokens
        .last()
        .is_some_and(|token| ["unit", "integration", "parity"].contains(token))
    {
        tokens.pop();
    }
    tokens.len() >= 2
}

fn valid_src_base(base: &str, sep: char) -> bool {
    let mut tokens: Vec<&str> = base.split(sep).filter(|t| !t.is_empty()).collect();
    if tokens
        .last()
        .is_some_and(|token| ["unit", "integration", "parity", "generated"].contains(token))
    {
        tokens.pop();
    }
    tokens.len() >= 2
        && tokens.iter().all(|token| {
            token
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn lint_test_filename(
    path: &Path,
    repo_root: &Path,
    exempt: &HashSet<String>,
    valid: bool,
    code: &str,
    grammar: &str,
    violations: &mut Vec<String>,
) {
    let rel = rel_path_of(path, repo_root);
    if exempt.contains(&rel) || valid {
        return;
    }
    violations.push(format!(
        "{rel}: [{code}] test filename violates {grammar} grammar."
    ));
}

fn rel_path_of(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|rel| rel.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

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

    let mut violations = Vec::new();
    let mut counts = Counts {
        rs_test_files: 0,
        rs_bench_files: 0,
        ts_test_files: 0,
        py_test_files: 0,
        ts_src_files: 0,
        py_src_files: 0,
        rs_src_files: 0,
        yaml_cases: 0,
        yaml_snippets: 0,
    };
    let harness_exempt = harness_exempt_files(&repo_root);

    let tests_dir = repo_root.join("crates");
    for entry in WalkDir::new(&tests_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs")
            && (path.to_string_lossy().contains("/tests/")
                || path.to_string_lossy().contains("/benches/"))
        {
            if path.to_string_lossy().contains("/benches/") {
                counts.rs_bench_files += 1;
            } else {
                counts.rs_test_files += 1;
                let stem = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_default();
                lint_test_filename(
                    path,
                    &repo_root,
                    &harness_exempt,
                    valid_rust_test_stem(&stem),
                    "NG-01",
                    "<entity>_<domain>[_role]_test.rs",
                    &mut violations,
                );
            }
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            lint_file(path, &content, &mut violations);
        }
    }

    let ts_dir = repo_root.join("packages/ts");
    for entry in WalkDir::new(&ts_dir)
        .into_iter()
        .filter_entry(|entry| !is_vendor_entry(entry))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(base) = name.strip_suffix(".test.ts") else {
            continue;
        };
        counts.ts_test_files += 1;
        lint_test_filename(
            path,
            &repo_root,
            &harness_exempt,
            valid_sdk_test_base(base, '.'),
            "NG-02",
            "<entity>.<domain>[.role].test.ts",
            &mut violations,
        );
    }

    let py_dir = repo_root.join("packages/py");
    for entry in WalkDir::new(&py_dir)
        .into_iter()
        .filter_entry(|entry| !is_vendor_entry(entry))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(without_ext) = name.strip_suffix(".py") else {
            continue;
        };
        let Some(base) = without_ext.strip_prefix("test_") else {
            continue;
        };
        counts.py_test_files += 1;
        lint_test_filename(
            path,
            &repo_root,
            &harness_exempt,
            valid_sdk_test_base(base, '_'),
            "NG-03",
            "test_<domain>_<entity>[_role].py",
            &mut violations,
        );
    }

    // see: cans/artifacts/test-plan/matrix.yaml#filename_grammar
    for entry in WalkDir::new(&ts_dir)
        .into_iter()
        .filter_entry(|entry| !is_vendor_entry(entry))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.to_string_lossy().contains("/src/") {
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == "index.ts" || name.ends_with(".d.ts") || name.ends_with(".test.ts") {
            continue;
        }
        let Some(base) = name.strip_suffix(".ts") else {
            continue;
        };
        counts.ts_src_files += 1;
        lint_test_filename(
            path,
            &repo_root,
            &harness_exempt,
            valid_src_base(base, '.'),
            "NG-04",
            "<entity>.<domain>[.role].ts",
            &mut violations,
        );
    }

    // see: cans/artifacts/test-plan/matrix.yaml#filename_grammar
    let py_src_dir = repo_root.join("packages/py/kelir");
    for entry in WalkDir::new(&py_src_dir)
        .into_iter()
        .filter_entry(|entry| !is_vendor_entry(entry))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == "__init__.py" {
            continue;
        }
        let Some(base) = name.strip_suffix(".py") else {
            continue;
        };
        counts.py_src_files += 1;
        lint_test_filename(
            path,
            &repo_root,
            &harness_exempt,
            valid_src_base(base, '_'),
            "NG-05",
            "<domain>_<entity>.py",
            &mut violations,
        );
    }

    // see: cans/artifacts/test-plan/matrix.yaml#filename_grammar
    let rs_src_dir = repo_root.join("crates");
    for entry in WalkDir::new(&rs_src_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.to_string_lossy().contains("/src/") {
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == "lib.rs" || name == "main.rs" || name == "mod.rs" {
            continue;
        }
        let Some(base) = name.strip_suffix(".rs") else {
            continue;
        };
        counts.rs_src_files += 1;
        lint_test_filename(
            path,
            &repo_root,
            &harness_exempt,
            valid_src_base(base, '_'),
            "NG-06",
            "<entity>_<domain>.rs",
            &mut violations,
        );
    }

    let cases_dir = repo_root.join("cans/artifacts/test-plan/cases");
    let mut case_paths: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&cases_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "yaml") {
            case_paths.push(path.to_path_buf());
        }
    }
    case_paths.sort();
    for path in &case_paths {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        counts.yaml_cases += 1;
        let case = parse_yaml_case(&repo_root, path, &content);
        counts.yaml_snippets += case.snippets.len();
        lint_yaml_case(&case, &mut violations);
    }

    if violations.is_empty() {
        println!(
            "Anti-Slop Lint passed: {} .rs test files, {} .rs bench files, {} YAML cases, {} verification snippets, zero violations.",
            counts.rs_test_files, counts.rs_bench_files, counts.yaml_cases, counts.yaml_snippets
        );
        println!(
            "Filename grammar gate: {} rust, {} typescript, {} python test filenames conform or are harness-exempt; {} typescript, {} python, {} rust source modules conform.",
            counts.rs_test_files,
            counts.ts_test_files,
            counts.py_test_files,
            counts.ts_src_files,
            counts.py_src_files,
            counts.rs_src_files
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "ERROR: Anti-Slop Linter detected {} rule violations across {} .rs test files, {} .rs bench files, {} YAML cases ({} verification snippets):",
            violations.len(),
            counts.rs_test_files,
            counts.rs_bench_files,
            counts.yaml_cases,
            counts.yaml_snippets
        );
        for v in violations {
            eprintln!("  {}", v);
        }
        ExitCode::FAILURE
    }
}

fn parse_yaml_case(repo_root: &Path, path: &Path, content: &str) -> YamlCase {
    let rel_path = path
        .strip_prefix(repo_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    let mut case = YamlCase {
        rel_path,
        rules: Vec::new(),
        snippets: Vec::new(),
        mitigation_laws: Vec::new(),
        hypervisor: None,
        has_emulated_opt_in: false,
    };
    let mut current_rule = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let item = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(rest) = item.strip_prefix("rule:") {
            current_rule = rest.trim().trim_matches('"').to_string();
            if !case.rules.contains(&current_rule) {
                case.rules.push(current_rule.clone());
            }
        } else if let Some(rest) = trimmed.strip_prefix("verification:") {
            let text = rest.trim().trim_matches('"').to_string();
            case.snippets.push((current_rule.clone(), text));
        } else if let Some(rest) = trimmed.strip_prefix("anti_slop_mitigation:") {
            let mut start = 0usize;
            while let Some(pos) = rest[start..].find("AS-") {
                let law = &rest[start + pos..];
                let end = law
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '.')
                    .unwrap_or(law.len());
                let law = law[..end].trim_end_matches(['.', ',']).to_string();
                if law.len() > 3 && !case.mitigation_laws.contains(&law) {
                    case.mitigation_laws.push(law);
                }
                start += pos + 3;
            }
        } else if let Some(rest) = trimmed.strip_prefix("hypervisor:") {
            case.hypervisor = Some(rest.trim().trim_matches('"').to_string());
        }
        if trimmed.contains("allow_emulated_fallback: true") {
            case.has_emulated_opt_in = true;
        }
    }
    case
}

// see: cans/artifacts/test-plan/matrix.yaml#anti_slop_laws.AS-02
const AS_02_PROOF_LAWS: &[&str] = &["AS-02", "AS-05", "AS-07"];

// see: cans/artifacts/test-plan/matrix.yaml#anti_slop_laws.AS-08
const AS_08_RELEASE_TOKENS: &[&str] = &[
    "descriptor_leaked",
    "/proc/self/fd",
    "lsof",
    "scratch",
    "is_pid_alive",
    "is_address_mapped",
    "in_flight",
    "reaped",
    "rss",
    "parked",
    "termios",
    "active_vcpu",
    "TcpListener::bind",
];

// see: cans/artifacts/test-plan/matrix.yaml#anti_slop_laws.AS-05
const AS_05_CHAIN_TOKENS: &[&str] = &[
    "merkle",
    "Merkle",
    "CausalDelta",
    "dirty_bytes",
    "leaf_hash",
    "wal",
    "root",
    "chain",
];

fn lint_yaml_case(case: &YamlCase, violations: &mut Vec<String>) {
    if case.snippets.is_empty() {
        violations.push(format!(
            "{}: [AS-02] Case carries no anti_slop_assertions verification snippet.",
            case.rel_path
        ));
    }
    for (rule, snippet) in &case.snippets {
        lint_snippet(&case.rel_path, rule, snippet, violations);
    }
    for law in AS_02_PROOF_LAWS {
        if case.mitigation_laws.iter().any(|l| l == law) && !case.rules.iter().any(|r| r == law) {
            violations.push(format!(
                "{}: [{}] anti_slop_mitigation names {} but no {} verification snippet exists.",
                case.rel_path, law, law, law
            ));
        }
    }
    if case.mitigation_laws.iter().any(|l| l == "AS-08")
        && !case.rules.iter().any(|r| r == "AS-08")
        && !case
            .snippets
            .iter()
            .any(|(_, s)| AS_08_RELEASE_TOKENS.iter().any(|t| s.contains(t)))
    {
        violations.push(format!(
            "{}: [AS-08] anti_slop_mitigation names AS-08 but no AS-08 snippet or resource-release probe exists.",
            case.rel_path
        ));
    }
    if case.hypervisor.as_deref() == Some("emulated") && !case.has_emulated_opt_in {
        violations.push(format!(
            "{}: [AS-07] Emulated platform_target without allow_emulated_fallback: true.",
            case.rel_path
        ));
    }
    for (rule, snippet) in &case.snippets {
        if rule == "AS-04B"
            && (snippet.contains("Duration::from_millis")
                || snippet.contains("Duration::from_micros")
                || snippet.contains("Duration::from_nanos")
                || snippet.contains("Duration::from_secs"))
        {
            violations.push(format!(
                "{}: [AS-04B] AS-04B snippet asserts a latency bound from a hardcoded Duration literal; enforce the perf/budgets.toml threshold instead.",
                case.rel_path
            ));
        }
        if rule == "AS-05" && !AS_05_CHAIN_TOKENS.iter().any(|t| snippet.contains(t)) {
            violations.push(format!(
                "{}: [AS-05] AS-05 snippet verifies no Merkle WAL chain, CausalDelta, or leaf hash.",
                case.rel_path
            ));
        }
    }
}

fn lint_snippet(path: &str, rule: &str, snippet: &str, violations: &mut Vec<String>) {
    if snippet.contains("mockall")
        || snippet.contains("MockVmmDriver")
        || snippet.contains("MockHostInterposer")
        || snippet.contains("MockBusDispatcher")
    {
        violations.push(format!(
            "{}: [AS-01] Mocking hypervisor interfaces in verification snippet.",
            path
        ));
    }
    if (snippet.contains(".is_ok()") || snippet.contains(".is_err()"))
        && (snippet.contains("rollback")
            || snippet.contains(".exec(")
            || snippet.contains("evaluate_trap"))
    {
        violations.push(format!(
            "{}: [AS-03] Naked .is_ok()/.is_err() on execution/rollback in verification snippet ({}). Must match typed Verdict variant or ExitKind.",
            path, rule
        ));
    }
    if snippet.contains("std::time::Instant")
        || snippet.contains("Instant::now()")
        || snippet.contains("SystemTime::now()")
        || snippet.contains("thread::sleep")
        || snippet.contains("sleep(")
        || snippet.contains("thread_rng")
        || snippet.contains("rand::random")
    {
        violations.push(format!(
            "{}: [AS-04A] Wall-clock sampling, sleep, or unseeded randomness in verification snippet ({}). Pin the virtual clock or seed ChaCha20.",
            path, rule
        ));
    }
    if is_tautology(snippet) {
        violations.push(format!(
            "{}: [AS-06] Tautology in verification snippet ({}): expected value derived from the output of the function under test. Expected values must be hardcoded literals.",
            path, rule
        ));
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
    for (line_idx, line) in content.lines().enumerate() {
        if is_tautology(line) {
            violations.push(format!(
                "{}:{}: [AS-06] Tautology: expected value derived from the output of the function under test. Expected values must be hardcoded literals.",
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

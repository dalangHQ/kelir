// see: cans/01-architecture.md#Codebase-invariants
use kelir_maturity::{
    ForbidMode, Level, Policy, check_tree, crate_of, level_of, read_allowed_levels,
};
use std::fs;
use std::path::{Path, PathBuf};

const POLICY: &str = r#"
[[level.L1.forbid]]
id = "impl-block"
pattern = '^\s*impl\b'
[[level.L2.forbid]]
id = "domain-trait-impl"
pattern = '^\s*impl\s+(FuseOps)\s+for\b'
[[level.L3.forbid]]
id = "trait-decl"
pattern = '^\s*(pub\s+)?trait\b'
"#;

fn policy() -> Policy {
    let parsed = Policy::from_toml(POLICY);
    assert!(parsed.is_ok(), "test policy must parse");
    match parsed {
        Ok(p) => p,
        Err(_) => Policy::empty(),
    }
}

fn policy_mode(mode: &str) -> Policy {
    let parsed = Policy::from_toml(&format!("[policy]\nforbid_mode = \"{mode}\"\n{POLICY}"));
    assert!(parsed.is_ok(), "test policy must parse");
    match parsed {
        Ok(p) => p,
        Err(_) => Policy::empty(),
    }
}

fn tree(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kelir-maturity-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    assert!(fs::create_dir_all(&dir).is_ok());
    dir
}

fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        assert!(fs::create_dir_all(parent).is_ok());
    }
    assert!(fs::write(path, text).is_ok());
}

fn krate(root: &Path, name: &str, levels: &str) -> PathBuf {
    let dir = root.join("crates").join(name);
    write_file(
        &dir.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"{}\"\n\n[package.metadata.kelir]\nallowed_levels = [{}]\n",
            name, levels
        ),
    );
    dir
}

fn clean(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn passing_tree() {
    let root = tree("pass");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "pub struct Flow { ops: u8 }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "passing tree must have zero fails: {:?}",
        report.fails
    );
    assert_eq!(report.files, 2);
    clean(&root);
}

#[test]
fn tests_dir_excluded() {
    let root = tree("notests");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    write_file(
        &dir.join("tests").join("helper_test.rs"),
        "impl Foo for Bar {}\nuse std::fs;\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "tests/ must not be checked: {:?}",
        report.fails
    );
    assert_eq!(report.files, 1);
    clean(&root);
}

#[test]
fn placement_fails_for_l3_in_l1_l2_crate() {
    let root = tree("place");
    let dir = krate(&root, "demo", "\"L1\", \"L2\"");
    write_file(&dir.join("src").join("body.run.rs"), "pub fn run() {}\n");
    let report = check_tree(&root, &policy());
    assert_eq!(report.fails.len(), 1);
    assert_eq!(report.hard_fails, 1);
    assert!(report.fails[0].contains("body.run.rs"));
    assert!(report.fails[0].contains("L3"));
    clean(&root);
}

#[test]
fn level_of_suffixes() {
    assert_eq!(level_of(Path::new("a.rs")), Level::L1);
    assert_eq!(level_of(Path::new("a.flow.rs")), Level::L2);
    assert_eq!(level_of(Path::new("a.run.rs")), Level::L3);
    assert_eq!(level_of(Path::new("lib.rs")), Level::L1);
    assert_eq!(level_of(Path::new("mod.rs")), Level::L1);
}

#[test]
fn read_allowed_levels_parses() {
    let root = tree("levels");
    let dir = krate(&root, "demo", "\"L1\", \"L2\"");
    let levels = read_allowed_levels(&dir.join("Cargo.toml"));
    assert_eq!(levels, vec![Level::L1, Level::L2]);
    clean(&root);
}

#[test]
fn crate_of_finds_enclosing_crate() {
    let root = tree("crateof");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    let file = dir.join("src").join("deep").join("x.rs");
    write_file(&file, "pub fn x() {}\n");
    assert_eq!(crate_of(&file), dir);
    clean(&root);
}

#[test]
fn l3_todo_warns_but_passes() {
    let root = tree("todo");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("body.run.rs"),
        "// TODO (L3): fill the body\npub fn run() {}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "prepared .run.rs stubs warn on TODO until L3 fills them: {:?}",
        report.fails
    );
    assert_eq!(report.warns.len(), 1);
    assert!(report.warns[0].contains("TODO"));
    clean(&root);
}

#[test]
fn l1_impl_block_fails_in_error_mode() {
    let root = tree("impl-err");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("lib.rs"),
        "pub struct S;\nimpl S { pub fn f(&self) {} }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        !report.passed(),
        "error-mode forbid rules must fail: {:?}",
        report.fails
    );
    assert!(report
        .fails
        .iter()
        .any(|f| f.contains("[forbid:impl-block]")));
    assert_eq!(report.hard_fails, 0);
    clean(&root);
}

#[test]
fn forbid_rules_warn_in_warn_mode() {
    let root = tree("impl-warn");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("lib.rs"),
        "pub struct S;\nimpl S { pub fn f(&self) {} }\n",
    );
    let report = check_tree(&root, &policy_mode("warn"));
    assert!(
        report.passed(),
        "warn-mode rollout keeps forbid rules non-fatal: {:?}",
        report.fails
    );
    assert!(report
        .warns
        .iter()
        .any(|w| w.contains("impl-block") && w.contains("(forbid, warn-mode rollout)")));
    clean(&root);
}

#[test]
fn forbid_mode_defaults_to_error_when_absent() {
    let parsed = Policy::from_toml(POLICY);
    assert!(parsed.is_ok(), "test policy must parse");
    if let Ok(p) = parsed {
        assert_eq!(p.forbid_mode, ForbidMode::Error);
    }
}

#[test]
fn forbid_mode_rejects_unknown_value() {
    let parsed = Policy::from_toml("[policy]\nforbid_mode = \"loud\"\n");
    assert!(parsed.is_err(), "unknown forbid_mode must be a parse error");
}

#[test]
fn l2_domain_trait_impl_fails_in_error_mode() {
    let root = tree("domain");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "impl FuseOps for Flow {}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        !report.passed(),
        "error-mode forbid rules must fail: {:?}",
        report.fails
    );
    assert!(report
        .fails
        .iter()
        .any(|f| f.contains("[forbid:domain-trait-impl]")));
    clean(&root);
}

#[test]
fn l3_trait_decl_fails_in_error_mode() {
    let root = tree("trait");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("body.run.rs"), "pub trait Foo {}\n");
    let report = check_tree(&root, &policy());
    assert!(
        !report.passed(),
        "error-mode forbid rules must fail: {:?}",
        report.fails
    );
    assert!(report
        .fails
        .iter()
        .any(|f| f.contains("[forbid:trait-decl]")));
    clean(&root);
}

#[test]
fn structure_s1_l1_runtime_concern_fails() {
    let root = tree("s1");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("contract.rs"), "pub async fn boot() {}\n");
    let report = check_tree(&root, &policy());
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("contract.rs"));
    assert!(report.fails[0].contains("[structure:S1]"));
    clean(&root);
}

#[test]
fn structure_s2_oversized_aggregator_fails() {
    let root = tree("s2");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    let mut lib = String::new();
    for i in 0..120 {
        lib.push_str(&format!("pub const F{i}: u8 = 0;\n"));
    }
    lib.push_str("pub struct S;\nimpl S { pub fn f(&mut self) { let _ = F0; } }\n");
    write_file(&dir.join("src").join("lib.rs"), &lib);
    let report = check_tree(&root, &Policy::empty());
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("lib.rs"));
    assert!(report.fails[0].contains("[structure:S2]"));
    clean(&root);
}

#[test]
fn structure_s3_flow_runtime_fails() {
    let root = tree("s3");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("spawn.flow.rs"),
        "pub fn start() { std::thread::spawn(|| {}); }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("spawn.flow.rs"));
    assert!(report.fails[0].contains("[structure:S3]"));
    clean(&root);
}

#[test]
fn structure_s4_flow_io_in_l3_crate_fails() {
    let root = tree("s4");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("io.flow.rs"),
        "pub async fn load() -> Vec<u8> { std::fs::read(\"x\").unwrap_or_default() }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("io.flow.rs"));
    assert!(report.fails[0].contains("[structure:S4]"));
    clean(&root);
}

#[test]
fn structure_s5_async_mod_fails() {
    let root = tree("s5");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("deep").join("mod.rs"),
        "pub struct S;\nimpl S { pub async fn f(&mut self) {} }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert!(report
        .fails
        .iter()
        .any(|f| f.contains("mod.rs") && f.contains("[structure:S5]")));
    clean(&root);
}

#[test]
fn structure_rules_warn_in_warn_mode() {
    let root = tree("s-warn");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("contract.rs"), "pub async fn boot() {}\n");
    let report = check_tree(&root, &policy_mode("warn"));
    assert!(
        report.passed(),
        "warn-mode rollout keeps structural rules non-fatal: {:?}",
        report.fails
    );
    assert!(report
        .warns
        .iter()
        .any(|w| w.contains("contract.rs") && w.contains("(structure:S1, warn-mode rollout)")));
    clean(&root);
}

#[test]
fn shipped_policy_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("perf")
        .join("maturity-policy.toml");
    let text = fs::read_to_string(&path);
    assert!(text.is_ok(), "cannot read shipped policy");
    let text = text.unwrap_or_default();
    let policy = Policy::from_toml(&text);
    assert!(policy.is_ok(), "shipped policy must parse");
    let policy = match policy {
        Ok(p) => p,
        Err(_) => Policy::empty(),
    };
    assert_eq!(policy.l2_max_delta_per_pr, 60);
    assert_eq!(policy.forbid_mode, ForbidMode::Warn);
    assert!(policy.rules.iter().any(|r| r.id == "impl-block"));
    assert!(policy.rules.iter().any(|r| r.id == "domain-trait-impl"));
    assert!(policy.rules.iter().any(|r| r.id == "trait-decl"));
    assert!(policy.rules.iter().any(|r| r.id == "dyn-any" && !r.forbid));
}

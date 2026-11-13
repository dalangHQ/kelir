// see: cans/01-architecture.md#Codebase-invariants
use kelir_maturity::{Level, Policy, check_tree, crate_of, level_of, read_allowed_levels};
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
fn l1_impl_block_warns_but_passes() {
    let root = tree("impl");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("lib.rs"),
        "pub struct S;\nimpl S { pub fn f(&self) {} }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "content rules warn only: {:?}",
        report.fails
    );
    assert_eq!(report.warns.len(), 1);
    assert!(report.warns[0].contains("impl-block"));
    clean(&root);
}

#[test]
fn l2_domain_trait_impl_warns() {
    let root = tree("domain");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "impl FuseOps for Flow {}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "content rules warn only: {:?}",
        report.fails
    );
    assert_eq!(report.warns.len(), 1);
    assert!(report.warns[0].contains("domain-trait-impl"));
    clean(&root);
}

#[test]
fn l3_trait_decl_warns() {
    let root = tree("trait");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("body.run.rs"), "pub trait Foo {}\n");
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "content rules warn only: {:?}",
        report.fails
    );
    assert_eq!(report.warns.len(), 1);
    assert!(report.warns[0].contains("trait-decl"));
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
    assert_eq!(policy.l2_max_delta_per_pr, 20);
    assert!(policy.rules.iter().any(|r| r.id == "impl-block"));
    assert!(policy.rules.iter().any(|r| r.id == "domain-trait-impl"));
    assert!(policy.rules.iter().any(|r| r.id == "trait-decl"));
    assert!(policy.rules.iter().any(|r| r.id == "dyn-any" && !r.forbid));
}

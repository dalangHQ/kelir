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
[[level.L1.forbid]]
id = "os-reach"
pattern = '\b(libc|std::fs|std::net|std::process|std::os|tokio::fs|tokio::net|tokio::process|block_on)\b'
[[level.L2.forbid]]
id = "domain-trait-impl"
pattern = '^\s*impl\s+(FuseOps)\s+for\b'
[[level.L2.forbid]]
id = "os-reach"
pattern = '\b(libc|std::fs|std::net|std::process|std::os)\b'
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
fn concern_leak_flags_many_run_without_flow() {
    let root = tree("leak");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    for name in ["a.run.rs", "b.run.rs", "c.run.rs"] {
        write_file(&dir.join("src").join(name), "pub fn run() {}\n");
    }
    let report = check_tree(&root, &policy());
    assert!(
        !report.passed(),
        "3 .run.rs with no .flow.rs must fail: {:?}",
        report.fails
    );
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("[concern-leak:C1]"));
    assert!(report.fails[0].contains("demo"));
    clean(&root);
}

#[test]
fn concern_leak_warns_in_warn_mode() {
    let root = tree("leak-warn");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    for name in ["a.run.rs", "b.run.rs", "c.run.rs"] {
        write_file(&dir.join("src").join(name), "pub fn run() {}\n");
    }
    let report = check_tree(&root, &policy_mode("warn"));
    assert!(
        report.passed(),
        "warn-mode rollout keeps the concern-leak rule non-fatal: {:?}",
        report.fails
    );
    assert!(report.warns.iter().any(|w| w.contains("concern-leak:C1")));
    clean(&root);
}

#[test]
fn concern_leak_ignores_bin_files() {
    let root = tree("leak-bin");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    for name in ["a.run.rs", "b.run.rs", "c.run.rs"] {
        write_file(&dir.join("src").join("bin").join(name), "pub fn run() {}\n");
    }
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "bin entry points have no downstream caller to leak to: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn concern_leak_allows_two_run_files() {
    let root = tree("leak-two");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    for name in ["a.run.rs", "b.run.rs"] {
        write_file(&dir.join("src").join(name), "pub fn run() {}\n");
    }
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "fewer than 3 .run.rs files is not a leak: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn concern_leak_allows_flow_seam() {
    let root = tree("leak-seam");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(&dir.join("src").join("lib.rs"), "pub trait Ops {}\n");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "pub struct Flow { ops: u8 }\n",
    );
    for name in ["a.run.rs", "b.run.rs", "c.run.rs"] {
        write_file(&dir.join("src").join(name), "pub fn run() {}\n");
    }
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "one .flow.rs seam covers any number of .run.rs files: {:?}",
        report.fails
    );
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
    assert!(
        report
            .fails
            .iter()
            .any(|f| f.contains("[forbid:impl-block]"))
    );
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
    assert!(
        report
            .warns
            .iter()
            .any(|w| w.contains("impl-block") && w.contains("(forbid, warn-mode rollout)"))
    );
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
    assert!(
        report
            .fails
            .iter()
            .any(|f| f.contains("[forbid:domain-trait-impl]"))
    );
    clean(&root);
}

#[test]
fn shipped_policy_allows_dispatch_impls_but_not_data_plane_impls() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("perf")
        .join("maturity-policy.toml");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let shipped = Policy::from_toml(&text).unwrap_or_else(|_| Policy::empty());
    assert!(
        shipped.rules.iter().any(|r| r.id == "domain-trait-impl"),
        "shipped policy must carry the domain-trait-impl rule"
    );
    let root = tree("domain-dispatch");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("dispatch.flow.rs"),
        "impl LifecycleParticipant for Plugin { async fn on_vm_ready(&self) -> Result<(), PluginError> { self.service.ready().await } }\nimpl TrapInterposer for Plugin { async fn on_trap(&self, event: &TrapEvent) -> Result<Verdict, PluginError> { self.service.eval(event).await } }\n",
    );
    let report = check_tree(&root, &shipped);
    assert!(
        report.passed(),
        "dispatch-hook impls are L2 wiring, not L3 bodies: {:?}",
        report.fails
    );
    write_file(
        &dir.join("src").join("net.flow.rs"),
        "impl NetInterposer for Plugin { fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) } }\n",
    );
    let report = check_tree(&root, &shipped);
    assert_eq!(
        report.fails.len(),
        1,
        "a data-plane interposer impl in L2 must still fail: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("[forbid:domain-trait-impl]"));
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
    assert!(
        report
            .fails
            .iter()
            .any(|f| f.contains("[forbid:trait-decl]"))
    );
    clean(&root);
}

#[test]
fn structure_s1_l1_runtime_concern_fails() {
    let root = tree("s1");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("contract.rs"),
        "pub async fn boot() {}\n",
    );
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
    assert!(
        report
            .fails
            .iter()
            .any(|f| f.contains("mod.rs") && f.contains("[structure:S5]"))
    );
    clean(&root);
}

#[test]
fn structure_rules_warn_in_warn_mode() {
    let root = tree("s-warn");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("contract.rs"),
        "pub async fn boot() {}\n",
    );
    let report = check_tree(&root, &policy_mode("warn"));
    assert!(
        report.passed(),
        "warn-mode rollout keeps structural rules non-fatal: {:?}",
        report.fails
    );
    assert!(
        report
            .warns
            .iter()
            .any(|w| w.contains("contract.rs") && w.contains("(structure:S1, warn-mode rollout)"))
    );
    clean(&root);
}

#[test]
fn l3_stub_marker_body_passes() {
    let root = tree("s6-marker");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("body.run.rs"),
        "pub fn run() -> u8 {\n    // TODO (L3): compute the value\n    0\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(report.passed(), "marker body is a stub: {:?}", report.fails);
    assert!(
        report.fails.iter().all(|f| !f.contains("[structure:S6]")),
        "no S6 fail for a marker body: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l3_stub_meat_body_fails() {
    let root = tree("s6-meat");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("body.run.rs"),
        "pub fn run(flag: bool) -> u8 {\n    if flag {\n        1\n    } else {\n        2\n    }\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(report.fails.len(), 1);
    assert!(report.fails[0].contains("body.run.rs"));
    assert!(report.fails[0].contains("[structure:S6]"));
    assert!(report.fails[0].contains("run"));
    clean(&root);
}

#[test]
fn l3_stub_build_and_new_pass() {
    let root = tree("s6-ctor");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("body.run.rs"),
        "pub struct Built { a: u8, b: u8 }\nimpl Built {\n    pub fn new(a: u8, b: u8) -> Self { Self { a, b } }\n    pub fn build_thing() -> Self { let a = 1; let b = 2; Self { a, b } }\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "single-expression new and let-plus-literal build_* are stubs: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l3_stub_marker_only_file_passes() {
    let root = tree("s6-markonly");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("body.run.rs"),
        "// TODO (L3): fill the body\npub fn run() {}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "marker-only file is a stub: {:?}",
        report.fails
    );
    assert_eq!(report.warns.len(), 1);
    assert!(report.warns[0].contains("TODO"));
    clean(&root);
}

#[test]
fn l2_os_reach_carves_libc_constants() {
    let root = tree("os-libc");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("errno_types.flow.rs"),
        "pub enum Errno { Eperm, Enoent }\nimpl Errno {\n    pub fn raw(&self) -> i32 {\n        match self {\n            Self::Eperm => libc::EPERM,\n            Self::Enoent => libc::ENOENT,\n        }\n    }\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "pure libc constants are not IO: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l2_os_reach_carves_net_types_imports_and_process_id() {
    let root = tree("os-net");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "use std::fs;\nuse std::net::Ipv4Addr;\nuse std::os::unix::fs::MetadataExt;\npub fn pid() -> u64 { u64::from(std::process::id()) }\npub fn dirs(root: &str) { let _ = std::fs::create_dir_all(root); }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "pure net types, imports, pid reads and dir lifecycle are not guest IO: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l2_os_reach_carves_lockfile_probe() {
    let root = tree("os-lock");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "pub const HW_LOCKFILE_PATH: &str = \"/tmp/hw.lock\";\npub fn probe() -> String { std::fs::read_to_string(HW_LOCKFILE_PATH).unwrap_or_default() }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "lockfile probe reads are not guest IO: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l2_os_reach_carves_comment_mention() {
    let root = tree("os-comment");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "// L3-GAP: live probes need libc syscalls\npub struct Flow { ops: u8 }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "a comment mentioning libc does no IO: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l2_os_reach_still_flags_guest_io() {
    let root = tree("os-guest");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("wiring.flow.rs"),
        "pub fn load(path: &str) -> String { std::fs::read_to_string(path).unwrap_or_default() }\npub fn sock() { unsafe { libc::socket(1, 2, 3); } }\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(report.fails.len(), 2);
    assert!(
        report.fails.iter().all(|f| f.contains("[forbid:os-reach]")),
        "guest reads and libc calls still fail: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn structure_s4_ignores_carved_out_io() {
    let root = tree("s4-carve");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("io.flow.rs"),
        "pub async fn load() -> u64 { u64::from(std::process::id()) }\n",
    );
    write_file(
        &dir.join("src").join("probe.flow.rs"),
        "pub async fn probe() -> String { std::fs::read_to_string(HW_LOCKFILE_PATH).unwrap_or_default() }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert!(
        report.passed(),
        "pid-only and lockfile-probe IO are carved out of S4: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn structure_s3_runtime_new_and_builder_fail() {
    let root = tree("s3-rt");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("rt.flow.rs"),
        "pub fn start() { let _ = tokio::runtime::Runtime::new(); }\n",
    );
    write_file(
        &dir.join("src").join("bd.flow.rs"),
        "pub fn start() { let _ = tokio::runtime::Builder::new_multi_thread(); }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert_eq!(
        report.fails.len(),
        2,
        "Runtime::new and Builder must both trip S3: {:?}",
        report.fails
    );
    assert!(
        report.fails.iter().all(|f| f.contains("[structure:S3]")),
        "both fails are S3: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn l1_os_reach_pub_use_reexport_passes_but_call_fails() {
    let root = tree("os-pubuse");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("lib.rs"),
        "pub use runtime::{JoinSet, block_on, fs, time};\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "a pure re-export line performs no IO: {:?}",
        report.fails
    );
    write_file(
        &dir.join("src").join("load.rs"),
        "pub fn load() -> String { std::fs::read_to_string(\"x\").unwrap_or_default() }\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(
        report.fails.len(),
        1,
        "a real std::fs call in L1 must fail: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("[forbid:os-reach]"));
    clean(&root);
}

#[test]
fn os_reach_guest_io_fails_but_tooling_and_bin_pass() {
    let root = tree("os-scope");
    let guest = krate(&root, "kelir-demo-guest", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &guest.join("src").join("wiring.flow.rs"),
        "pub fn load(path: &str) -> String { std::fs::read_to_string(path).unwrap_or_default() }\n",
    );
    let tool = krate(&root, "kelir-plugin-cli", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &tool.join("src").join("wiring.flow.rs"),
        "pub fn load(path: &str) -> String { std::fs::read_to_string(path).unwrap_or_default() }\n",
    );
    let bin_krate = krate(&root, "kelir-demo-bin", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &bin_krate.join("src").join("bin").join("tool.flow.rs"),
        "pub fn load(path: &str) -> String { std::fs::read_to_string(path).unwrap_or_default() }\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(
        report.fails.len(),
        1,
        "only the guest-crate IO must fail: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("kelir-demo-guest"));
    assert!(report.fails[0].contains("[forbid:os-reach]"));
    clean(&root);
}

#[test]
fn l2_os_reach_carves_stored_path_metadata() {
    let root = tree("os-meta");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("probe.flow.rs"),
        "pub fn blocks(&self) -> u64 { std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0) }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "a read-only stat of a stored path is a probe, not guest IO: {:?}",
        report.fails
    );
    write_file(
        &dir.join("src").join("other.flow.rs"),
        "pub fn load(path: &str) -> u64 { std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) }\n",
    );
    write_file(
        &dir.join("src").join("lit.flow.rs"),
        "pub fn lit() -> u64 { std::fs::metadata(\"x\").map(|m| m.len()).unwrap_or(0) }\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(
        report.fails.len(),
        2,
        "metadata of a bare variable or a literal path must still fail: {:?}",
        report.fails
    );
    assert!(
        report.fails.iter().all(|f| f.contains("[forbid:os-reach]")),
        "both fails are os-reach: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn os_reach_named_profile_read_exempt() {
    let root = tree("os-profile");
    let dir = krate(&root, "kelir", "\"L1\", \"L2\"");
    write_file(
        &dir.join("src").join("builder_sdk.flow.rs"),
        "pub fn from_config(path: impl Into<PathBuf>) -> Result<Self, PluginError> {\n    let path = path.into();\n    let text = std::fs::read_to_string(&path).map_err(PluginError::Io)?;\n    Ok(Self)\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "the host SDK reading a caller-supplied profile path is host-side: {:?}",
        report.fails
    );
    write_file(
        &dir.join("src").join("other.flow.rs"),
        "pub fn load(path: &str) -> String {\n    let text = std::fs::read_to_string(&path).map_err(PluginError::Io)?;\n    text\n}\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(
        report.fails.len(),
        1,
        "the same read anywhere else must still fail: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("[forbid:os-reach]"));
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
    assert_eq!(policy.forbid_mode, ForbidMode::Error);
    assert!(policy.rules.iter().any(|r| r.id == "impl-block"));
    assert!(policy.rules.iter().any(|r| r.id == "domain-trait-impl"));
    assert!(policy.rules.iter().any(|r| r.id == "trait-decl"));
    assert!(policy.rules.iter().any(|r| r.id == "dyn-any" && !r.forbid));
}

#[test]
fn l1_os_reach_pure_type_passes_but_call_fails() {
    let root = tree("os-l1type");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("types.rs"),
        "pub type HvRet = libc::c_int;\npub struct Driver { child_pid: Option<libc::pid_t>, fd: std::os::unix::io::OwnedFd }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "pure libc/std::os types with no call are not IO: {:?}",
        report.fails
    );
    write_file(
        &dir.join("src").join("load.rs"),
        "pub fn blocks() -> u64 { std::fs::metadata(\"x\").map(|m| m.len()).unwrap_or(0) }\n",
    );
    let report = check_tree(&root, &policy());
    assert_eq!(
        report.fails.len(),
        1,
        "a real std::fs call in L1 must fail: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("[forbid:os-reach]"));
    clean(&root);
}

#[test]
fn os_reach_bin_main_rs_passes() {
    let root = tree("os-mainrs");
    let dir = krate(&root, "kelir-demo-shim", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("main.rs"),
        "pub fn main() { let _ = std::fs::read_to_string(\"x\"); }\n",
    );
    let report = check_tree(&root, &policy());
    assert!(
        report.passed(),
        "a crate-root binary entry point is host-side bootstrap, not guest code: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn structure_s3_block_on_only_passes() {
    let root = tree("s3-blockon");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("runtime.flow.rs"),
        "pub fn block_on<F: Future<Output = T>, T>(future: F) -> std::io::Result<T> { let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?; Ok(runtime.block_on(future)) }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert!(
        report.passed(),
        "a runtime built only inside fn block_on is wiring, not a spawned runtime: {:?}",
        report.fails
    );
    clean(&root);
}

#[test]
fn structure_s3_block_on_plus_other_spawn_fails() {
    let root = tree("s3-blockon2");
    let dir = krate(&root, "demo", "\"L1\", \"L2\", \"L3\"");
    write_file(
        &dir.join("src").join("runtime.flow.rs"),
        "pub fn block_on<F: Future<Output = T>, T>(future: F) -> std::io::Result<T> { let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?; Ok(runtime.block_on(future)) }\npub fn start() { std::thread::spawn(|| {}); }\n",
    );
    let report = check_tree(&root, &Policy::empty());
    assert_eq!(
        report.fails.len(),
        1,
        "a spawn outside block_on must still trip S3: {:?}",
        report.fails
    );
    assert!(report.fails[0].contains("[structure:S3]"));
    clean(&root);
}

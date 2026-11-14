// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
use std::collections::HashMap;

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::TrapInterposer;
use kelir_interface::protocol_wire::{PROT_EXEC, TrapEvent, TrappedSyscall, Verdict};
use kelir_plugin_testkit::block_on;
use kelir_plugin_testkit::handlers_testkit::handler_ctx;
use plugin_supply_chain::{
    AuthorizedJitEngine, PackageGuardConfig, SupplyChainConfig, SupplyChainPlugin, WxJitConfig,
};

fn trap_event(
    pid: u32,
    binary_path: &str,
    binary_blake3: [u8; 32],
    argv: Vec<String>,
    syscall: Option<TrappedSyscall>,
) -> TrapEvent {
    TrapEvent {
        seq: 1,
        pid,
        ppid: 100,
        task_id: 1,
        binary_path: binary_path.into(),
        binary_dev: 2049,
        binary_ino: 400_007,
        binary_blake3,
        argv,
        envp: HashMap::new(),
        cwd: "/workspace".into(),
        syscall,
        ..Default::default()
    }
}

fn exec_event(pid: u32, ppid: u32, binary_path: &str, argv: Vec<String>) -> TrapEvent {
    TrapEvent {
        seq: 1,
        pid,
        ppid,
        task_id: 1,
        binary_path: binary_path.into(),
        binary_dev: 2049,
        binary_ino: 400_007,
        binary_blake3: [0u8; 32],
        argv,
        envp: HashMap::new(),
        cwd: "/workspace".into(),
        syscall: None,
        ..Default::default()
    }
}

fn config_with_jit(trap_memfd: bool, trap_mprotect_exec: bool) -> SupplyChainConfig {
    SupplyChainConfig {
        wx_jit: WxJitConfig {
            trap_memfd,
            trap_mprotect_exec,
            allowed_jit_binaries: Vec::new(),
        },
        package_guard: PackageGuardConfig::default(),
    }
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_03_seccomp_unotify_wx_jit_mediation.yaml#case_id
#[test]
fn sc_03_seccomp_unotify_wx_jit_mediation() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(config_with_jit(false, true));
    let ctx = handler_ctx([0u8; 16], 1234);
    let event = trap_event(
        1234,
        "/tmp/custom_c_binary",
        [0u8; 32],
        vec!["custom_c_binary".into()],
        Some(TrappedSyscall {
            nr: 10,
            mprotect_prot: Some(PROT_EXEC),
            memfd_create_flags: None,
        }),
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Ewxviolation,
            ..
        } => (),
        verdict => {
            return Err(format!(
                "unverified mprotect(PROT_EXEC) must drop with EWXVIOLATION, got {verdict:?}"
            ));
        }
    }
    let open = SupplyChainPlugin::new(SupplyChainConfig::default());
    match block_on(open.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Pass => (),
        verdict => {
            return Err(format!(
                "with W^X mediation off the same trap must pass, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_04_verified_jit_catalog_token_grant.yaml#case_id
#[test]
fn sc_04_verified_jit_catalog_token_grant() -> Result<(), String> {
    let config = SupplyChainConfig {
        wx_jit: WxJitConfig {
            trap_memfd: false,
            trap_mprotect_exec: true,
            allowed_jit_binaries: vec![AuthorizedJitEngine {
                binary_path: "/usr/bin/node".into(),
                binary_blake3: [0x42; 32],
            }],
        },
        package_guard: PackageGuardConfig::default(),
    };
    let plugin = SupplyChainPlugin::new(config);
    let ctx = handler_ctx([0u8; 16], 1234);
    let event = trap_event(
        1234,
        "/usr/bin/node",
        [0x42; 32],
        vec!["node".into()],
        Some(TrappedSyscall {
            nr: 10,
            mprotect_prot: Some(PROT_EXEC),
            memfd_create_flags: None,
        }),
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Pass => (),
        verdict => {
            return Err(format!(
                "verified JIT engine mprotect(PROT_EXEC) must pass with a token, got {verdict:?}"
            ));
        }
    }
    let token = plugin
        .ctx()
        .consume_wx_token(1)
        .map_err(|err| format!("{err:?}"))?;
    assert_eq!(token.nonce, 1);
    assert_eq!(token.binary_blake3, [0x42; 32]);
    match plugin.ctx().consume_wx_token(1) {
        Err(err) if format!("{err}").contains("EWXVIOLATION") => (),
        Err(err) => {
            return Err(format!(
                "second consume of the same token must fail with EWXVIOLATION, got {err:?}"
            ));
        }
        Ok(token) => {
            return Err(format!(
                "one-time W^X token was consumed twice; nonce {} must be spent",
                token.nonce
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_05_unverified_binary_jit_ewxviolation.yaml#case_id
#[test]
fn sc_05_unverified_binary_jit_ewxviolation() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(config_with_jit(true, false));
    let ctx = handler_ctx([0u8; 16], 2403);
    let event = trap_event(
        2403,
        "/tmp/custom_c_binary",
        [0u8; 32],
        vec!["custom_c_binary".into()],
        Some(TrappedSyscall {
            nr: 319,
            mprotect_prot: None,
            memfd_create_flags: Some(0),
        }),
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Ewxviolation,
            reason,
        } => {
            assert!(
                reason.contains("2403"),
                "drop reason must name the pid: {reason}"
            );
        }
        verdict => {
            return Err(format!(
                "unverified memfd_create must drop with EWXVIOLATION, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_08_memfd_create_dynamic_elf_block.yaml#case_id
#[test]
fn sc_08_memfd_create_dynamic_elf_block() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(config_with_jit(true, false));
    let ctx = handler_ctx([0u8; 16], 2403);
    let event = trap_event(
        2403,
        "/tmp/elf-loader",
        [0x07; 32],
        vec!["elf-loader".into()],
        Some(TrappedSyscall {
            nr: 319,
            mprotect_prot: None,
            memfd_create_flags: Some(1),
        }),
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Ewxviolation,
            ..
        } => (),
        verdict => {
            return Err(format!(
                "unverified memfd_create for in-memory ELF load must drop with EWXVIOLATION, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_09_whitelisted_interpreter_entropy_bypass.yaml#case_id
#[test]
fn sc_09_whitelisted_interpreter_entropy_bypass() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(SupplyChainConfig::default());
    let ctx = handler_ctx([0u8; 16], 1234);
    let event = trap_event(
        1234,
        "/usr/bin/python",
        [0x42; 32],
        vec![
            "python".into(),
            "-c".into(),
            "aGVsbG8gd29ybGQhIGdob3N0IHNlY3JldCBpcyBub3QgaGVyZSBidXQgdGhlIGFyZ3YgaXMgbG9uZyBhbmQgaGlnaCBlbnRyb3B5".into(),
        ],
        None,
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Pass => (),
        verdict => {
            return Err(format!(
                "whitelisted interpreter with high-entropy argv must pass, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_01_lifecycle_script_trap_exit126.yaml#case_id
#[test]
#[ignore]
fn sc_01_lifecycle_script_trap_exit126() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(SupplyChainConfig::default());
    let ctx = handler_ctx([0u8; 16], 4100);
    let event = exec_event(
        4100,
        100,
        "node_modules/malicious-pkg/postinstall.sh",
        vec![
            "sh".into(),
            "node_modules/malicious-pkg/postinstall.sh".into(),
        ],
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Eacces,
            reason,
        } => {
            assert!(
                reason.contains("lifecycle script denied")
                    && reason.contains("node_modules/malicious-pkg/postinstall.sh"),
                "deny must name the blocked lifecycle script: {reason}"
            );
        }
        verdict => {
            return Err(format!(
                "lifecycle script execve must be dropped with EACCES (exit 126), got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_02_argv_high_entropy_obfuscation_kill.yaml#case_id
#[test]
#[ignore]
fn sc_02_argv_high_entropy_obfuscation_kill() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(SupplyChainConfig::default());
    let ctx = handler_ctx([0u8; 16], 4200);
    let payload = "R4vK7xPz2QmW9tLbYcN8fGhJ5sD_aXeU3vZrT6wAyBnC1oEiM0pSqO7kFdH2gJuL-V+ZxWcPqNmTbYvErfCgXdKsAhUoIeDwLjMzNyBtGcSapQok";
    let event = exec_event(
        4200,
        100,
        "/bin/sh",
        vec!["sh".into(), "-c".into(), payload.to_string()],
    );
    match block_on(plugin.on_trap(&event, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Eacces,
            reason,
        } => {
            assert!(
                reason.contains("obfuscated argv denied"),
                "deny must name the obfuscated argv kill: {reason}"
            );
        }
        verdict => {
            return Err(format!(
                "high-entropy obfuscated argv (Shannon > 5.2 bits/char) must be dropped with EACCES, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_06_package_guard_pip_lifecycle_block.yaml#case_id
#[test]
#[ignore]
fn sc_06_package_guard_pip_lifecycle_block() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(SupplyChainConfig::default());
    let ctx = handler_ctx([0u8; 16], 4300);
    let pip = exec_event(
        4300,
        100,
        "pip",
        vec!["pip".into(), "install".into(), "malicious-wheel.whl".into()],
    );
    match block_on(plugin.on_trap(&pip, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Pass => (),
        verdict => {
            return Err(format!("pip install itself must pass, got {verdict:?}"));
        }
    }
    let build = exec_event(
        4301,
        4300,
        "setup.py",
        vec!["python".into(), "setup.py".into(), "build".into()],
    );
    match block_on(plugin.on_trap(&build, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Eacces,
            reason,
        } => {
            assert!(
                reason.contains("lifecycle script denied") && reason.contains("setup.py"),
                "deny must name the blocked setup.py build: {reason}"
            );
        }
        verdict => {
            return Err(format!(
                "setup.py lifecycle build under pip install must be dropped with EACCES (exit 126), got {verdict:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/supply_chain_security/sc_07_nested_script_ancestor_trap.yaml#case_id
#[test]
#[ignore]
fn sc_07_nested_script_ancestor_trap() -> Result<(), String> {
    let plugin = SupplyChainPlugin::new(SupplyChainConfig::default());
    let ctx = handler_ctx([0u8; 16], 4400);
    let npm = exec_event(4400, 100, "npm", vec!["npm".into(), "install".into()]);
    match block_on(plugin.on_trap(&npm, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Pass => (),
        verdict => {
            return Err(format!("npm install itself must pass, got {verdict:?}"));
        }
    }
    let sh = exec_event(
        4401,
        4400,
        "/bin/sh",
        vec!["/bin/sh".into(), "-c".into(), "node payload.js".into()],
    );
    match block_on(plugin.on_trap(&sh, &ctx)).map_err(|err| format!("{err:?}"))? {
        Verdict::Drop {
            errno: Errno::Eacces,
            reason,
        } => {
            assert!(
                reason.contains("lifecycle script denied") && reason.contains("/bin/sh"),
                "deny must name the trapped sh ancestor: {reason}"
            );
        }
        verdict => {
            return Err(format!(
                "the /bin/sh ancestor under npm must be dropped with EACCES before any node payload execve, got {verdict:?}"
            ));
        }
    }
    Ok(())
}

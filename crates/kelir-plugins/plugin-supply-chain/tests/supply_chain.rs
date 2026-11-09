// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use kelir_interface::plugin::TrapInterposer;
use kelir_interface::protocol::{PROT_EXEC, TrapEvent, TrappedSyscall, Verdict};
use kelir_plugin_testkit::handlers::handler_ctx;
use plugin_supply_chain::{
    AuthorizedJitEngine, PackageGuardConfig, SupplyChainConfig, SupplyChainPlugin, WxJitConfig,
};

// see: cans/15-operations.md#Local-velocity
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

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
            errno: Some(errno), ..
        } if errno == "EWXVIOLATION" => (),
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
        Err(err) if format!("{err:?}").contains("EWXVIOLATION") => (),
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
            errno: Some(errno),
            reason,
        } if errno == "EWXVIOLATION" => {
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
            errno: Some(errno), ..
        } if errno == "EWXVIOLATION" => (),
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

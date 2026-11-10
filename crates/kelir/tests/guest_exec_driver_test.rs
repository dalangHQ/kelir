// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_02_wx_mmu_prot_exec_trap.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::{VmConfig, WxPolicy, default_vm_config};
use kelir::plugin_model::PluginError;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_02_wx_mmu_prot_exec_trap.yaml#case_id
fn wx_config(driver: Option<&str>, allow_emulated: bool) -> VmConfig {
    let mut config = default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    );
    config.driver_policy.driver = driver.map(str::to_string);
    config.driver_policy.allow_emulated_fallback = allow_emulated;
    config.driver_policy.wx_enforcement = WxPolicy::Strict;
    config.deterministic_clock = driver.is_none();
    config
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_02_wx_mmu_prot_exec_trap.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_02_wx_mmu_prot_exec_trap() -> Result<(), String> {
    let session = KelirBuilder::new(wx_config(None, false))
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session.spawn("/bin/exploit_jit", &[]).await {
        Ok(result) => Err(format!(
            "anonymous PROT_EXEC exec completed with exit {} instead of the E_WX_VIOLATION Stage-2 fault",
            result.exit_code
        )),
        Err(PluginError::Halt(message)) => {
            if message.contains("E_WX_VIOLATION") {
                Ok(())
            } else {
                Err(format!(
                    "exec of an anonymous PROT_EXEC binary must halt with E_WX_VIOLATION under Strict W^X; got: {message}"
                ))
            }
        }
        Err(other) => Err(format!("wrong error kind for the W^X exec trap: {other}")),
    }
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_10_proot_ptrace_trap_conformance.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_10_proot_ptrace_trap_conformance() -> Result<(), String> {
    let session = KelirBuilder::new(wx_config(Some("proot"), true))
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session.spawn("/bin/uname", &[]).await {
        Ok(result) => {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "proot ptrace-interposed exec exited {} instead of 0",
                    result.exit_code
                ))
            }
        }
        Err(e) => Err(format!("proot ptrace exec never dispatched: {e}")),
    }
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_14_guest_kernel_hardening_kptr_restrict.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_14_guest_kernel_hardening_kptr_restrict() -> Result<(), String> {
    let session = KelirBuilder::new(wx_config(None, false))
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session
        .spawn_builder("/bin/cat")
        .args(["/bin/cat", "/proc/kallsyms"])
        .await
    {
        Ok(result) => {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "kptr_restrict=2 read of /proc/kallsyms exited {} instead of 0",
                    result.exit_code
                ))
            }
        }
        Err(e) => Err(format!("guest exec of /bin/cat never dispatched: {e}")),
    }
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_18_rosetta_binfmt_registration.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_18_rosetta_binfmt_registration() -> Result<(), String> {
    let session = KelirBuilder::new(wx_config(None, false))
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    match session.spawn("/bin/x86_64_static_elf", &[]).await {
        Ok(result) => {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "x86_64 exec via the Rosetta binfmt handler exited {} instead of 0",
                    result.exit_code
                ))
            }
        }
        Err(e) => Err(format!(
            "x86_64 guest exec never dispatched through /run/rosetta: {e}"
        )),
    }
}

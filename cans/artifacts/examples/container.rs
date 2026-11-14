// see: cans/plugins/container.md#Plugin-container
// see: cans/plugins/container.md#Non-nested-OCI-container-execution
// see: cans/plugins/container.md#Rootless-container-execution
// see: cans/01-architecture.md#VirtIO-Device-Complex
// STATUS: aspirational; see §4 of work-order
//   L3-GAP: ContainerService::ingest_image cannot be invoked yet — the trait
//   takes &mut self, ContainerCtx::service only hands out a shared Arc, and
//   the trait is not re-exported by the kelir facade; the boot/spawn path
//   below is real and stays.

use kelir::{
    ContainerConfig, ContainerCtx, ContainerPlugin, HCons, HNil, Host, KelirBuilder, NoParams,
    Unsealed, VmConfig,
    driver_interface::{DriverPolicy, WxPolicy},
};

const OCI_TARBALL: &str = "./fixtures/oci-hello-world.tar";

fn container_builder(
) -> KelirBuilder<Host, Unsealed, (), HNil, HCons<ContainerCtx, HNil>, NoParams> {
    KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-container.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    .plugin_instance(ContainerPlugin::new(ContainerConfig {
        runtime: "crun".into(),
        isolate_var_lib_containers: true,
        allow_unsandboxed: false,
        pin_subuid_ranges: true,
        scratch_device: None,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = container_builder().seal()?.boot().await?;

    let _container = session.cap::<ContainerCtx>();    let image = OCI_TARBALL;

    let run = session
        .spawn(
            "podman",
            &["run", "--rm", "--rootless", image, "echo", "hello from the microVM"],
        )
        .await?;
    if run.exit_code != 0 {
        return Err(format!("container run failed: {}", run.exit_code).into());
    }
    eprintln!("[kelir] container ok: {image}");

    // see: cans/plugins/container.md#Privileged-container-escape-attempt-or-raw-device-mount-rejects-with-EPERM
    let escape = session
        .spawn(
            "podman",
            &[
                "run",
                "--rm",
                "--privileged",
                "--device",
                "/dev/kvm",
                image,
                "true",
            ],
        )
        .await?;
    if escape.exit_code == 0 {
        return Err("privileged escape unexpectedly succeeded".into());
    }
    eprintln!(
        "[kelir] privileged attempt rejected: exit={} (EPERM)",
        escape.exit_code
    );

    session.stop().await?;
    Ok(())
}

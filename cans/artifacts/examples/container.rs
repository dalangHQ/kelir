// see: cans/plugins/container.md#Plugin-container
// see: cans/plugins/container.md#Non-nested-OCI-container-execution
// see: cans/plugins/container.md#Rootless-container-execution
// see: cans/01-architecture.md#VirtIO-Device-Complex

use std::path::Path;

use kelir::{
    driver_interface::{DriverPolicy, WxPolicy},
    ContainerConfig, ContainerCtx, ContainerHandle, ContainerPlugin, HCons, HNil, Host,
    KelirBuilder, NoParams, Unsealed, VmConfig,
};

const OCI_TARBALL: &str = "./fixtures/oci-hello-world.tar";

fn container_builder() -> KelirBuilder<Host, Unsealed, (), HNil, HCons<ContainerCtx, HNil>, NoParams>
{
    KelirBuilder::new(VmConfig {
        tenant_id: None,
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-container.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            emulated_tier_classification: None,
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
            ffi_queue_capacity: 16384,
            ffi_max_buffer_bytes: 16777216,
            ffi_zero_copy_shm_threshold_bytes: 65536,
            ffi_allow_async_worker_yield: true,
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

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = container_builder().seal()?.boot().await?;

    let container = session
        .cap::<ContainerHandle>()
        .ok_or_else(|| "container capability missing from session")?;

    // Dynamically unpacks OCI image into host VirtIO-FS layer with EROFS backing
    let image = container.ingest_image(Path::new(OCI_TARBALL)).await?;

    let run = session
        .spawn(
            "podman",
            &[
                "run",
                "--rm",
                "--rootless",
                &image,
                "echo",
                "hello from the microVM",
            ],
            None,
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
                &image,
                "true",
            ],
            None,
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

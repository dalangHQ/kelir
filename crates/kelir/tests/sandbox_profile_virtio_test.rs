// see: cans/artifacts/test-plan/cases/virtio_rings/vr_04_virtio_rng_profile_immutable.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::plugin_model::PluginError;
use kelir::sandbox_runtime::SandboxProfile;
use plugin_profile::ProfilePlugin;

fn rng_disabled_profile_toml() -> &'static str {
    "version = 1\n\n[session]\n\n[session.artifacts]\nkernel_path = \"kernel/vmlinux\"\nrootfs_erofs_path = \"rootfs.erofs\"\n\n[session.devices.virtio_rng]\nenabled = false\nsource = \"host\"\n"
}

// see: cans/artifacts/test-plan/cases/virtio_rings/vr_04_virtio_rng_profile_immutable.yaml#case_id
#[tokio::test]
#[ignore]
async fn vr_04_virtio_rng_profile_immutable() -> Result<(), String> {
    let profile: SandboxProfile =
        toml::from_str(rng_disabled_profile_toml()).map_err(|e| e.to_string())?;
    let config = profile.to_vm_config();
    let builder = KelirBuilder::new(config).plugin(ProfilePlugin::new());
    match builder.boot().await {
        Err(PluginError::Halt(message)) => {
            assert!(
                message.contains("EConfig") && message.contains("virtio-rng"),
                "rng-stripping profile rejected with the wrong diagnostic: {message}"
            );
        }
        Err(e) => return Err(format!("rng-stripping profile gave wrong error: {e}")),
        Ok(_) => {
            return Err(
                "profile with session.devices.virtio_rng.enabled=false booted; SandboxProfile::to_vm_config silently drops the devices section so no EConfig virtio-rng-mandatory rejection exists"
                    .to_string(),
            )
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_15_vcpu_scale_online_offline_dynamic.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::default_vm_config;

// see: cans/artifacts/test-plan/cases/hypervisor_driver/hd_15_vcpu_scale_online_offline_dynamic.yaml#case_id
#[tokio::test]
#[ignore]
async fn hd_15_vcpu_scale_online_offline_dynamic() -> Result<(), String> {
    let config = default_vm_config(
        Some(2),
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    );
    let session = KelirBuilder::new(config)
        .seal()
        .map_err(|e| e.to_string())?
        .boot()
        .await
        .map_err(|e| e.to_string())?;
    let vm = session.vm_handle();
    match vm.set_vcpus(4).await {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "dynamic vCPU scale-up from 2 to 4 never parked/unparked host threads: {e}"
        )),
    }
}

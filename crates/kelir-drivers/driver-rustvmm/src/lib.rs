#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-rustvmm
// TODO (L3) [hd_03_hardware_rollback_timing_bound]: tests/test_cow_fork.rs + test_rollback.rs per cans/artifacts/config/project-structure.txt#kelir-drivers; slop: rollback test absent, 8ms reset bound never measured
pub mod host_probe;

use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

#[allow(dead_code)]
pub struct RustVmmDriver {
    #[cfg(target_os = "linux")]
    kvm_fd: Option<kvm_ioctls::Kvm>,
    #[cfg(target_os = "linux")]
    vm_fd: Option<kvm_ioctls::VmFd>,
    #[cfg(target_os = "linux")]
    vcpu_fds: Vec<kvm_ioctls::VcpuFd>,
    #[cfg(target_os = "linux")]
    uffd: Option<userfaultfd::Uffd>,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}

impl RustVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        #[cfg(not(target_os = "linux"))]
        return Err(VmmError::Unsupported(
            "driver-rustvmm requires Linux".into(),
        ));
        #[cfg(target_os = "linux")]
        Ok(Self {
            kvm_fd: None,
            vm_fd: None,
            vcpu_fds: Vec::new(),
            uffd: None,
            config: None,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    fn init_backend(&mut self) -> Result<(), VmmError> {
        // TODO (L3) [hd_04_cold_boot_budget_enforcement]: initialize KVM, VM, memory slots, and vCPUs via KVM_CREATE_VM; slop: boot Ok without KVM_CREATE_VM, 50ms budget never measured
        Err(VmmError::Unsupported(
            "rustvmm boot: KVM_CREATE_VM init (L3)".into(),
        ))
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn filter_synthetic_cpuid(&self, _vcpu: &kvm_ioctls::VcpuFd) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#Driver-backends
        // TODO (L3) [hd_11_cpuid_tsc_deadline_normalization]: enforce invariant TSC and Leaf 0x1 TSC-deadline; slop: guest CPUID passes host leaves through, TSC-deadline bit unset
        Err(VmmError::Unsupported(
            "rustvmm cpuid filter: synthetic CPUID enforcement (L3)".into(),
        ))
    }
}

#[async_trait::async_trait]
impl VmmDriver for RustVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "rustvmm",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: true,
            supports_cow_fork: true,
        }
    }

    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        // see: cans/15-operations.md#Developer-CLI
        check_hw_lockfile(&config.driver_policy, &self.capabilities())?;
        self.config = Some(config);
        self.init_backend()
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        // TODO (L3) [vr_03_virtio_rng_mandatory_entropy]: swap the virtio-rng device model for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn; slop: seeded stream never wired, host getrandom passed off as guest entropy
        self.entropy_backend = Some(backend);
        self.entropy_offset = 0;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        // TODO (L3) [vr_03_virtio_rng_mandatory_entropy]: report the live seeded-stream offset; host entropy is not position-trackable; slop: offset 0 reported while draws advance, replay diverges silently
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => Ok(EntropyStreamPosition {
                offset_bytes: self.entropy_offset,
            }),
            _ => Err(VmmError::Unsupported(
                "rustvmm entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        // TODO (L3) [vr_03_virtio_rng_mandatory_entropy]: rewind the seeded stream to the restored offset before the next draw; slop: rewind sets offset field only, stream continues from live position
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => {
                self.entropy_offset = position.offset_bytes;
                Ok(())
            }
            _ => Err(VmmError::Unsupported(
                "rustvmm restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }
}

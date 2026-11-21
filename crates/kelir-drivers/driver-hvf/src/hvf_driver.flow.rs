// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

use crate::HvfDriver;

impl HvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            vcpu_ids: Vec::new(),
            memory_mapped_bytes: 0,
            config: None,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    // see: cans/01-architecture.md#Driver-backends
    pub fn vcpu_count(&self) -> usize {
        self.vcpu_ids.len()
    }

    // see: cans/01-architecture.md#Driver-backends
    pub fn memory_mapped_bytes(&self) -> usize {
        self.memory_mapped_bytes
    }

    fn create_vm(&mut self) -> Result<(), VmmError> {
        // TODO: [hd_04_cold_boot_budget_enforcement]: call sys_driver::hv_vm_create(0) inside an unsafe block, then sys_driver::hv_vcpu_create per vCPU; slop: boot Ok without hv_vm_create, 50ms budget never measured (deferred to L3)
        Err(VmmError::Unsupported(
 "hvf boot: hv_vm_create + hv_vcpu_create".into(),
        ))
    }

    fn run_vcpu(&mut self) -> Result<(), VmmError> {
        // TODO: [hd_04_cold_boot_budget_enforcement]: call sys_driver::hv_vcpu_run on each vcpu handle inside an unsafe block; slop: vCPU never runs yet boot reports first exec within budget (deferred to L3)
 Err(VmmError:Unsupported("hvf run: hv_vcpu_run".into()))
    }

    fn protect_fork(&self) -> Result<(), VmmError> {
        // TODO: [lc_01_cow_fork_warm_memory_sharing]: in-process Stage-2 CoW fork via sys_driver::hv_vm_protect, never an OS fork() call on multi-threaded hosts; slop: OS fork() in a multi-threaded Tokio/NAPI host SIGABRTs before the child runs, and fork shares pages without Stage-2 write protection, child mutates parent (deferred to L3)
        Err(VmmError::Unsupported(
 "hvf fork: hv_vm_protect CoW fork".into(),
        ))
    }

    fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: [hd_06_hvf_stage2_chunk_promoter]: promote 2MB contiguous blocks via Mach vm_copy CoW; slop: 4KB pages promoted as 2MB chunks, chunk size never asserted (deferred to L3)
        Err(VmmError::Unsupported(
 "hvf stage-2 promotion: Mach vm_copy CoW".into(),
        ))
    }
}

#[async_trait::async_trait]
impl VmmDriver for HvfDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "hvf",
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
        self.create_vm()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.protect_fork()?;
        // TODO: [lc_01_cow_fork_warm_memory_sharing]: clone VM state via in-process hv_vm_protect Stage-2 CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs, never an OS fork() call; slop: OS fork() in a multi-threaded host aborts, child gets fresh state, warm parent memory not inherited (deferred to L3)
        Err(VmmError::Unsupported(
 "hvf fork: CoW child driver".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        // TODO: [vr_03_virtio_rng_mandatory_entropy]: swap the virtio-rng device model for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn; slop: seeded stream never wired, host getrandom passed off as guest entropy (deferred to L3)
        self.entropy_backend = Some(backend);
        self.entropy_offset = 0;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        // TODO: [vr_03_virtio_rng_mandatory_entropy]: report the live seeded-stream offset; host entropy is not position-trackable; slop: offset 0 reported while draws advance, replay diverges silently (deferred to L3)
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => Ok(EntropyStreamPosition {
                offset_bytes: self.entropy_offset,
            }),
            _ => Err(VmmError::Unsupported(
 "hvf entropy_position: host entropy stream is not position-trackable".into(),
            )),
        }
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        // TODO: [vr_03_virtio_rng_mandatory_entropy]: rewind the seeded stream to the restored offset before the next draw; slop: rewind sets offset field only, stream continues from live position (deferred to L3)
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => {
                self.entropy_offset = position.offset_bytes;
                Ok(())
            }
            _ => Err(VmmError::Unsupported(
 "hvf restore_entropy_position: host entropy stream is not position-trackable"
                    .into(),
            )),
        }
    }
}

// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-hvf
pub mod sys_driver;

use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

#[allow(dead_code)]
pub struct HvfDriver {
    vcpu_ids: Vec<sys_driver::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}

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
        // TODO (L3): call sys_driver::hv_vm_create(0) inside an unsafe block, then sys_driver::hv_vcpu_create per vCPU.
        Err(VmmError::Unsupported(
            "hvf boot: hv_vm_create + hv_vcpu_create (L3)".into(),
        ))
    }

    #[allow(dead_code)]
    fn run_vcpu(&mut self) -> Result<(), VmmError> {
        // TODO (L3): call sys_driver::hv_vcpu_run on each vcpu handle inside an unsafe block.
        Err(VmmError::Unsupported("hvf run: hv_vcpu_run (L3)".into()))
    }

    fn protect_fork(&self) -> Result<(), VmmError> {
        // TODO (L3): Mach copy-on-write + Stage-2 write protection via sys_driver::hv_vm_protect.
        Err(VmmError::Unsupported(
            "hvf fork: hv_vm_protect CoW fork (L3)".into(),
        ))
    }

    #[allow(dead_code)]
    fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO (L3): promote 2MB contiguous blocks via Mach vm_copy CoW.
        Err(VmmError::Unsupported(
            "hvf stage-2 promotion: Mach vm_copy CoW (L3)".into(),
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
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "hvf fork: CoW child driver (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        // TODO (L3): swap the virtio-rng device model for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn.
        self.entropy_backend = Some(backend);
        self.entropy_offset = 0;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        // TODO (L3): report the live seeded-stream offset; host entropy is not position-trackable.
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => Ok(EntropyStreamPosition {
                offset_bytes: self.entropy_offset,
            }),
            _ => Err(VmmError::Unsupported(
                "hvf entropy_position: host entropy stream is not position-trackable (L3)".into(),
            )),
        }
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        // TODO (L3): rewind the seeded stream to the restored offset before the next draw.
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => {
                self.entropy_offset = position.offset_bytes;
                Ok(())
            }
            _ => Err(VmmError::Unsupported(
                "hvf restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }
}

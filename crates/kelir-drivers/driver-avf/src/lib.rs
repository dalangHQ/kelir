#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-avf
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}

impl AvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            avf_fd: -1,
            config: None,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    // see: cans/01-architecture.md#Driver-backends
    pub fn avf_fd(&self) -> i32 {
        self.avf_fd
    }

    fn open_node(&mut self) -> Result<(), VmmError> {
        // TODO (L3) [hd_08_avf_pkvm_boundary_enforcement]: connect /dev/avf or the pKVM node; slop: un-donated memory maps without E_PKVM_NON_DONATED_MEMORY fault
        Err(VmmError::Unsupported("avf boot: /dev/avf open (L3)".into()))
    }
}

#[async_trait::async_trait]
impl VmmDriver for AvfDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "avf",
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
        self.open_node()
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
                "avf entropy_position: host entropy stream is not position-trackable (L3)".into(),
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
                "avf restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }
}

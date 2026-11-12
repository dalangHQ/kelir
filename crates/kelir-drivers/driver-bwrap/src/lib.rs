#![forbid(unsafe_code)]
// TODO (L3) [lc_12_host_reaper_pdeathsig_enforcement]: read BwrapDriver.child_pid once the supervisor reaps the bwrap child; slop: child pid stored but never read
#![allow(dead_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-bwrap
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile, reject_emulated_deterministic_clock,
};

pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}

impl BwrapDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            child_pid: None,
            config: None,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    fn spawn_namespace(&mut self) -> Result<(), VmmError> {
        // TODO (L3) [hd_09_bwrap_emulated_fallback_opt_in]: spawn bubblewrap/unshare user namespace via raw libc; slop: namespace spawns without opt-in check, silent emulated boot
        Err(VmmError::Unsupported(
            "bwrap boot: bubblewrap namespace spawn (L3)".into(),
        ))
    }
}

#[async_trait::async_trait]
impl VmmDriver for BwrapDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "bwrap",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        // see: cans/15-operations.md#Developer-CLI
        check_hw_lockfile(&config.driver_policy, &self.capabilities())?;
        if !self.allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        // TODO (L3) [hypervisor_driver]: deterministic replay needs hypervisor clock control (kvm-clock/KVM_SET_CLOCK); a namespace wrapper cannot provide it; slop: emulated clock faked as kvm-clock, replay timestamps diverge
        reject_emulated_deterministic_clock("bwrap", config.deterministic_clock)?;
        self.config = Some(config);
        self.spawn_namespace()
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        // TODO (L3) [vr_03_virtio_rng_mandatory_entropy]: swap the workload entropy source for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn; slop: seeded stream never wired, host getrandom passed off as guest entropy
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
                "bwrap entropy_position: host entropy stream is not position-trackable (L3)".into(),
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
                "bwrap restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }
}

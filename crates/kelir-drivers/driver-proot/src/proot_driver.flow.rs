// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile, reject_emulated_deterministic_clock,
};

use crate::ProotDriver;

impl ProotDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            config: None,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    fn init_tracer(&mut self) -> Result<(), VmmError> {
        // TODO: [hd_10_proot_ptrace_trap_conformance]: initialize proot ptrace tracing harness; slop: exec runs without ptrace trap, TrapEvent never generated (deferred to L3)
        Err(VmmError::Unsupported(
 "proot boot: ptrace tracing harness init".into(),
        ))
    }

    fn seccomp_install_probe(&self) -> Result<(), VmmError> {
        // TODO: [hypervisor_driver]: live behavioral probe installing the seccomp filter and confirming the kernel traps instead of silently blocking the install; slop: static /dev checks report a working filter while the kernel silently blocks it. (deferred to L3)
        // TODO: [hypervisor_driver]: inherit the identical BPF syscall and socket-family set as kelir-shim DefenseFlags (io_uring and keyrings blocked, AF_ALG/AF_RXRPC/NETLINK_XFRM denied, deny_abi_compat).; slop: driver filter diverges from the shim filter, the same syscall is trapped in one and passed in the other. (deferred to L3)
        Err(VmmError::Unsupported(
 "proot pre-boot probe: seccomp_install_probe".into(),
        ))
    }

    fn wait_killable_probe(&self) -> Result<(), VmmError> {
        // TODO: [hypervisor_driver]: live behavioral probe confirming a trapped syscall waits killably instead of hanging the guest unkillably; slop: static checks report a live trap while the guest hangs unkillably. (deferred to L3)
        // TODO: [hypervisor_driver]: inherit the identical BPF syscall and socket-family set as kelir-shim DefenseFlags (io_uring and keyrings blocked, AF_ALG/AF_RXRPC/NETLINK_XFRM denied, deny_abi_compat).; slop: driver filter diverges from the shim filter, the same syscall is trapped in one and passed in the other. (deferred to L3)
        Err(VmmError::Unsupported(
 "proot pre-boot probe: wait_killable_probe".into(),
        ))
    }
}

#[async_trait::async_trait]
impl VmmDriver for ProotDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "proot",
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
        // TODO: [hypervisor_driver]: deterministic replay needs hypervisor clock control (kvm-clock/KVM_SET_CLOCK); a ptrace tracer cannot intercept VDSO clock reads; slop: VDSO clock reads unintercepted, replay timestamps diverge (deferred to L3)
        reject_emulated_deterministic_clock("proot", config.deterministic_clock)?;
        self.config = Some(config);
        self.seccomp_install_probe()?;
        self.wait_killable_probe()?;
        self.init_tracer()
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        // TODO: [vr_03_virtio_rng_mandatory_entropy]: swap the workload entropy source for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn; slop: seeded stream never wired, host getrandom passed off as guest entropy (deferred to L3)
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
 "proot entropy_position: host entropy stream is not position-trackable".into(),
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
 "proot restore_entropy_position: host entropy stream is not position-trackable"
                    .into(),
            )),
        }
    }
}

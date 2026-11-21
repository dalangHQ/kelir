// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::VmmError;

use crate::ProotDriver;

impl ProotDriver {
    // TODO (L3) [hd_10_proot_ptrace_trap_conformance]: initialize proot ptrace tracing harness; slop: exec runs without ptrace trap, TrapEvent never generated
    pub(crate) fn init_tracer(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "proot boot: ptrace tracing harness init".into(),
        ))
    }

    // TODO (L3) [hypervisor_driver]: live behavioral probe installing the seccomp filter and confirming the kernel traps instead of silently blocking the install; slop: static /dev checks report a working filter while the kernel silently blocks it.
    // TODO (L3) [hypervisor_driver]: inherit the identical BPF syscall and socket-family set as kelir-shim DefenseFlags (io_uring and keyrings blocked, AF_ALG/AF_RXRPC/NETLINK_XFRM denied, deny_abi_compat).; slop: driver filter diverges from the shim filter, the same syscall is trapped in one and passed in the other.
    pub(crate) fn seccomp_install_probe(&self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "proot pre-boot probe: seccomp_install_probe".into(),
        ))
    }

    // TODO (L3) [hypervisor_driver]: live behavioral probe confirming a trapped syscall waits killably instead of hanging the guest unkillably; slop: static checks report a live trap while the guest hangs unkillably.
    // TODO (L3) [hypervisor_driver]: inherit the identical BPF syscall and socket-family set as kelir-shim DefenseFlags (io_uring and keyrings blocked, AF_ALG/AF_RXRPC/NETLINK_XFRM denied, deny_abi_compat).; slop: driver filter diverges from the shim filter, the same syscall is trapped in one and passed in the other.
    pub(crate) fn wait_killable_probe(&self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "proot pre-boot probe: wait_killable_probe".into(),
        ))
    }
}

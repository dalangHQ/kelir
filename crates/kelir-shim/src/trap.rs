// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)

pub fn init_seccomp_unotify() -> Result<i32, ()> {
    // TODO: seccomp(SECCOMP_SET_MODE_FILTER) + SECCOMP_IOCTL_NOTIF_RECV fd (L3).
    Err(())
}

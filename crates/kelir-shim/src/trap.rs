// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
// see: cans/artifacts/diagrams/trap-flow.txt

#[allow(dead_code)]
pub fn init_seccomp_unotify() -> Result<i32, ()> {
    // see: cans/artifacts/diagrams/trap-flow.txt
    Err(())
}

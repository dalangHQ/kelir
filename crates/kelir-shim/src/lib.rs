// see: cans/11-protocol.md#Wire-framing
#[allow(dead_code)]
#[allow(unused_imports)]
mod codec;
// see: cans/11-protocol.md#Trap-fast-path
#[allow(dead_code)]
mod introspect;
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
pub mod pty;
// see: cans/11-protocol.md#Trap-fast-path
#[allow(dead_code)]
mod trap;
// see: cans/11-protocol.md#Trap-fast-path
pub use trap::ShimError;

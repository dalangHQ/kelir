// see: cans/11-protocol.md#Wire-framing
#[allow(dead_code)]
#[allow(unused_imports)]
mod codec_protocol;
// see: cans/11-protocol.md#Trap-fast-path
#[allow(dead_code)]
mod introspect_trap;
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
pub mod pty_terminal;
// see: cans/11-protocol.md#Trap-fast-path
#[allow(dead_code)]
mod trap_protocol;
// see: cans/11-protocol.md#Trap-fast-path
pub use trap_protocol::ShimError;

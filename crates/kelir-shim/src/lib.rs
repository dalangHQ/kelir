// TODO (L3) [pm_01_control_channel_0_lifecycle_priority]: wire the trap/codec/introspect skeleton into live execution; slop: shim skeleton unwired, crate-wide allow until then
#![allow(dead_code)]
// see: cans/11-protocol.md#Wire-framing
#[allow(unused_imports)]
mod codec_protocol;
// see: cans/11-protocol.md#Trap-fast-path
#[path = "introspect_trap.run.rs"]
mod introspect_trap;
// see: cans/11-protocol.md#Guest-paravirtual-micro-init-boundary-(kelir-shim)
#[path = "pty_terminal.run.rs"]
mod pty_terminal;
// see: cans/11-protocol.md#Trap-fast-path
#[path = "trap_protocol.run.rs"]
mod trap_protocol;
// see: cans/11-protocol.md#Trap-fast-path
#[path = "shim.flow.rs"]
pub mod shim;
pub use crate::shim::{ShimError, open_pty_slave};

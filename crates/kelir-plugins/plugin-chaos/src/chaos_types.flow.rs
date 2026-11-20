#![forbid(unsafe_code)]

// see: cans/plugins/chaos.md#Network-chaos
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FaultDirection {
    #[default]
    Outbound,
    Inbound,
    Both,
}

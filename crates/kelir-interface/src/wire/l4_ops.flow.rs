// see: cans/07-wire-plane.md#Wire-architecture
use crate::protocol_wire::StreamKind;
use crate::wire::l4::L4Stream;

// see: cans/07-wire-plane.md#Wire-architecture
impl L4Stream {
    pub fn new(guest_port: u16, host_target: String) -> Self {
        Self {
            kind: StreamKind::L4Forward,
            guest_port,
            host_target,
        }
    }
}

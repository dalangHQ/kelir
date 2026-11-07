// see: cans/plugins/db-proxy.md
use kelir_interface::plugin::PluginError;

// see: cans/plugins/db-proxy.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgFrameKind {
    Query,
    Parse,
    Other,
}

// see: cans/plugins/db-proxy.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgFrameDescriptor {
    pub kind: PgFrameKind,
    pub length: u32,
}

impl PgFrameDescriptor {
    pub fn decode(buffer: &[u8]) -> Result<Option<Self>, PluginError> {
        let _ = buffer;
        // TODO (L3): TCP reassembly per cans/07-wire-plane.md#Transparent-L4-stream-diversion, then classify 'Q'/'P' frames per cans/plugins/db-proxy.md.
        Ok(None)
    }
}

// see: cans/plugins/db-proxy.md#Plugin-db-proxy
// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgFrameKind {
    Query,
    Parse,
    Other,
}

// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgFrameDescriptor {
    pub kind: PgFrameKind,
    pub length: u32,
}

impl PgFrameDescriptor {
    // see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
    pub fn classify(tag: u8, length: u32) -> Self {
        let kind = match tag {
            b'Q' => PgFrameKind::Query,
            b'P' => PgFrameKind::Parse,
            _ => PgFrameKind::Other,
        };
        Self { kind, length }
    }
}

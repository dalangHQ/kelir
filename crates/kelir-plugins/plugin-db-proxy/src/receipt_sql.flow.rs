// see: cans/plugins/db-proxy.md#SQL-receipt-extension
use kelir_interface::telemetry::ExecutionReceipt;

// see: cans/plugins/db-proxy.md#SQL-receipt-extension
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetL7SqlExt {
    pub statements: u32,
    pub distinct_shapes: u32,
}

// see: cans/plugins/db-proxy.md#SQL-receipt-extension
pub const NET_L7_SQL_NAMESPACE: &str = "net:l7:sql";

// see: cans/plugins/db-proxy.md#SQL-receipt-extension
pub trait SqlReceiptExt {
    fn net_l7_sql(&self) -> Option<NetL7SqlExt>;
}

impl SqlReceiptExt for ExecutionReceipt {
    // see: cans/plugins/db-proxy.md#SQL-receipt-extension
    fn net_l7_sql(&self) -> Option<NetL7SqlExt> {
        let payload = self
            .ext_raw()
            .into_iter()
            .find(|(namespace, _)| namespace == NET_L7_SQL_NAMESPACE)?
            .1;
        let bytes: [u8; 8] = payload.as_slice().try_into().ok()?;
        Some(NetL7SqlExt {
            statements: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            distinct_shapes: u32::from_le_bytes(bytes[4..8].try_into().ok()?),
        })
    }
}

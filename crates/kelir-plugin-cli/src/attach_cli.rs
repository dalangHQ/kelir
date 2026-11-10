// see: cans/08-lifecycle.md#Detached-lifecycle
use kelir_catalog::catalog_path;
use kelir_catalog::schema_catalog::LeaseKind;
use kelir_interface::hex_codec::{HexDecodeError, decode_hex_fixed};

// see: cans/08-lifecycle.md#Detached-lifecycle
fn parse_session_id(arg: &str) -> Result<[u8; 16], Box<dyn std::error::Error>> {
    decode_hex_fixed::<16>(arg).map_err(|error| {
        let message = match error {
            HexDecodeError::InvalidDigit(byte) => {
                format!("attach: session id has non-hex byte {byte}")
            }
            _ => format!("attach: session id `{arg}` is not 32 hex chars"),
        };
        message.into()
    })
}

fn now_unix() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    i64::try_from(secs).unwrap_or(i64::MAX)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // TODO (L3): connect to relay.sock and stream the detached session after the catalog lease check per cans/08-lifecycle.md#Detached-lifecycle.
    let Some(id_arg) = args.first() else {
        return Err("attach requires a session id: kelir attach <session-id>".into());
    };
    let session_id = parse_session_id(id_arg)?;
    let db = kelir_catalog::CatalogDb::open(catalog_path()?)?;
    let Some(row) = db.lookup_session(&session_id)? else {
        return Err(format!("attach: no catalog row for session `{id_arg}`").into());
    };
    if row.lease_kind == LeaseKind::Lease
        && row
            .expires_at_unix
            .is_none_or(|expires| expires <= now_unix())
    {
        return Err(format!("attach: lease for session `{id_arg}` is expired").into());
    }
    let relay = kelir_core::Session::relay_socket_path(&session_id)?;
    println!(
        "kelir attach: session {id_arg} runner_pid={} relay={}",
        row.runner_pid,
        relay.display()
    );
    Err("TODO (L3): kelir attach relay.sock connect".into())
}

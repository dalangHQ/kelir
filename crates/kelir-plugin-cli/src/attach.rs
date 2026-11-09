// see: cans/08-lifecycle.md#Detached-lifecycle
use std::path::PathBuf;

use kelir_catalog::schema::LeaseKind;

fn hex_val(byte: u8) -> Result<u8, Box<dyn std::error::Error>> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("attach: session id has non-hex byte {byte}").into()),
    }
}

fn parse_session_id(arg: &str) -> Result<[u8; 16], Box<dyn std::error::Error>> {
    let bytes = arg.as_bytes();
    if bytes.len() != 32 {
        return Err(format!("attach: session id `{arg}` is not 32 hex chars").into());
    }
    let mut id = [0u8; 16];
    for (i, chunk) in bytes.chunks(2).enumerate() {
        id[i] = (hex_val(chunk[0])? << 4) | hex_val(chunk[1])?;
    }
    Ok(id)
}

fn catalog_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home =
        std::env::var("HOME").map_err(|_| "HOME is not set; cannot locate ~/.kelir/catalog.db")?;
    Ok(PathBuf::from(home).join(".kelir").join("catalog.db"))
}

fn now_unix() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    match i64::try_from(secs) {
        Ok(v) => v,
        Err(_) => i64::MAX,
    }
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

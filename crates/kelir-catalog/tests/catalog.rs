// see: cans/artifacts/test-plan/cases/storage_plane/sp_21_catalog_session_register_lookup_roundtrip.yaml#case_id
use std::path::PathBuf;

use kelir_catalog::CatalogDb;
use kelir_catalog::schema::{IncarnationId, LeaseKind, SessionRow};

fn temp_db(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kelir-et2-{name}.db"))
}

fn session_row(session_id: [u8; 16], runner_pid: i64) -> SessionRow {
    SessionRow {
        session_id,
        incarnation_id: IncarnationId([2u8; 16]),
        lease_kind: LeaseKind::Ephemeral,
        expires_at_unix: None,
        runner_pid,
        boot_id: "boot-1".to_string(),
        start_time_ns: 12345,
    }
}

fn open_clean(name: &str) -> Result<CatalogDb, String> {
    let path = temp_db(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    CatalogDb::open(&path).map_err(|e| e.to_string())
}

fn drop_db(name: &str) {
    let path = temp_db(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_21_catalog_session_register_lookup_roundtrip.yaml#case_id
#[test]
fn sp_21_catalog_session_register_lookup_roundtrip() -> Result<(), String> {
    let db = open_clean("sp21")?;
    let row = session_row([1u8; 16], 1001);
    db.register_session(&row).map_err(|e| e.to_string())?;
    let found = db
        .lookup_session(&row.session_id)
        .map_err(|e| e.to_string())?;
    match found {
        None => return Err("registered row missing".to_string()),
        Some(found) => {
            assert_eq!(found.session_id, row.session_id);
            assert_eq!(found.incarnation_id, row.incarnation_id);
            assert_eq!(found.lease_kind, row.lease_kind);
            assert_eq!(found.expires_at_unix, row.expires_at_unix);
            assert_eq!(found.runner_pid, row.runner_pid);
            assert_eq!(found.boot_id, row.boot_id);
            assert_eq!(found.start_time_ns, row.start_time_ns);
        }
    }
    let unknown = db.lookup_session(&[9u8; 16]).map_err(|e| e.to_string())?;
    match unknown {
        None => {}
        Some(_) => return Err("unknown id returned a row".to_string()),
    }
    drop_db("sp21");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_22_catalog_heartbeat_lease_cas_guards_handoff.yaml#case_id
#[test]
fn sp_22_catalog_heartbeat_lease_cas_guards_handoff() -> Result<(), String> {
    let db = open_clean("sp22")?;
    let row = session_row([3u8; 16], 1001);
    db.register_session(&row).map_err(|e| e.to_string())?;
    assert!(
        db.heartbeat_lease(&row.session_id, 1001, 30)
            .map_err(|e| e.to_string())?
    );
    assert!(
        !db.heartbeat_lease(&row.session_id, 1002, 30)
            .map_err(|e| e.to_string())?
    );
    assert!(
        !db.disk_lock_handoff(&row.session_id, 1002, 30)
            .map_err(|e| e.to_string())?
    );
    let found = db
        .lookup_session(&row.session_id)
        .map_err(|e| e.to_string())?;
    match found {
        None => return Err("registered row missing after handoff attempt".to_string()),
        Some(found) => {
            assert_eq!(found.runner_pid, 1001);
            assert_eq!(found.session_id, row.session_id);
        }
    }
    assert!(
        db.disk_lock_handoff(&row.session_id, 1001, 30)
            .map_err(|e| e.to_string())?
    );
    drop_db("sp22");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/lifecycle_checkpoint/lc_22_sqlite_wal_busy_timeout_recovery.yaml#case_id
#[test]
fn lc_22_sqlite_wal_busy_timeout_recovery() -> Result<(), String> {
    let db = open_clean("lc22")?;
    let journal_mode: String = db
        .pool
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    assert_eq!(journal_mode, "wal");
    let busy_timeout: i64 = db
        .pool
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    assert_eq!(busy_timeout, 5000);
    let path = temp_db("lc22");
    let first_writer = CatalogDb::open(&path).map_err(|e| e.to_string())?;
    let second_writer = CatalogDb::open(&path).map_err(|e| e.to_string())?;
    let first = std::thread::spawn(move || -> Result<(), String> {
        first_writer
            .register_session(&session_row([0xa1u8; 16], 2001))
            .map_err(|e| e.to_string())
    });
    let second = std::thread::spawn(move || -> Result<(), String> {
        second_writer
            .register_session(&session_row([0xa2u8; 16], 2002))
            .map_err(|e| e.to_string())
    });
    first
        .join()
        .map_err(|_| "first writer thread panicked".to_string())??;
    second
        .join()
        .map_err(|_| "second writer thread panicked".to_string())??;
    let first_found = db
        .lookup_session(&[0xa1u8; 16])
        .map_err(|e| e.to_string())?;
    let second_found = db
        .lookup_session(&[0xa2u8; 16])
        .map_err(|e| e.to_string())?;
    match (first_found, second_found) {
        (Some(first), Some(second)) => {
            assert_eq!(first.runner_pid, 2001);
            assert_eq!(second.runner_pid, 2002);
        }
        _ => return Err("concurrent writer row missing".to_string()),
    }
    drop_db("lc22");
    Ok(())
}

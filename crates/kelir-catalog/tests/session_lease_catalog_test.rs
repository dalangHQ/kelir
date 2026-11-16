// see: cans/artifacts/test-plan/cases/storage_plane/sp_21_catalog_session_register_lookup_roundtrip.yaml#case_id
use std::path::PathBuf;

use kelir_catalog::CatalogDb;
use kelir_catalog::schema_catalog::{
    CAS_LEASE_TTL_SECONDS, CatalogBackend, CatalogConfig, DEFAULT_TENANT_ID, IncarnationId,
    LeaseKind, POSTGRES_PARTITION_DDL, SessionRow, tenant_partition_ddl,
};

fn temp_db(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kelir-et2-{name}.db"))
}

fn session_row(tenant_id: &str, session_id: [u8; 16], runner_pid: i64) -> SessionRow {
    SessionRow {
        tenant_id: tenant_id.to_string(),
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
    let row = session_row("tenant-a", [1u8; 16], 1001);
    db.register_session(&row).map_err(|e| e.to_string())?;
    let found = db
        .lookup_session("tenant-a", &row.session_id)
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
    let unknown = db
        .lookup_session("tenant-a", &[9u8; 16])
        .map_err(|e| e.to_string())?;
    match unknown {
        None => {}
        Some(_) => return Err("unknown id returned a row".to_string()),
    }
    drop_db("sp21");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_21_catalog_session_register_lookup_roundtrip.yaml#case_id
#[test]
fn catalog_session_unregister_roundtrip() -> Result<(), String> {
    let db = open_clean("unreg")?;
    let row = session_row("tenant-a", [5u8; 16], 1001);
    db.register_session(&row).map_err(|e| e.to_string())?;
    assert!(
        db.unregister_session("tenant-a", &row.session_id)
            .map_err(|e| e.to_string())?
    );
    let found = db
        .lookup_session("tenant-a", &row.session_id)
        .map_err(|e| e.to_string())?;
    match found {
        None => {}
        Some(_) => return Err("unregistered row still present".to_string()),
    }
    assert!(
        !db.unregister_session("tenant-a", &row.session_id)
            .map_err(|e| e.to_string())?
    );
    drop_db("unreg");
    Ok(())
}

// see: cans/artifacts/test-plan/cases/storage_plane/sp_22_catalog_heartbeat_lease_cas_guards_handoff.yaml#case_id
#[test]
fn sp_22_catalog_heartbeat_lease_cas_guards_handoff() -> Result<(), String> {
    let db = open_clean("sp22")?;
    let row = session_row("tenant-a", [3u8; 16], 1001);
    db.register_session(&row).map_err(|e| e.to_string())?;
    assert!(
        db.heartbeat_lease("tenant-a", &row.session_id, 1001, 30)
            .map_err(|e| e.to_string())?
    );
    assert!(
        !db.heartbeat_lease("tenant-a", &row.session_id, 1002, 30)
            .map_err(|e| e.to_string())?
    );
    assert!(
        !db.disk_lock_handoff("tenant-a", &row.session_id, 1002, 30)
            .map_err(|e| e.to_string())?
    );
    let found = db
        .lookup_session("tenant-a", &row.session_id)
        .map_err(|e| e.to_string())?;
    match found {
        None => return Err("registered row missing after handoff attempt".to_string()),
        Some(found) => {
            assert_eq!(found.runner_pid, 1001);
            assert_eq!(found.session_id, row.session_id);
        }
    }
    assert!(
        db.disk_lock_handoff("tenant-a", &row.session_id, 1001, 30)
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
            .register_session(&session_row("tenant-a", [0xa1u8; 16], 2001))
            .map_err(|e| e.to_string())
    });
    let second = std::thread::spawn(move || -> Result<(), String> {
        second_writer
            .register_session(&session_row("tenant-a", [0xa2u8; 16], 2002))
            .map_err(|e| e.to_string())
    });
    first
        .join()
        .map_err(|_| "first writer thread panicked".to_string())??;
    second
        .join()
        .map_err(|_| "second writer thread panicked".to_string())??;
    let first_found = db
        .lookup_session("tenant-a", &[0xa1u8; 16])
        .map_err(|e| e.to_string())?;
    let second_found = db
        .lookup_session("tenant-a", &[0xa2u8; 16])
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

// see: cans/06-storage-catalog.md#Entity-model
#[test]
fn sp_23_catalog_tenant_partition_isolates_sessions() -> Result<(), String> {
    let db = open_clean("sp23")?;
    let shared_id = [7u8; 16];
    let row_a = session_row("tenant-a", shared_id, 1001);
    let row_b = session_row("tenant-b", shared_id, 2002);
    db.register_session(&row_a).map_err(|e| e.to_string())?;
    db.register_session(&row_b).map_err(|e| e.to_string())?;
    let found_a = db
        .lookup_session("tenant-a", &shared_id)
        .map_err(|e| e.to_string())?
        .ok_or("tenant-a row missing")?;
    let found_b = db
        .lookup_session("tenant-b", &shared_id)
        .map_err(|e| e.to_string())?
        .ok_or("tenant-b row missing")?;
    assert_eq!(found_a.runner_pid, 1001);
    assert_eq!(found_b.runner_pid, 2002);
    assert_eq!(found_a.tenant_id, "tenant-a");
    assert_eq!(found_b.tenant_id, "tenant-b");
    assert!(
        db.lookup_session("tenant-c", &shared_id)
            .map_err(|e| e.to_string())?
            .is_none(),
        "unregistered tenant must not see another tenant row"
    );
    assert!(
        db.register_session(&row_a).is_err(),
        "same (tenant_id, session_id) must violate the composite unique key"
    );
    assert!(
        db.unregister_session("tenant-a", &shared_id)
            .map_err(|e| e.to_string())?
    );
    assert!(
        db.lookup_session("tenant-b", &shared_id)
            .map_err(|e| e.to_string())?
            .is_some(),
        "unregistering tenant-a must not drop the tenant-b row"
    );
    drop_db("sp23");
    Ok(())
}

// see: cans/06-storage-catalog.md#Entity-model
#[test]
fn sp_24_catalog_cas_lease_ttl_30_seconds_and_uuidv7_incarnation() -> Result<(), String> {
    assert_eq!(CAS_LEASE_TTL_SECONDS, 30);
    let incarnation = IncarnationId::new_v7();
    assert!(incarnation.is_uuid_v7(), "minted incarnation is not UUIDv7");
    assert_ne!(IncarnationId::new_v7(), incarnation);
    let db = open_clean("sp24")?;
    let expires = db
        .register_cas_lease(&[0xabu8; 32], 1001)
        .map_err(|e| e.to_string())?;
    let stored: i64 = db
        .pool
        .query_row(
            "SELECT expires_at_unix FROM cas_leases WHERE blob_hash = ?1 AND holder_pid = ?2",
            rusqlite::params![&[0xabu8; 32][..], 1001],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    assert_eq!(
        stored, expires,
        "CAS lease expiry read back differs from the registered expiry"
    );
    assert_eq!(
        db.prune_cas_leases().map_err(|e| e.to_string())?,
        0,
        "unexpired CAS lease pruned early"
    );
    drop_db("sp24");
    Ok(())
}

// see: cans/06-storage-catalog.md#Entity-model
#[test]
fn sp_25_catalog_backends_partition_ddl_and_sqlite_fallback() -> Result<(), String> {
    assert!(
        POSTGRES_PARTITION_DDL.contains("PARTITION BY LIST (tenant_id)"),
        "postgres DDL carries no tenant partition"
    );
    assert_eq!(
        tenant_partition_ddl(CatalogBackend::Postgres),
        POSTGRES_PARTITION_DDL
    );
    assert!(
        tenant_partition_ddl(CatalogBackend::Sqlite).contains("sessions_tenant_session_uidx"),
        "sqlite DDL carries no composite tenant index"
    );
    let sqlite = CatalogConfig::sqlite(temp_db("sp25-sqlite").to_string_lossy().into_owned());
    assert_eq!(sqlite.backend, CatalogBackend::Sqlite);
    assert!(sqlite.schema_tenant_partitioning);
    let db = CatalogDb::open_with_config(&sqlite).map_err(|e| e.to_string())?;
    let journal_mode: String = db
        .pool
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    assert_eq!(journal_mode, "wal");
    let user_version: i64 = db
        .pool
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    assert_eq!(user_version, 2);
    drop_db("sp25-sqlite");
    let postgres = CatalogConfig::postgres(
        "postgres://kelir_admin@pg.internal.net:5432/kelir_catalog",
        "~/.kelir/catalog.db",
    );
    assert_eq!(postgres.backend, CatalogBackend::Postgres);
    match CatalogDb::open_with_config(&postgres) {
        Err(kelir_catalog::CatalogError::PostgresLiveBoot) => {}
        Err(other) => return Err(format!("postgres boot gave the wrong error: {other}")),
        Ok(_) => {
            return Err("postgres catalog booted without a live PostgreSQL endpoint".to_string());
        }
    }
    Ok(())
}

// see: cans/06-storage-catalog.md#Entity-model
#[test]
fn sp_26_catalog_v1_schema_migrates_to_tenant_v2() -> Result<(), String> {
    let path = temp_db("sp26");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    {
        let conn = rusqlite::Connection::open(&path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE sessions (
                session_id BLOB PRIMARY KEY,
                incarnation_id BLOB NOT NULL,
                lease_kind TEXT CHECK(lease_kind IN ('ephemeral', 'lease')) NOT NULL,
                expires_at_unix INTEGER,
                runner_pid INTEGER NOT NULL,
                boot_id TEXT NOT NULL,
                start_time_ns INTEGER NOT NULL
            );
            INSERT INTO sessions (session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns)
            VALUES (X'01010101010101010101010101010101', X'02020202020202020202020202020202', 'ephemeral', NULL, 1001, 'boot-1', 12345);",
        )
        .map_err(|e| e.to_string())?;
    }
    let db = CatalogDb::open(&path).map_err(|e| e.to_string())?;
    let found = db
        .lookup_session(DEFAULT_TENANT_ID, &[1u8; 16])
        .map_err(|e| e.to_string())?
        .ok_or("v1 row missing after migration")?;
    assert_eq!(found.runner_pid, 1001);
    assert_eq!(found.boot_id, "boot-1");
    assert_eq!(found.tenant_id, DEFAULT_TENANT_ID);
    drop_db("sp26");
    Ok(())
}

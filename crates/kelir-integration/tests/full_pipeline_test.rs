// see: cans/15-operations.md#Manifest-invariant

// TODO (L3): fill each stub with the live fast-boot/DAX surface per its case file.

#[test]
#[ignore]
fn test_sp_dax_available() -> Result<(), String> {
    Err("red: no DaxWindow type until L3; DaxWindowMode is a builder-only stub".to_string())
}

#[test]
#[ignore]
fn test_sp_dax_fallback() -> Result<(), String> {
    Err("red: no DaxWindow type until L3; FUSE fallback has no live window to fail".to_string())
}

#[test]
#[ignore]
fn test_sp_dax_teardown() -> Result<(), String> {
    Err(
        "red: no DaxWindow type until L3; Stage-2 unmap has no live window to tear down"
            .to_string(),
    )
}

#[test]
#[ignore]
fn test_hd_fast_boot_hit() -> Result<(), String> {
    Err("red: FastBootMode::Auto/Enabled driver boot path is a TODO (L3)".to_string())
}

#[test]
#[ignore]
fn test_hd_fast_boot_miss() -> Result<(), String> {
    Err(
        "red: FastBootMode driver boot path is a TODO (L3); cold fallback has no live path"
            .to_string(),
    )
}

#[test]
#[ignore]
fn test_hd_fast_boot_drift() -> Result<(), String> {
    Err("red: FastBootMode driver boot path is a TODO (L3); BLAKE3 key recomputation has no live path".to_string())
}

#[test]
#[ignore]
fn test_hd_fast_boot_backend_change() -> Result<(), String> {
    Err(
        "red: FastBootMode driver boot path is a TODO (L3); driver_id keying has no live path"
            .to_string(),
    )
}

#[test]
#[ignore]
fn test_lc_base_snapshot_capture() -> Result<(), String> {
    Err("red: base snapshot capture hook is L3; finish_boot has no capture yet".to_string())
}

#[test]
#[ignore]
fn test_lc_base_snapshot_lifecycle() -> Result<(), String> {
    Err("red: base snapshot capture and hydration are L3; engine-internal exclusion has no live path".to_string())
}

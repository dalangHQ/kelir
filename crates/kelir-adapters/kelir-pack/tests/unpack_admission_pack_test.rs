// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn run_pack(args: &[&str]) -> Result<(std::process::ExitStatus, String), String> {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_kelir-pack"))
        .args(args)
        .output()
        .map_err(|e| format!("spawn kelir-pack: {e}"))?;
    Ok((
        output.status,
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn fixture_dir(case: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("kelir-pack-red-{}-{case}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("blobs").join("sha256"))
        .map_err(|e| format!("create fixture dir: {e}"))?;
    Ok(dir)
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn ustar_header(name: &str, size: u64) -> [u8; 512] {
    let mut header = [0u8; 512];
    let name_len = name.len().min(99);
    header[..name_len].copy_from_slice(&name.as_bytes()[..name_len]);
    header[100..107].copy_from_slice(b"0000644");
    header[108..115].copy_from_slice(b"0000000");
    header[116..123].copy_from_slice(b"0000000");
    let size_field = format!("{size:011o}\0");
    header[124..136].copy_from_slice(size_field.as_bytes());
    header[136..147].copy_from_slice(b"00000000000");
    header[148..156].copy_from_slice(b"        ");
    header[156] = b'0';
    header[257..262].copy_from_slice(b"ustar");
    header[263..265].copy_from_slice(b"00");
    let checksum: u32 = header.iter().map(|b| u32::from(*b)).sum();
    let chksum = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(chksum.as_bytes());
    header
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn tar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar = Vec::new();
    for (name, data) in entries {
        tar.extend_from_slice(&ustar_header(name, data.len() as u64));
        tar.extend_from_slice(data);
        let pad = (512 - data.len() % 512) % 512;
        tar.extend(std::iter::repeat_n(0u8, pad));
    }
    tar.extend_from_slice(&[0u8; 1024]);
    tar
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn write_blob(dir: &Path, bytes: &[u8]) -> Result<(String, u64), String> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hex = format!("{:x}", hasher.finalize());
    std::fs::write(dir.join("blobs").join("sha256").join(&hex), bytes)
        .map_err(|e| format!("write layer blob: {e}"))?;
    Ok((format!("sha256:{hex}"), bytes.len() as u64))
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
fn write_manifest(dir: &Path, layers: &[(String, u64)]) -> Result<PathBuf, String> {
    let body = layers
        .iter()
        .map(|(digest, size)| {
            format!(
                "{{\"mediaType\":\"application/vnd.oci.image.layer.v1.tar\",\"digest\":\"{digest}\",\"size\":{size}}}"
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let path = dir.join("manifest.json");
    std::fs::write(
        &path,
        format!("{{\"schemaVersion\":2,\"layers\":[{body}]}}"),
    )
    .map_err(|e| format!("write manifest: {e}"))?;
    Ok(path)
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_02_expansion_ratio_15_to_1_abort.yaml#case_id
fn names_oversize_abort(stderr: &str) -> bool {
    stderr.contains("LayerOversize")
        || stderr.contains("layer oversize")
        || stderr.contains("E_LAYER_OVERSIZE")
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_01_radix_layer_unpack_whiteout_merge.yaml#case_id
#[test]
#[ignore]
fn ad_01_radix_layer_unpack_whiteout_merge() -> Result<(), String> {
    let dir = fixture_dir("ad_01")?;
    let base = tar_bytes(&[("secret", b"top-secret\n"), ("etc/old.conf", b"stale\n")]);
    let whiteouts = tar_bytes(&[(".wh.secret", b""), ("etc/.wh..wh..opq", b"")]);
    let base_meta = write_blob(&dir, &base)?;
    let whiteout_meta = write_blob(&dir, &whiteouts)?;
    let manifest = write_manifest(&dir, &[base_meta, whiteout_meta])?;
    let output = dir.join("rootfs.erofs");
    let (status, stderr) = run_pack(&[
        manifest.to_str().ok_or("manifest path is not utf-8")?,
        output.to_str().ok_or("output path is not utf-8")?,
    ])?;
    let verdict = if !status.success() {
        Err(format!(
            "whiteout-merging build aborted with `{stderr}` instead of applying .wh. entries in the radix tree"
        ))
    } else if !output.is_file() {
        Err("build exited 0 but emitted no rootfs.erofs image".to_string())
    } else {
        Ok(())
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_02_expansion_ratio_15_to_1_abort.yaml#case_id
#[test]
#[ignore]
fn ad_02_expansion_ratio_15_to_1_abort() -> Result<(), String> {
    let dir = fixture_dir("ad_02")?;
    let bomb = vec![b'A'; 16 * 1024 * 1024];
    let layer = tar_bytes(&[("bomb", &bomb)]);
    let (digest, _) = write_blob(&dir, &layer)?;
    let manifest = write_manifest(&dir, &[(digest, 1024 * 1024)])?;
    let output = dir.join("rootfs.erofs");
    let (status, stderr) = run_pack(&[
        manifest.to_str().ok_or("manifest path is not utf-8")?,
        output.to_str().ok_or("output path is not utf-8")?,
    ])?;
    let verdict = if status.success() {
        Err("a 1 MiB declared layer expanding to 16 MiB compiled instead of aborting".to_string())
    } else if !names_oversize_abort(&stderr) {
        Err(format!(
            "16:1 expansion aborted with `{stderr}` instead of the LayerOversize / E_LAYER_OVERSIZE admission abort"
        ))
    } else if output.exists() {
        Err("oversized layer aborted but still emitted an EROFS image".to_string())
    } else {
        Ok(())
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_03_single_layer_8gib_ceiling_enforcement.yaml#case_id
#[test]
#[ignore]
fn ad_03_single_layer_8gib_ceiling_enforcement() -> Result<(), String> {
    let dir = fixture_dir("ad_03")?;
    let (digest, _) = write_blob(&dir, b"kelir-pack red fixture\n")?;
    let manifest = write_manifest(&dir, &[(digest, 8 * 1024 * 1024 * 1024 + 1)])?;
    let output = dir.join("rootfs.erofs");
    let (status, stderr) = run_pack(&[
        manifest.to_str().ok_or("manifest path is not utf-8")?,
        output.to_str().ok_or("output path is not utf-8")?,
    ])?;
    let verdict = if status.success() {
        Err("a declared 8 GiB + 1 byte layer compiled instead of aborting".to_string())
    } else if !names_oversize_abort(&stderr) {
        Err(format!(
            "8 GiB + 1 layer aborted with `{stderr}` instead of the LayerOversize / E_LAYER_OVERSIZE ceiling abort"
        ))
    } else if output.exists() {
        Err("oversized layer aborted but still emitted an EROFS image".to_string())
    } else {
        Ok(())
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_04_layer_entry_500k_inode_cap.yaml#case_id
#[test]
#[ignore]
fn ad_04_layer_entry_500k_inode_cap() -> Result<(), String> {
    let dir = fixture_dir("ad_04")?;
    let blob_path = dir.join("blobs").join("sha256").join("pending");
    let mut blob = std::fs::File::create(&blob_path).map_err(|e| format!("create blob: {e}"))?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;
    for index in 0..500_001u32 {
        let name = format!("f{index:06}");
        let header = ustar_header(&name, 0);
        blob.write_all(&header)
            .map_err(|e| format!("write tar header {index}: {e}"))?;
        hasher.update(header);
        size += header.len() as u64;
    }
    let end = [0u8; 1024];
    blob.write_all(&end)
        .map_err(|e| format!("write tar end: {e}"))?;
    hasher.update(end);
    size += end.len() as u64;
    drop(blob);
    let hex = format!("{:x}", hasher.finalize());
    std::fs::rename(&blob_path, dir.join("blobs").join("sha256").join(&hex))
        .map_err(|e| format!("rename blob: {e}"))?;
    let manifest = write_manifest(&dir, &[(format!("sha256:{hex}"), size)])?;
    let output = dir.join("rootfs.erofs");
    let (status, stderr) = run_pack(&[
        manifest.to_str().ok_or("manifest path is not utf-8")?,
        output.to_str().ok_or("output path is not utf-8")?,
    ])?;
    let verdict = if status.success() {
        Err("a layer carrying 500001 entries compiled instead of aborting".to_string())
    } else if !names_oversize_abort(&stderr) {
        Err(format!(
            "500001-entry layer aborted with `{stderr}` instead of the LayerOversize / E_LAYER_OVERSIZE inode-cap abort"
        ))
    } else if output.exists() {
        Err("oversized layer aborted but still emitted an EROFS image".to_string())
    } else {
        Ok(())
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_05_single_pass_erofs_blake3_digest.yaml#case_id
fn cas_blobs(dir: &Path) -> Result<Vec<String>, String> {
    let blobs = dir.join("cas").join("blobs");
    if !blobs.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let entries = std::fs::read_dir(&blobs).map_err(|e| format!("read cas/blobs: {e}"))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("blake3_") {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

// see: cans/artifacts/test-plan/cases/adapters_ingestion/ad_05_single_pass_erofs_blake3_digest.yaml#case_id
#[test]
#[ignore]
fn ad_05_single_pass_erofs_blake3_digest() -> Result<(), String> {
    let dir = fixture_dir("ad_05")?;
    let layer = tar_bytes(&[("etc/motd", b"kelir-pack digest fixture\n")]);
    let meta = write_blob(&dir, &layer)?;
    let manifest = write_manifest(&dir, &[meta])?;
    let manifest_arg = manifest
        .to_str()
        .ok_or("manifest path is not utf-8")?
        .to_string();
    let mut runs: Vec<Vec<String>> = Vec::new();
    for run in 0..2 {
        let output = dir.join(format!("rootfs-{run}.erofs"));
        let output_arg = output
            .to_str()
            .ok_or("output path is not utf-8")?
            .to_string();
        let (status, stderr) = run_pack(&[&manifest_arg, &output_arg])?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!(
                "single-pass build {run} aborted with `{stderr}`; no canonical BLAKE3 digest was computed"
            ));
        }
        if !output.is_file() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!("build {run} exited 0 but emitted no EROFS image"));
        }
        let blobs = cas_blobs(&dir)?;
        if blobs.is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!(
                "build {run} emitted no cas/blobs/blake3_<hash> blob for the boot path"
            ));
        }
        runs.push(blobs);
    }
    let verdict = if runs[0] != runs[1] {
        Err(format!(
            "two builds of the same layer emitted different CAS blobs: {:?} vs {:?}",
            runs[0], runs[1]
        ))
    } else {
        Ok(())
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

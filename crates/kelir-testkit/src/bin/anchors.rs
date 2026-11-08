// see: cans/15-operations.md#Coverage-gates
use kelir_testkit::spec_anchors;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Failed to locate workspace root")?
        .to_path_buf();

    let index = spec_anchors::build_index(&repo_root)?;
    let anchor_count: usize = index.files.values().map(Vec::len).sum();
    let rendered = spec_anchors::render_index(&index)?;
    let out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("anchors.json");
    fs::write(&out_path, rendered)?;
    println!(
        "Anchors index updated: {} files, {} anchors -> {}",
        index.files.len(),
        anchor_count,
        out_path.display()
    );
    Ok(())
}

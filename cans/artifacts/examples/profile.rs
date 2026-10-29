//! examples/profile.rs
//!
//! Focused example: declarative profile ingestion via Sandbox facade.
//! One plugin (profile), one fixture (kelir.toml), one verb per box op.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["profile"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

use kelir::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TOML declares plugins, limits, mounts. Profile flattens into
    // the same builder chain the other examples use manually.
    // Fixture: ./fixtures/kelir.toml
    let mut box_ = Sandbox::from_config("./fixtures/kelir.toml")
        .await?
        .boot()
        .await?;

    let r = box_.exec("git").args(["status"]).await?;
    eprintln!("[kelir] git status -> exit={}", r.exit_code);

    // Round-trip: emit the active topology back to disk.
    box_.export_spec("./fixtures/kelir.exported.toml").await?;

    box_.terminate().await?;
    Ok(())
}
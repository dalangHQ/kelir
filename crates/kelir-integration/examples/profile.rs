// see: cans/plugins/profile.md

use kelir::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut box_ = Sandbox::from_config("./fixtures/kelir.toml")
        .await?
        .boot()
        .await?;

    let r = box_.exec("git").args(["status"]).await?;
    eprintln!("[kelir] git status -> exit={}", r.exit_code);

    box_.export_spec("./fixtures/kelir.exported.toml").await?;

    box_.terminate().await?;
    Ok(())
}

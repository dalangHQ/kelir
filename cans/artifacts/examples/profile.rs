// see: cans/plugins/profile.md#Plugin-profile
// see: cans/plugins/profile.md#Declarative-profile-execution

use kelir::Sandbox;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut box_ = Sandbox::from_config("./fixtures/kelir.toml")
        .await?
        .boot()
        .await?;

    let r = box_.exec("git", &["status"]).await?;
    eprintln!("[kelir] git status -> exit={}", r.exit_code);

    box_.export_spec("./fixtures/kelir.exported.toml").await?;

    box_.terminate().await?;
    Ok(())
}

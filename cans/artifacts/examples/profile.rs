// see: cans/plugins/profile.md#Plugin-profile
// see: cans/plugins/profile.md#Declarative-profile-execution
// see: cans/02-plugin-model.md#Unified-builder

use kelir::KelirBuilder;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = KelirBuilder::from_config("./fixtures/kelir.toml")?
        .seal()?
        .boot()
        .await?;

    let r = session.spawn("git", &["status"], None).await?;
    eprintln!("[kelir] git status -> exit={}", r.exit_code);

    session.export_spec("./fixtures/kelir.exported.toml").await?;

    session.stop().await?;
    Ok(())
}

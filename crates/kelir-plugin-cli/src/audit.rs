// see: cans/15-operations.md#Per-plugin-CI
use std::fs;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let Some(name) = args.first() else {
        return Err("check -p requires a plugin name".into());
    };
    run_plugin(name)
}

pub fn run_plugin(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = format!("crates/kelir-plugins/{name}/Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|_| format!("check -p: no such plugin `{name}` (missing {manifest})"))?;
    for line in text.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("name = ") {
            let found = value.trim().trim_matches('"');
            if found != name {
                return Err(format!(
                    "check -p: package name mismatch for `{name}`: manifest says `{found}`"
                )
                .into());
            }
            return Ok(());
        }
    }
    Err(format!("check -p: no package name in {manifest}").into())
}

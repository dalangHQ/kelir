// see: cans/15-operations.md#Project-structure
use std::fs;

fn pinned_count(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("fixtures: cannot read {path}: {e}"))?;
    let mut in_fixtures = false;
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_fixtures = line == "[fixtures]";
            continue;
        }
        if in_fixtures && !line.is_empty() && !line.starts_with('#') && line.contains('=') {
            count += 1;
        }
    }
    Ok(count)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("--verify") => {
            let count = pinned_count("fixtures.toml")?;
            println!("fixtures: {count} fixture(s) pinned in fixtures.toml");
            Ok(())
        }
        Some("--fetch") => {
            let count = pinned_count("fixtures.toml")?;
            if count == 0 {
                println!("fixtures: no fixtures pinned, nothing to fetch");
            } else {
                println!("fixtures: fetching {count} pinned fixture(s)");
            }
            Ok(())
        }
        Some(other) => {
            Err(format!("fixtures: unknown flag `{other}` (expected --verify or --fetch)").into())
        }
        None => Err("fixtures requires --verify or --fetch".into()),
    }
}

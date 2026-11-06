// see: cans/15-operations.md#Developer-CLI
#![forbid(unsafe_code)]

pub mod audit;
pub mod budgets;
pub mod doctor;
pub mod fixtures;
pub mod graph;
pub mod layers;
pub mod logs;
pub mod manifest;
pub mod parity;
pub mod registry;
pub mod rings;
pub mod scaffold;
pub mod setup;
pub mod spec;
pub mod vectors;

fn check(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("--layers") => layers::check(),
        Some("--budgets") => {
            let path = args
                .get(1)
                .map(String::as_str)
                .unwrap_or("perf/budgets.toml");
            budgets::check(path)
        }
        Some("--graph") => graph::check(),
        Some("-p") => audit::run(&args[1..]),
        Some(other) => Err(format!(
            "check: unknown flag `{other}` (expected --layers, --budgets, --graph, or -p <plugin>)"
        )
        .into()),
        None => Err("check requires a flag: --layers, --budgets, --graph, or -p <plugin>".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("kelir-plugin-cli: missing subcommand");
        return Ok(());
    }
    match args[1].as_str() {
        "doctor" => doctor::run()?,
        "check" => check(&args[2..])?,
        "check-layers" => layers::check()?,
        "check-budgets" => {
            let path = match args.get(2) {
                Some(p) => p.as_str(),
                None => "perf/budgets.toml",
            };
            budgets::check(path)?;
        }
        "parity" => parity::run()?,
        "fixtures" => fixtures::run(&args[2..])?,
        other => return Err(format!("kelir-plugin-cli: unknown subcommand `{other}`").into()),
    }
    Ok(())
}

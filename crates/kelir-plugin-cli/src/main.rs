// see: cans/15-operations.md#Developer-CLI
#![forbid(unsafe_code)]

pub mod attach_cli;
pub mod audit_cli;
pub mod budgets_cli;
pub mod doctor_cli;
pub mod fixtures_cli;
pub mod graph_cli;
pub mod layers_cli;
pub mod logs_cli;
pub mod manifest_cli;
pub mod parity_cli;
pub mod registry_cli;
pub mod rings_cli;
pub mod scaffold_cli;
pub mod setup_cli;
pub mod spec_cli;
pub mod vectors_cli;

fn check_layers() -> Result<(), Box<dyn std::error::Error>> {
    layers_cli::check()
}

fn check_budgets(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args
        .first()
        .map(String::as_str)
        .unwrap_or("perf/budgets.toml");
    budgets_cli::check(path)
}

fn check(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("--layers") => check_layers(),
        Some("--budgets") => check_budgets(&args[1..]),
        Some("--graph") => graph_cli::check(),
        Some("-p") => audit_cli::run(&args[1..]),
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
        "doctor" => doctor_cli::run(&args[2..])?,
        "attach" => attach_cli::run(&args[2..])?,
        "check" => check(&args[2..])?,
        "check-layers" => check_layers()?,
        "check-budgets" => check_budgets(&args[2..])?,
        "parity" => parity_cli::run(&args[2..])?,
        "fixtures" => fixtures_cli::run(&args[2..])?,
        // TODO (L3): logs, manifest, registry, rings, and scaffold dispatch into skeleton subcommands that succeed silently.
        "logs" => logs_cli::run()?,
        "manifest" => manifest_cli::run()?,
        "registry" => registry_cli::run()?,
        "rings" => rings_cli::run()?,
        "scaffold" => scaffold_cli::run()?,
        "setup" => setup_cli::run(&args[2..])?,
        "spec" => spec_cli::run(&args[2..])?,
        "vectors" => vectors_cli::run(&args[2..])?,
        other => return Err(format!("kelir-plugin-cli: unknown subcommand `{other}`").into()),
    }
    Ok(())
}

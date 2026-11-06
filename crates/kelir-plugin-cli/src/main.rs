// see: cans/15-operations.md#Developer-CLI
#![forbid(unsafe_code)]

pub mod audit;
pub mod budgets;
pub mod doctor;
pub mod fixtures;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("kelir-plugin-cli: missing subcommand");
        return Ok(());
    }
    match args[1].as_str() {
        "doctor" => doctor::run()?,
        "check" => audit::run(&args[2..])?,
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
        _ => {}
    }
    Ok(())
}

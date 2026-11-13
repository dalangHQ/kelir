// see: cans/15-operations.md#Developer-CLI
use crate::cli_error::CliError;
use crate::{
    attach_cli, attest_cli, audit_cli, budgets_cli, bump_core_cli, doctor_cli, fixtures_cli,
    graph_cli, layers_cli, logs_cli, manifest_cli, parity_cli, registry_cli, rings_cli,
    scaffold_cli, setup_cli, spec_cli, vectors_cli,
};

fn check_layers() -> Result<(), CliError> {
    layers_cli::check()
}

fn check_budgets(args: &[String]) -> Result<(), CliError> {
    let path = match args.first() {
        None => crate::workspace_root()?.join("perf/budgets.toml"),
        Some(path) => std::path::PathBuf::from(path),
    };
    budgets_cli::check(&path)
}

fn expect_no_args(args: &[String], flag: &str) -> Result<(), CliError> {
    if let Some(arg) = args.first() {
        return Err(CliError::usage(format!(
            "check: unknown argument `{arg}` after {flag} (expected --layers, --budgets, --graph, or -p <plugin>)"
        )));
    }
    Ok(())
}

fn check(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        Some("--layers") => {
            expect_no_args(&args[1..], "--layers")?;
            check_layers()
        }
        Some("--budgets") => {
            let rest = &args[1..];
            if rest.len() > 1 {
                return Err(CliError::usage(
                    "check: --budgets takes at most one budget file path",
                ));
            }
            for arg in rest {
                if arg.starts_with('-') {
                    return Err(CliError::usage(format!(
                        "check: unknown flag `{arg}` (expected --layers, --budgets, --graph, or -p <plugin>)"
                    )));
                }
            }
            check_budgets(rest)
        }
        Some("--graph") => {
            expect_no_args(&args[1..], "--graph")?;
            graph_cli::check()
        }
        Some("-p") => {
            if args.len() != 2 {
                return Err(CliError::usage(
                    "check: -p requires exactly one plugin name and no further flags",
                ));
            }
            audit_cli::run(&args[1..])
        }
        Some(other) => Err(CliError::usage(format!(
            "check: unknown flag `{other}` (expected --layers, --budgets, --graph, or -p <plugin>)"
        ))),
        None => Err(CliError::usage(
            "check requires a flag: --layers, --budgets, --graph, or -p <plugin>",
        )),
    }
}

pub fn dispatch(args: &[String]) -> Result<(), CliError> {
    if args.len() < 2 {
        return Err(CliError::usage(
            "kelir-plugin-cli: missing subcommand (expected doctor|attach|attest|bump-core|check|parity|fixtures|logs|manifest|registry|rings|new|setup|spec|vectors)",
        ));
    }
    match args[1].as_str() {
        "doctor" => doctor_cli::run(&args[2..])?,
        "attach" => attach_cli::run(&args[2..])?,
        "attest" => attest_cli::run(&args[2..])?,
        "bump-core" => bump_core_cli::run(&args[2..])?,
        "check" => check(&args[2..])?,
        "parity" => parity_cli::run(&args[2..])?,
        "fixtures" => fixtures_cli::run(&args[2..])?,
        "logs" => logs_cli::run()?,
        "manifest" => manifest_cli::run()?,
        "registry" => registry_cli::run()?,
        "rings" => rings_cli::run()?,
        "new" | "scaffold" => scaffold_cli::run()?,
        "setup" => setup_cli::run(&args[2..])?,
        "spec" => spec_cli::run(&args[2..])?,
        "vectors" => vectors_cli::run(&args[2..])?,
        other => {
            return Err(CliError::usage(format!(
                "kelir-plugin-cli: unknown subcommand `{other}`"
            )));
        }
    }
    Ok(())
}

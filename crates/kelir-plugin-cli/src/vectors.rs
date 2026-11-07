// see: cans/15-operations.md#Multi-language-parity-gate
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let out = args.first().map(String::as_str).unwrap_or("vectors.json");
    let suite = kelir_plugin_testkit::vectors::VectorSuite::fixtures()?;
    let count = suite.vectors.len();
    let json = suite.to_canonical_json()?;
    std::fs::write(out, json)?;
    println!("vectors: regenerated {out} ({count} vector(s) from testkit fixtures)");
    Ok(())
}

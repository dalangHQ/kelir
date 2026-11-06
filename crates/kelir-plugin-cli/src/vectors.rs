// see: cans/15-operations.md#Multi-language-parity-gate
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let out = args.first().map(String::as_str).unwrap_or("vectors.json");
    // TODO (L3): regenerate vectors.json from fixtures via the testkit vector serializer.
    Err(format!(
        "vectors: regenerating {out} from fixtures requires the testkit vector serializer (L3)"
    )
    .into())
}

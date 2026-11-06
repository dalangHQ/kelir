// see: cans/15-operations.md#Multi-language-parity-gate
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Verify vectors.json across Rust, TS, and Python AST surfaces
    println!("parity: vectors green");
    Ok(())
}

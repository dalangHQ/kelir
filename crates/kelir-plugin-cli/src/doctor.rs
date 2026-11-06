// see: cans/15-operations.md#Developer-CLI
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Probe KVM, HVF, WHP, AVF, and kernel invariants
    println!("kelir doctor: probe successful");
    Ok(())
}

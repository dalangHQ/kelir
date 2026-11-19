// see: cans/02-plugin-model.md#Chain-decisions
use kelir::{KelirBuilder, VmConfig};

fn main() {
    let builder = KelirBuilder::new(VmConfig::default());
    let _session = builder.boot();
}

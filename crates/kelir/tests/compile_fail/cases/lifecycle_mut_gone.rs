// see: cans/02-plugin-model.md#Unified-builder
use kelir::{KelirBuilder, VmConfig};

fn main() {
    let builder = KelirBuilder::new(VmConfig::default());
    builder.lifecycle_mut();
}

// see: cans/02-plugin-model.md#Unified-builder
use kelir::{Host, KelirBuilder, VmConfig};

fn main() {
    let builder = KelirBuilder::<Host, kelir::Unsealed, (), kelir::HNil, kelir::HNil, kelir::NoParams>::new(VmConfig::default());
    builder.lifecycle_mut();
}

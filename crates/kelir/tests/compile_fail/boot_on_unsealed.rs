// see: cans/02-plugin-model.md#Chain-decisions
use kelir::{Host, KelirBuilder, Unsealed, VmConfig};

fn main() {
    let builder = KelirBuilder::<Host, Unsealed, (), kelir::HNil, kelir::HNil, kelir::NoParams>::new(VmConfig::default());
    let _session = builder.boot();
}

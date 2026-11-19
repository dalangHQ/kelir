// see: cans/02-plugin-model.md#Nested-scopes
use kelir::{Host, KelirBuilder, Verdict, VmConfig};

fn main() {
    let _builder = KelirBuilder::new(VmConfig::default())
        .fs(|f| f.on_trap(|_event, _ctx| Ok(Verdict::Pass)).end());
}

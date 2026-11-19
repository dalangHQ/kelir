// see: cans/02-plugin-model.md#Unified-builder
use kelir::{
    planes::storage_plane::StoragePlaneConfig, Host, KelirBuilder, Plugin, Unsealed, VmConfig,
};

fn main() {
    let plugin = KelirBuilder::<Plugin, Unsealed, (), kelir::HNil, kelir::HNil, kelir::NoParams>::named("p");
    plugin.storage(StoragePlaneConfig::default());
}

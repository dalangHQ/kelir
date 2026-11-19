// see: cans/02-plugin-model.md#Unified-builder
use kelir::{planes::storage_plane::StoragePlaneConfig, KelirBuilder};

fn main() {
    let plugin = KelirBuilder::named("p");
    plugin.storage(StoragePlaneConfig::default());
}

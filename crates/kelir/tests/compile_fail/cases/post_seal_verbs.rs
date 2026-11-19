// see: cans/02-plugin-model.md#Unified-builder
use kelir::plugin_model::PluginError;
use kelir::{Host, HNil, KelirBuilder, NoParams, Sealed, Verdict, VmConfig};

fn sealed_builder() -> Result<KelirBuilder<Host, Sealed, (), HNil, HNil, NoParams>, PluginError> {
    KelirBuilder::new(VmConfig::default())
        .trap("git", |_event, _ctx| Ok(Verdict::Pass))
        .seal()
}

fn main() {
    let sealed = sealed_builder().unwrap();
    sealed.trap("bash", |_event, _ctx| Ok(Verdict::Pass));
    sealed.fs(|f| f.on_fs_read(|_path, data| Ok(data.to_vec())).end());
    sealed.detached(true);
}

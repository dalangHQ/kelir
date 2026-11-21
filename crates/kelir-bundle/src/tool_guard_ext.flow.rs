use kelir::hlist_sdk::HList;
use kelir::{Host, KelirBuilder, Unsealed};
use plugin_tool_guard::{ToolGuardPlugin, ToolSpec};

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
pub trait ToolGuardExt: Sized {
    fn tool(self, spec: ToolSpec) -> Self;
}

// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
impl<Plugins: HList, Params: 'static, Ctx, Deps: HList> ToolGuardExt
    for KelirBuilder<Host, Unsealed, Ctx, Deps, Plugins, Params>
{
    fn tool(self, spec: ToolSpec) -> Self {
        let plugin = ToolGuardPlugin::new().tool(spec);
        self.mount_instance(plugin)
    }
}

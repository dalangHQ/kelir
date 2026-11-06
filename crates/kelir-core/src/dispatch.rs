// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
pub struct PipelineEngine {
    trap_interceptors: Vec<Box<dyn kelir_interface::plugin::TrapInterposer + Send + Sync>>,
    fs_interceptors: Vec<Box<dyn kelir_interface::plugin::FsInterposer + Send + Sync>>,
    net_interceptors: Vec<Box<dyn kelir_interface::plugin::NetInterposer + Send + Sync>>,
    console_interceptors: Vec<Box<dyn kelir_interface::plugin::ConsoleInterposer + Send + Sync>>,
    host_interceptors: Vec<Box<dyn kelir_interface::plugin::HostInterposer + Send + Sync>>,
    sealed: bool,
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineEngine {
    pub fn new() -> Self {
        Self {
            trap_interceptors: Vec::new(),
            fs_interceptors: Vec::new(),
            net_interceptors: Vec::new(),
            console_interceptors: Vec::new(),
            host_interceptors: Vec::new(),
            sealed: false,
        }
    }

    pub fn seal(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        if self.sealed {
            return Err(kelir_interface::errors::PluginError::Halt(
                "pipeline already sealed".into(),
            ));
        }
        // TODO: freeze all five ring chains; console/host chains have no dispatch method yet.
        let _ = (&self.console_interceptors, &self.host_interceptors);
        self.sealed = true;
        Ok(())
    }

    pub fn dispatch_trap(
        &self,
        event: &kelir_interface::protocol::TrapEvent,
        ctx: &kelir_interface::plugin::HandlerCtx,
    ) -> Result<kelir_interface::protocol::Verdict, kelir_interface::errors::PluginError> {
        // TODO: await on_trap per trap_interceptor once dispatch is async; fail-closed on unwind.
        let _ = (event, ctx, &self.trap_interceptors);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            kelir_interface::protocol::Verdict::Pass
        }));
        match caught {
            Ok(verdict) => Ok(verdict),
            Err(_) => Ok(kelir_interface::protocol::Verdict::Drop {
                errno: Some("EFAULT".into()),
                reason: "Plugin panic caught".into(),
            }),
        }
    }

    pub fn dispatch_fs_write(
        &self,
        path: &str,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError> {
        // TODO: run fs_interceptors on_fs_write in order; offset maps to the write position.
        let _ = (&self.fs_interceptors, path, offset, buf);
        Ok(kelir_interface::protocol::FilterAction::Forward)
    }

    pub fn dispatch_net_outbound(
        &self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::errors::PluginError> {
        // TODO: run net_interceptors on_net_outbound in order.
        let _ = (&self.net_interceptors, frame);
        Ok(kelir_interface::protocol::FilterAction::Forward)
    }
}

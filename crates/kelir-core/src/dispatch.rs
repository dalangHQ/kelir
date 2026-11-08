// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
use std::sync::Arc;

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    ConsoleInterposer, FsInterposer, HandlerCtx, HostInterposer, MountScope, NetInterposer,
    PipelineBinder, StreamInterposer, TrapInterposer,
};
use kelir_interface::protocol::{FilterAction, ProcessExit, StreamKind, TrapEvent, Verdict};
use kelir_interface::service_registry::ServiceRegistry;
use kelir_interface::vsock::{VsockServiceHandler, VsockServiceSpec};

use crate::trap_gate::{GateDecision, TrapGateEvaluator};

pub struct PipelineEngine {
    trap_interceptors: Vec<Arc<dyn TrapInterposer>>,
    trap_scopes: Vec<MountScope>,
    gate: TrapGateEvaluator,
    fs_interceptors: Vec<Arc<dyn FsInterposer>>,
    net_interceptors: Vec<Arc<dyn NetInterposer>>,
    console_interceptors: Vec<Arc<dyn ConsoleInterposer>>,
    host_interceptors: Vec<Arc<dyn HostInterposer>>,
    stream_interposers: Vec<(StreamKind, Arc<dyn StreamInterposer>)>,
    vsock_services: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>)>,
    services: ServiceRegistry,
    sealed: bool,
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineBinder for PipelineEngine {
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>) {
        self.fs_interceptors.push(stage);
    }
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>) {
        self.net_interceptors.push(stage);
    }
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>) {
        self.trap_interceptors.push(stage);
        self.trap_scopes.push(MountScope::default());
    }
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>) {
        self.console_interceptors.push(stage);
    }
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>) {
        self.host_interceptors.push(stage);
    }
    fn bind_stream(&mut self, kind: StreamKind, stage: Arc<dyn StreamInterposer>) {
        self.stream_interposers.push((kind, stage));
    }
    fn bind_vsock_service(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
    ) {
        self.vsock_services.push((spec, service));
    }
    fn service_registry(&mut self) -> &mut ServiceRegistry {
        &mut self.services
    }
}

impl PipelineEngine {
    pub fn new() -> Self {
        Self {
            trap_interceptors: Vec::new(),
            trap_scopes: Vec::new(),
            gate: TrapGateEvaluator::new(Vec::new()),
            fs_interceptors: Vec::new(),
            net_interceptors: Vec::new(),
            console_interceptors: Vec::new(),
            host_interceptors: Vec::new(),
            stream_interposers: Vec::new(),
            vsock_services: Vec::new(),
            services: ServiceRegistry::new(),
            sealed: false,
        }
    }

    pub fn services(&self) -> &ServiceRegistry {
        &self.services
    }

    pub fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope) {
        self.trap_interceptors.push(stage);
        self.trap_scopes.push(scope);
    }

    pub fn seal(&mut self) -> Result<(), PluginError> {
        if self.sealed {
            return Err(PluginError::Halt("pipeline already sealed".into()));
        }
        // TODO (L3): conflicts surface the structured EKindConflict/EPortConflict envelope from the artifacts.
        for (index, (kind, _)) in self.stream_interposers.iter().enumerate() {
            if self.stream_interposers[..index]
                .iter()
                .any(|(bound, _)| bound == kind)
            {
                return Err(PluginError::Halt(format!(
                    "stream kind already bound: {kind:?}"
                )));
            }
        }
        for (index, (spec, _)) in self.vsock_services.iter().enumerate() {
            if self.vsock_services[..index]
                .iter()
                .any(|(bound, _)| bound.port == spec.port)
            {
                return Err(PluginError::Halt(format!(
                    "vsock port already bound: {}",
                    spec.port
                )));
            }
        }
        // TODO: freeze all five ring chains.
        self.sealed = true;
        Ok(())
    }

    pub async fn dispatch_trap(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        // TODO (L3): panic quarantine across await needs an executor-level catch_unwind; a panicking async hook unwinds to the caller.
        for (index, interceptor) in self.trap_interceptors.iter().enumerate() {
            let fallback = MountScope::default();
            let scope = self.trap_scopes.get(index).unwrap_or(&fallback);
            match self.gate.evaluate(event, scope) {
                GateDecision::Allow => continue,
                GateDecision::Deny(verdict) => return Ok(verdict),
                GateDecision::FallThrough => {}
            }
            let verdict = interceptor.on_trap(event, ctx).await?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    pub async fn dispatch_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        for interceptor in &self.trap_interceptors {
            interceptor.on_process_exit(event, ctx).await?;
        }
        Ok(())
    }

    pub fn dispatch_fs_write(
        &self,
        path: &std::path::Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        for interceptor in &self.fs_interceptors {
            let action = interceptor.on_fs_write(path, data, pid, ctx)?;
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    pub fn dispatch_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        for interceptor in &self.net_interceptors {
            let action = interceptor.on_net_outbound(frame)?;
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    pub fn dispatch_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        for interceptor in &self.net_interceptors {
            let action = interceptor.on_net_inbound(frame)?;
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        Ok(FilterAction::Forward)
    }

    pub async fn dispatch_console_input(
        &self,
        port: u32,
        bytes: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for interceptor in &self.console_interceptors {
            interceptor.on_console_input(port, bytes).await?;
        }
        Ok(())
    }

    pub async fn dispatch_console_output(
        &self,
        port: u32,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for interceptor in &self.console_interceptors {
            interceptor.on_console_output(port, chunk).await?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for interceptor in &self.console_interceptors {
            interceptor.on_exec_stdout(pid, exec_id, chunk).await?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        for interceptor in &self.console_interceptors {
            interceptor.on_exec_stderr(pid, exec_id, chunk).await?;
        }
        Ok(())
    }

    pub fn dispatch_host_lsm_exec(
        &self,
        path: &std::path::Path,
        pid: u32,
    ) -> Result<Verdict, PluginError> {
        for interceptor in &self.host_interceptors {
            let verdict = interceptor.on_host_lsm_exec(path, pid)?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    pub fn dispatch_host_pressure(&self, level: u32) -> Result<Verdict, PluginError> {
        for interceptor in &self.host_interceptors {
            let verdict = interceptor.on_host_pressure(level)?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }

    pub fn dispatch_host_keyring_access(&self, key: &str) -> Result<Verdict, PluginError> {
        for interceptor in &self.host_interceptors {
            let verdict = interceptor.on_host_keyring_access(key)?;
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        Ok(Verdict::Pass)
    }
}

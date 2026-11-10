// see: cans/02-plugin-model.md#KelirPlugin-trait
use async_trait::async_trait;
use kelir::interface::driver_interface::{Stream, WatchdogVerdict};
use kelir::interface::plugin_model::HandlerCtx;
use kelir::interface::protocol_wire::Verdict;
use kelir::interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};
use kelir::plugin_model::{FilterAction, MountScope, PluginError};
use kelir::{HNil, KelirBuilder, NoParams, PluginBuilder, ProfilePlugin, Unsealed};

#[cfg(feature = "plugin-chaos")]
use kelir::interface::plugin_model::{FsInterposer, KelirPlugin, NetInterposer};
#[cfg(feature = "plugin-chaos")]
use plugin_chaos::{ChaosConfig, ChaosPlugin};

struct _NullVsock;

#[async_trait]
impl VsockServiceHandler for _NullVsock {
    async fn on_connect(
        &self,
        _peer: &VsockPeer,
        _stream: Box<dyn Stream>,
        _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

fn _compiles(builder: KelirBuilder<Unsealed, HNil, NoParams>) {
    let builder = builder.plugin(ProfilePlugin::new());
    let wrapper = PluginBuilder::new("plain")
        .require::<ProfilePlugin>()
        .context(|| ())
        .scope(MountScope::default())
        .fork_with(|_child| Box::new(ProfilePlugin::new()))
        .on_trap(|_event, _ctx| Ok(Verdict::Pass))
        .on_process_exit(|_event, _ctx| Ok(()))
        .on_fs_lookup(|_path| Ok(Verdict::Pass))
        .on_fs_read(|_path, _buffer| Ok(()))
        .on_fs_write(|_path, _data, _pid, _ctx| Ok(FilterAction::Forward))
        .on_fs_lock(|_path, _cmd, _kind, _pid| Ok(Verdict::Pass))
        .on_net_outbound(|_frame| Ok(FilterAction::Forward))
        .on_net_inbound(|_frame| Ok(FilterAction::Forward))
        .on_console_input(|_port, _bytes| Ok(()))
        .on_console_output(|_port, _chunk| Ok(()))
        .on_exec_stdout(|_pid, _exec_id, _chunk| Ok(()))
        .on_exec_stderr(|_pid, _exec_id, _chunk| Ok(()))
        .on_host_lsm_exec(|_path, _pid| Ok(Verdict::Pass))
        .on_host_pressure(|_level| Ok(Verdict::Pass))
        .on_host_keyring_access(|_key| Ok(Verdict::Pass))
        .on_vm_init(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_ready(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_pre_commit(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_post_commit(|_ctx, _committed| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_host_preempt(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        .on_memory_pressure(|_available, _total| async move { Ok::<(), PluginError>(()) })
        .on_heartbeat(|_uptime| async move { Ok::<(), PluginError>(()) })
        .on_watchdog(|_event| {
            Box::pin(
                async move { Ok::<WatchdogVerdict, PluginError>(WatchdogVerdict::ExecuteDefault) },
            )
        })
        .vsock_service(
            VsockServiceSpec {
                port: 5252,
                service: "plain".to_string(),
            },
            _NullVsock,
        )
        .build();
    let _builder = builder.plugin(wrapper);
}

#[cfg(feature = "plugin-chaos")]
fn _chaos_migrates(builder: KelirBuilder<Unsealed, HNil, NoParams>) {
    let plugin = ChaosPlugin::new(ChaosConfig::default());
    let context_plugin = plugin.clone();
    let net_plugin = plugin.clone();
    let fs_plugin = plugin.clone();
    let wrapper = PluginBuilder::new("chaos")
        .context(move || context_plugin.context())
        .scope(MountScope::default())
        .on_net_outbound(move |frame| net_plugin.on_net_outbound(frame))
        .on_fs_write(move |path, data, pid, ctx| fs_plugin.on_fs_write(path, data, pid, ctx))
        .build();
    let _builder = builder.plugin(wrapper);
}

// see: cans/02-plugin-model.md#Unified-builder
use kelir::interface::driver_interface::default_vm_config;
use kelir::interface::plugin_model::{KelirPlugin, PluginError, VetoEvent};
use kelir::interface::protocol_wire::{PostExecAction, Verdict};
use kelir::plugin_model::FilterAction;
use kelir::{HNil, Host, KelirBuilder, NoParams, TrapFilter, Unsealed};

fn test_builder() -> KelirBuilder<Host, Unsealed, (), HNil, HNil, NoParams> {
    KelirBuilder::new(default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    ))
}

fn error_text<T>(result: Result<T, PluginError>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => String::new(),
    }
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn unbreakable_host_chain_seals() {
    let sealed = test_builder()
        .plugin(KelirBuilder::named("plain").on_trap(|_event, _ctx| Ok(Verdict::Pass)))
        .r#use(
            KelirBuilder::named("second")
                .trap_scope(|t| t.on_trap(|_event, _ctx| Ok(Verdict::Pass)).end()),
        )
        .trap("git", |_event, _ctx| Ok(Verdict::Pass))
        .trap_async("git", |_ctx| async move {
            Ok::<Verdict, PluginError>(Verdict::Pass)
        })
        .post_exec("git", |_ctx| async move {
            Ok::<PostExecAction, PluginError>(PostExecAction::Proceed)
        })
        .trap_scope(|t| t.on_process_exit(|_event, _ctx| Ok(())).end())
        .fs(|f| f.on_fs_read(|_path, buffer| Ok(buffer.to_vec())).end())
        .net(|n| n.on_net_outbound(|_frame| Ok(FilterAction::Forward)).end())
        .console(|c| c.on_console_input(|_port, bytes| Ok(bytes.to_vec())).end())
        .exec(|e| {
            e.on_exec_stdout(|_pid, _exec_id, chunk| Ok(chunk.to_vec()))
                .end()
        })
        .host(|h| h.on_host_pressure(|_level| Ok(Verdict::Pass)).end())
        .lifecycle(|l| {
            l.on_vm_init(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
                .register_veto(VetoEvent::Fork, Box::new(|| true))
                .end()
        })
        .telemetry(|t| {
            t.on_memory_pressure(|_available, _total| async move { Ok::<(), PluginError>(()) })
                .end()
        })
        .group(|b| b.on_net_inbound(|_frame| Ok(FilterAction::Forward)))
        .guard(true, |b| {
            b.on_console_output(|_port, chunk| Ok(chunk.to_vec()))
        })
        .seal();
    assert!(sealed.is_ok());
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn lifecycle_mut_chains_and_returns_builder() {
    let mut builder = test_builder();
    builder
        .lifecycle_mut()
        .register_veto(VetoEvent::Fork, Box::new(|| true))
        .register_veto(VetoEvent::Suspend, Box::new(|| false))
        .end();
    let sealed = builder.on_trap(|_event, _ctx| Ok(Verdict::Pass)).seal();
    assert!(sealed.is_ok());
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn plugin_mode_has_both_trap_forms() {
    let plugin = KelirBuilder::named("both")
        .trap("git", |_event, _ctx| Ok(Verdict::Pass))
        .trap_scope(|t| t.on_trap(|_event, _ctx| Ok(Verdict::Pass)).end());
    assert!(test_builder().plugin(plugin).seal().is_ok());
    let host_scoped = test_builder()
        .trap_scope(|t| t.on_trap(|_event, _ctx| Ok(Verdict::Pass)).end())
        .seal();
    assert!(host_scoped.is_ok());
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn seal_rejects_duplicate_mount() {
    let text = error_text(
        test_builder()
            .plugin(KelirBuilder::named("dup"))
            .plugin(KelirBuilder::named("dup"))
            .seal(),
    );
    assert!(
        text.contains("EConfig") && text.contains("plugin already mounted: dup"),
        "unexpected error: {text}"
    );
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
#[test]
fn seal_rejects_empty_trap_filter() {
    let host_binary = test_builder()
        .trap("", |_event, _ctx| Ok(Verdict::Pass))
        .seal();
    let text = error_text(host_binary);
    assert!(
        text.contains("EConfig") && text.contains("trap filter binary must not be empty"),
        "unexpected error: {text}"
    );
    let host_arg = test_builder()
        .trap_async(TrapFilter::binary("git").with_arg(""), |_ctx| async move {
            Ok::<Verdict, PluginError>(Verdict::Pass)
        })
        .seal();
    let text = error_text(host_arg);
    assert!(
        text.contains("EConfig") && text.contains("trap filter arg must not be empty"),
        "unexpected error: {text}"
    );
    let mounted = test_builder()
        .plugin(KelirBuilder::named("bad-filter").trap("", |_event, _ctx| Ok(Verdict::Pass)))
        .seal();
    let text = error_text(mounted);
    assert!(
        text.contains("EConfig")
            && text.contains("bad-filter")
            && text.contains("trap filter binary must not be empty"),
        "unexpected error: {text}"
    );
    let post_exec = test_builder()
        .post_exec("", |_ctx| async move {
            Ok::<PostExecAction, PluginError>(PostExecAction::Proceed)
        })
        .seal();
    let text = error_text(post_exec);
    assert!(
        text.contains("EConfig") && text.contains("trap filter binary must not be empty"),
        "unexpected error: {text}"
    );
}

// see: cans/02-plugin-model.md#Chain-decisions
#[test]
fn plugin_mode_fork_state_clones_and_fork_with_wins() {
    let plugin = KelirBuilder::named("forked");
    let cloned = plugin.fork_state("child");
    assert_eq!(cloned.name(), "forked");
    let custom = KelirBuilder::named("parent")
        .fork_with(|_child| Box::new(KelirBuilder::named("child-plugin")));
    let forked = custom.fork_state("child");
    assert_eq!(forked.name(), "child-plugin");
}

// see: cans/02-plugin-model.md#Unified-builder
#[test]
fn guard_applies_only_when_enabled() {
    let skipped = test_builder()
        .guard(false, |b| b.trap("", |_event, _ctx| Ok(Verdict::Pass)))
        .seal();
    assert!(skipped.is_ok());
    let applied = test_builder()
        .guard(true, |b| b.trap("", |_event, _ctx| Ok(Verdict::Pass)))
        .seal();
    let text = error_text(applied);
    assert!(
        text.contains("EConfig") && text.contains("trap filter binary must not be empty"),
        "unexpected error: {text}"
    );
}

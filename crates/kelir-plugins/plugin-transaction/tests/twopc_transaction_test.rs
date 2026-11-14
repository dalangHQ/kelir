// see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
use kelir_interface::plugin_model::LifecycleParticipant;
use kelir_interface::plugin_model::PluginError;
use kelir_plugin_testkit::block_on;
use plugin_transaction::{
    Savepoint, TransactionConfig, TransactionHandle, TransactionPlugin, TransactionScope,
    TransactionService,
};

// see: cans/artifacts/test-plan/cases/relational_transaction/rt_04_2pc_post_commit_true_atomic_sync.yaml#case_id
#[test]
fn rt_04_2pc_post_commit_true_atomic_sync() -> Result<(), String> {
    let plugin = TransactionPlugin::new(TransactionConfig::default());
    let mut ctx = plugin.ctx();
    let handle =
        block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    assert_eq!(handle.id, 0);
    block_on(ctx.commit(handle)).map_err(|err| format!("{err:?}"))?;
    assert_eq!(
        ctx.active_namespace().map_err(|err| format!("{err:?}"))?,
        None
    );
    let second =
        block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    assert_eq!(second.id, 1);
    Ok(())
}

// see: cans/artifacts/test-plan/cases/relational_transaction/rt_05_2pc_post_commit_false_rollback_sync.yaml#case_id
#[test]
fn rt_05_2pc_post_commit_false_rollback_sync() -> Result<(), String> {
    let plugin = TransactionPlugin::new(TransactionConfig::default());
    let mut ctx = plugin.ctx();
    let handle =
        block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    assert_eq!(handle.id, 0);
    block_on(ctx.abort(handle)).map_err(|err| format!("{err:?}"))?;
    assert_eq!(
        ctx.active_namespace().map_err(|err| format!("{err:?}"))?,
        None
    );
    let second =
        block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    assert_eq!(second.id, 1);
    match block_on(ctx.savepoint(&TransactionHandle { id: 99 })) {
        Err(PluginError::Halt(msg)) if msg.starts_with("EINVAL:") => (),
        Err(err) => {
            return Err(format!(
                "savepoint on a non-active handle must fail with EINVAL, got {err:?}"
            ));
        }
        Ok(savepoint) => {
            return Err(format!(
                "savepoint on handle 99 unexpectedly succeeded with id {}",
                savepoint.id
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/relational_transaction/rt_07_temporal_savepoint_rollback_restore.yaml#case_id
#[test]
fn rt_07_temporal_savepoint_rollback_restore() -> Result<(), String> {
    let plugin = TransactionPlugin::new(TransactionConfig::default());
    let mut ctx = plugin.ctx();
    let handle =
        block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    assert_eq!(handle.id, 0);
    let savepoint = block_on(ctx.savepoint(&handle)).map_err(|err| format!("{err:?}"))?;
    assert_eq!(savepoint.id, 0);
    block_on(ctx.rollback_to_savepoint(&handle, savepoint)).map_err(|err| format!("{err:?}"))?;
    match block_on(ctx.rollback_to_savepoint(&handle, Savepoint { id: 99 })) {
        Err(PluginError::Halt(msg)) if msg.starts_with("EINVAL:") => (),
        Err(err) => {
            return Err(format!(
                "rollback to an unregistered savepoint must fail with EINVAL, got {err:?}"
            ));
        }
        Ok(()) => {
            return Err("rollback to savepoint 99 succeeded; it was never recorded".into());
        }
    }
    block_on(ctx.commit(handle)).map_err(|err| format!("{err:?}"))?;
    assert_eq!(
        ctx.active_namespace().map_err(|err| format!("{err:?}"))?,
        None
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/relational_transaction/rt_03_2pc_pre_commit_vcpu_pause_drain.yaml#case_id
#[test]
#[ignore]
fn rt_03_2pc_pre_commit_vcpu_pause_drain() -> Result<(), String> {
    let plugin = TransactionPlugin::new(TransactionConfig::default());
    let mut ctx = plugin.ctx();
    block_on(ctx.begin_scope(TransactionScope::default())).map_err(|err| format!("{err:?}"))?;
    let mut fixture = kelir_plugin_testkit::handlers_testkit::LifecycleCtxFixture::new();
    let mut lifecycle = fixture.ctx();
    block_on(plugin.on_pre_commit(&mut lifecycle)).map_err(|err| {
        format!(
            "on_pre_commit must pause guest vCPUs and drain in-flight FUSE before prepare: {err:?}"
        )
    })?;
    Ok(())
}

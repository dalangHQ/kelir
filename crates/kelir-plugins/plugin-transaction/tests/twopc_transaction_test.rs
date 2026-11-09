// see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use kelir_interface::plugin_model::PluginError;
use plugin_transaction::{
    Savepoint, TransactionConfig, TransactionHandle, TransactionPlugin, TransactionScope,
    TransactionService,
};

// see: cans/15-operations.md#Local-velocity
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

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
        Err(PluginError::Halt(message)) if message.starts_with("EINVAL:") => (),
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
        Err(PluginError::Halt(message)) if message.starts_with("EINVAL:") => (),
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

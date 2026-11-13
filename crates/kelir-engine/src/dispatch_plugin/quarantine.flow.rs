// see: cans/artifacts/code/plugin-trait.rs#PANIC_QUARANTINE_ENABLED
use super::*;
// see: cans/artifacts/code/plugin-trait.rs#PANIC_QUARANTINE_ENABLED
pub(super) struct Quarantined<F> {
    inner: Option<Pin<Box<F>>>,
}

impl<F: Future> Quarantined<F> {
    pub(super) fn new(future: F) -> Self {
        Self {
            inner: Some(Box::pin(future)),
        }
    }
}

impl<F: Future> Future for Quarantined<F> {
    type Output = Result<F::Output, PluginError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(inner) = this.inner.as_mut() else {
            return Poll::Ready(Err(PluginError::Halt(
                "quarantined future polled after completion".into(),
            )));
        };
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: log the panic payload and backtrace to the host ledger and roll back the active StepSlot; slop: panic logs without StepSlot rollback, tainted state kept
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Future::poll(inner.as_mut(), cx)
        })) {
            Ok(Poll::Ready(output)) => {
                this.inner = None;
                Poll::Ready(Ok(output))
            }
            Ok(Poll::Pending) => Poll::Pending,
            Err(_) => {
                this.inner = None;
                Poll::Ready(Err(PluginError::Halt("plugin panicked".into())))
            }
        }
    }
}

pub(super) async fn quarantine_verdict<F>(future: F) -> Result<Verdict, PluginError>
where
    F: Future<Output = Result<Verdict, PluginError>>,
{
    match Quarantined::new(future).await {
        Ok(output) => output,
        Err(_) => Ok(Verdict::Drop {
            errno: Some(Errno::Efault),
            reason: "Plugin panicked".into(),
        }),
    }
}

pub(super) async fn quarantine_unit<F, T>(future: F) -> Result<T, PluginError>
where
    F: Future<Output = Result<T, PluginError>>,
{
    Quarantined::new(future).await?
}

pub(super) fn quarantine_sync<T>(run: impl FnOnce() -> T, on_panic: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(output) => output,
        Err(_) => on_panic(),
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub(crate) fn map_reject_to_verdict(
    result: Result<Verdict, PluginError>,
) -> Result<Verdict, PluginError> {
    match result {
        Err(PluginError::Reject { errno, reason }) => Ok(Verdict::Drop {
            errno: Some(errno),
            reason,
        }),
        other => other,
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub(crate) fn map_reject_to_filter_action(
    result: Result<FilterAction, PluginError>,
) -> Result<FilterAction, PluginError> {
    match result {
        Err(PluginError::Reject { errno, reason }) => Ok(FilterAction::Drop {
            errno: Some(errno),
            reason,
        }),
        other => other,
    }
}

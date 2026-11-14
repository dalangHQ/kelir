// see: cans/artifacts/code/plugin-trait.rs#PANIC_QUARANTINE_ENABLED
use super::*;

pub(crate) struct Quarantined<F> {
    inner: Option<Pin<Box<F>>>,
}

impl<F: Future> Quarantined<F> {
    pub(crate) fn new(future: F) -> Self {
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

pub(crate) async fn quarantine_verdict<F>(future: F) -> Result<Verdict, PluginError>
where
    F: Future<Output = Result<Verdict, PluginError>>,
{
    match Quarantined::new(future).await {
        Ok(output) => output,
        Err(_) => Ok(Verdict::Drop {
            errno: Errno::Efault,
            reason: "Plugin panicked".into(),
        }),
    }
}

pub(crate) async fn quarantine_unit<F, T>(future: F) -> Result<T, PluginError>
where
    F: Future<Output = Result<T, PluginError>>,
{
    Quarantined::new(future).await?
}

pub(crate) fn quarantine_sync<T>(run: impl FnOnce() -> T, on_panic: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(output) => output,
        Err(_) => on_panic(),
    }
}

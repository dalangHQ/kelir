// see: cans/08-lifecycle.md#State-machine
use crate::driver_interface::SuspendReason;

// see: cans/08-lifecycle.md#State-machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Initializing,
    Running,
    Draining,
    Suspended(SuspendReason),
    Terminated,
}

// see: cans/08-lifecycle.md#State-machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    IllegalTransition {
        from: LifecycleState,
        to: LifecycleState,
    },
}

impl LifecycleState {
    // see: cans/08-lifecycle.md#State-machine
    pub fn transition(self, next: LifecycleState) -> Result<LifecycleState, LifecycleError> {
        let legal = matches!(
            (self, next),
            (Self::Initializing, Self::Running)
                | (Self::Running, Self::Draining)
                | (Self::Running, Self::Suspended(_))
                | (Self::Running, Self::Terminated)
                | (Self::Draining, Self::Terminated)
                | (Self::Suspended(_), Self::Running)
                | (Self::Suspended(_), Self::Terminated)
        );
        if legal {
            Ok(next)
        } else {
            Err(LifecycleError::IllegalTransition {
                from: self,
                to: next,
            })
        }
    }
}

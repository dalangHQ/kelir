// see: cans/02-plugin-model.md#Facade-blindness
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::plugin_model::{HandlerCtx, PluginError};
use crate::protocol_wire::TrapEvent;

// see: cans/02-plugin-model.md#Facade-blindness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostExecAction {
    Proceed,
    RollbackAndHalt,
}

// see: cans/02-plugin-model.md#Facade-blindness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepVerdict {
    Commit,
    Rollback,
    YieldToHost,
}

impl PostExecAction {
    // see: cans/02-plugin-model.md#Facade-blindness
    pub const fn to_step_verdict(self) -> StepVerdict {
        match self {
            Self::Proceed => StepVerdict::Commit,
            Self::RollbackAndHalt => StepVerdict::Rollback,
        }
    }
}

// see: cans/02-plugin-model.md#Facade-blindness
#[async_trait]
pub trait PostExecGate: Send + Sync {
    async fn check(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<PostExecAction, PluginError> {
        let _ = (event, ctx);
        Ok(PostExecAction::Proceed)
    }
}

// see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
use super::*;

pub type TrapDenyCallback = Box<
    dyn Fn(&TrapEvent, &Session) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub type CustomActionCallback = Box<
    dyn Fn(
            ciborium::value::Value,
        ) -> Pin<
            Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>,
        > + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub struct CustomActionBinding {
    pub name: String,
    pub handler: CustomActionCallback,
}

// see: cans/08-lifecycle.md#CoW-fork
#[derive(Clone)]
pub struct HostRegistrationState {
    pub on_trap_deny: Option<Arc<TrapDenyCallback>>,
    pub custom_actions: Vec<Arc<CustomActionBinding>>,
    pub stream_kind_services: Vec<(u16, Arc<dyn StreamFrameSink>)>,
    pub lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    pub token_revoker: Option<Arc<dyn TokenRevocation>>,
}

// see: cans/08-lifecycle.md#CoW-fork
use super::*;

impl PipelineEngine {
    // see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
    pub fn set_on_trap_deny(&mut self, callback: TrapDenyCallback) {
        self.on_trap_deny = Some(Arc::new(callback));
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn set_on_trap_deny_shared(&mut self, callback: Arc<TrapDenyCallback>) {
        self.on_trap_deny = Some(callback);
    }

    // see: cans/03-sdk-architecture.md#Rust-SDK
    pub fn register_custom_action(&mut self, name: String, handler: CustomActionCallback) {
        self.custom_actions
            .push(Arc::new(CustomActionBinding { name, handler }));
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn register_custom_action_shared(&mut self, binding: Arc<CustomActionBinding>) {
        self.custom_actions.push(binding);
    }

    // see: cans/11-protocol.md#Kind-registration
    pub fn register_stream_kind(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>) {
        self.stream_sinks.push((kind, sink, MountScope::default()));
    }

    // see: cans/11-protocol.md#Kind-registration
    pub fn stream_kind_services(&self) -> Vec<(StreamKind, Arc<dyn StreamFrameSink>)> {
        self.stream_sinks
            .iter()
            .map(|(kind, sink, _)| (*kind, Arc::clone(sink)))
            .collect()
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn register_fused_callback(
        &mut self,
        ring: TrampolineRing,
        ordinal: u32,
        callback: Arc<dyn FusedTrampolineCallback>,
    ) {
        self.fused_callbacks.push((ring, ordinal, callback));
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn register_fused_callbacks(&mut self, descriptor: &FusedTrampolineDescriptor) {
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(callback) = hook.run.clone() {
                    self.register_fused_callback(chain.ring, hook.ordinal, callback);
                }
            }
        }
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn host_registration_state(&self) -> HostRegistrationState {
        HostRegistrationState {
            on_trap_deny: self.on_trap_deny.as_ref().map(Arc::clone),
            custom_actions: self.custom_actions.iter().map(Arc::clone).collect(),
            stream_kind_services: self.stream_kind_services(),
            lock_broker: self.lock_broker(),
            token_revoker: self.token_revoker(),
        }
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn receipt_store(&self) -> &ReceiptStore {
        &self.receipt_store
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn store_receipt(&self, receipt: ExecutionReceipt) {
        self.receipt_store.insert(receipt);
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn receipt(&self, pid: u32) -> Option<ExecutionReceipt> {
        self.receipt_store.lookup(pid)
    }

    // see: cans/08-lifecycle.md#Rollback
    pub fn register_token_revoker(&mut self, revoker: Arc<dyn TokenRevocation>) {
        self.token_revoker = Some(revoker);
    }

    // see: cans/08-lifecycle.md#CoW-fork
    // TODO (L2): sinks re-register under MountScope::default(); a fork_state that re-binds the same kind leaves a duplicate entry resolved first-match by stream_sink.
    pub fn rehydrate_host_registration(&mut self, state: HostRegistrationState) {
        if let Some(callback) = state.on_trap_deny {
            self.set_on_trap_deny_shared(callback);
        }
        for binding in state.custom_actions {
            self.register_custom_action_shared(binding);
        }
        for (kind, sink) in state.stream_kind_services {
            self.register_stream_kind(kind, sink);
        }
        if let Some(broker) = state.lock_broker {
            self.register_lock_broker(broker);
        }
        if let Some(revoker) = state.token_revoker {
            self.register_token_revoker(revoker);
        }
    }
}

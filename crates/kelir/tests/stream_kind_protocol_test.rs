// see: cans/artifacts/test-plan/cases/protocol_mux/pm_03_core_stream_kind_reregistration_abort.yaml#case_id
use kelir::builder_sdk::KelirBuilder;
use kelir::interface::driver_interface::default_vm_config;
use kelir::plugin_model::PluginError;

fn builder_config() -> kelir::interface::driver_interface::VmConfig {
    default_vm_config(
        None,
        None,
        Some("kernel/vmlinux".to_string()),
        Some("rootfs.erofs".to_string()),
        None,
        None,
    )
}

type HandlerOutput = Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
type HandlerFuture = std::future::Ready<HandlerOutput>;

fn stream_kind_handler() -> impl Fn(Vec<u8>) -> HandlerFuture + Clone + Send + Sync + 'static {
    move |_request: Vec<u8>| std::future::ready(Ok(Vec::new()))
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_03_core_stream_kind_reregistration_abort.yaml#case_id
#[tokio::test]
#[ignore]
async fn pm_03_core_stream_kind_reregistration_abort() -> Result<(), String> {
    let builder = KelirBuilder::new(builder_config()).register_stream_kind(
        0x0001,
        serde_json::json!({}),
        stream_kind_handler(),
    );
    match builder.boot().await {
        Err(PluginError::Halt(message)) => {
            assert!(
                message.contains("EKindReserved"),
                "core-kind re-registration aborted with the wrong diagnostic: {message}"
            );
        }
        Err(e) => return Err(format!("core-kind re-registration gave wrong error: {e}")),
        Ok(_) => {
            return Err(
                "bootstrap sealed with a plugin re-registering the frozen core kind 0x0001 EXEC"
                    .to_string(),
            );
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/protocol_mux/pm_05_stream_kind_conflict_ekindconflict.yaml#case_id
#[tokio::test]
#[ignore]
async fn pm_05_stream_kind_conflict_ekindconflict() -> Result<(), String> {
    let builder = KelirBuilder::new(builder_config())
        .register_stream_kind(0x0019, serde_json::json!({}), stream_kind_handler())
        .register_stream_kind(0x0019, serde_json::json!({}), stream_kind_handler());
    match builder.boot().await {
        Err(PluginError::Halt(message)) => {
            assert!(
                message.contains("EKindConflict"),
                "kind collision aborted with the wrong diagnostic: {message}"
            );
        }
        Err(e) => return Err(format!("kind collision gave wrong error: {e}")),
        Ok(_) => {
            return Err(
                "bootstrap sealed with two plugins registered for the same stream kind 0x0019"
                    .to_string(),
            );
        }
    }
    Ok(())
}

// see: cans/11-protocol.md#Kind-registration
struct ContractSink;

// see: cans/11-protocol.md#Kind-registration
impl kelir::interface::plugin_model::StreamFrameSink for ContractSink {
    fn on_stream_frame(
        &self,
        _stream_id: u32,
        _kind: kelir::interface::protocol_wire::StreamKind,
        _flags: u8,
        _payload: &[u8],
    ) -> Result<(), kelir::plugin_model::PluginError> {
        Ok(())
    }
}

// see: cans/11-protocol.md#Kind-registration
#[test]
fn engine_stream_kind_services_bind_canonical_sink() -> Result<(), String> {
    let mut engine = kelir_core::dispatch_plugin::PipelineEngine::new();
    engine.register_stream_kind(
        kelir::interface::protocol_wire::StreamKind::Bulk,
        std::sync::Arc::new(ContractSink),
    );
    let services = engine.stream_kind_services();
    assert_eq!(services.len(), 1);
    let mut bus = kelir_core::bus::OwnedBusDispatcher::new(1024, 1024);
    for (kind, sink) in services {
        bus.dispatcher()
            .bind_stream_sink(kind, sink)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

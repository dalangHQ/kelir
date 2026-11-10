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

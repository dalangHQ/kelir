// see: cans/plugins/tool-guard.md#Immutable-schema-locking
use kelir_interface::plugin_model::ConsoleInterposer;
use kelir_interface::plugin_model::PluginError;
use kelir_plugin_testkit::block_on;
use plugin_tool_guard::ToolGuardPlugin;
use plugin_tool_guard::service_tool_guard::{
    ToolRegistry, ToolRegistryService, ToolSchema, ToolsListResult,
};

// see: cans/artifacts/test-plan/cases/governance_tools/gt_02_immutable_schema_locking_esecurity.yaml#case_id
#[test]
fn gt_02_immutable_schema_locking_esecurity() -> Result<(), String> {
    let registry = ToolRegistry::new();
    let schema = ToolSchema {
        name: "execute_command".into(),
        description: "run a command".into(),
        input_schema: b"{}".to_vec(),
        signature: vec![1, 2, 3],
    };
    let advertised = ToolsListResult {
        tools: vec![schema.clone()],
    };
    block_on(registry.ingest_tools_list(&advertised)).map_err(|err| format!("{err:?}"))?;
    let found =
        block_on(registry.lookup_schema("execute_command")).map_err(|err| format!("{err:?}"))?;
    assert_eq!(found, Some(schema.clone()));
    block_on(registry.lock_catalog()).map_err(|err| format!("{err:?}"))?;
    match registry.locked_digest().map_err(|err| format!("{err:?}"))? {
        Some(digest) => assert_eq!(digest.len(), 32),
        None => return Err("locked_digest is None after lock_catalog".into()),
    }
    let drifted = ToolSchema {
        name: "execute_command".into(),
        description: "run a command with extra parameters".into(),
        input_schema: b"{}".to_vec(),
        signature: vec![1, 2, 3],
    };
    let drifted_list = ToolsListResult {
        tools: vec![drifted],
    };
    match block_on(registry.ingest_tools_list(&drifted_list)) {
        Err(PluginError::Reject { errno, .. }) if errno == "ESecurityViolation" => (),
        Err(err) => {
            return Err(format!(
                "post-lock drift must halt with ESecurityViolation, got {err:?}"
            ));
        }
        Ok(()) => {
            return Err("post-lock schema drift was accepted; the locked catalog must halt".into());
        }
    }
    block_on(registry.ingest_tools_list(&advertised)).map_err(|err| format!("{err:?}"))?;
    Ok(())
}

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
#[test]
fn tool_guard_unparsed_frame_fails_closed() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    let mut bytes = b"\x00not-a-json-rpc-frame".to_vec();
    match block_on(plugin.on_console_input(0x0012, &mut bytes)) {
        Err(PluginError::Reject { errno, reason }) => {
            if errno != "EPERM" {
                return Err(format!(
                    "unparsed frame must reject with EPERM, got {errno}"
                ));
            }
            if !reason.contains("unparsed MCP frame fails closed") {
                return Err(format!(
                    "rejection must carry a concrete reason, got {reason}"
                ));
            }
        }
        Err(err) => {
            return Err(format!(
                "unparsed frame must reject with EPERM, got {err:?}"
            ));
        }
        Ok(()) => {
            return Err(
                "unparsed MCP frame passed through tool-guard; interposition must fail closed"
                    .into(),
            );
        }
    }
    Ok(())
}

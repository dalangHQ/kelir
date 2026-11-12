// see: cans/plugins/tool-guard.md#Immutable-schema-locking
use kelir_interface::plugin_model::ConsoleInterposer;
use kelir_interface::plugin_model::KelirPlugin;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Errno;
use kelir_plugin_testkit::block_on;
use plugin_tool_guard::ToolGuardPlugin;
use plugin_tool_guard::service_tool_guard::{
    ToolRegistry, ToolRegistryService, ToolSchema, ToolTaint, ToolsListResult,
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
        Err(PluginError::Reject {
            errno: Errno::Esecurityviolation,
            ..
        }) => (),
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
    let bytes = b"\x00not-a-json-rpc-frame".to_vec();
    match block_on(plugin.on_console_input(0x0012, &bytes)) {
        Err(PluginError::Reject { errno, reason }) => {
            if errno != Errno::Eperm {
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
        Ok(_) => {
            return Err(
                "unparsed MCP frame passed through tool-guard; interposition must fail closed"
                    .into(),
            );
        }
    }
    Ok(())
}

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
#[test]
fn tool_guard_pty_port_0_not_interposed() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    let bytes = b"\x00not-a-json-rpc-frame".to_vec();
    let before = bytes.clone();
    let returned =
        block_on(plugin.on_console_input(0, &bytes)).map_err(|err| format!("{err:?}"))?;
    if returned != before {
        return Err("tool-guard mutated PTY port 0 bytes; interposition must not reach it".into());
    }
    let chunk = b"\x00not-a-json-rpc-frame".to_vec();
    let chunk_before = chunk.clone();
    let returned =
        block_on(plugin.on_console_output(0, &chunk)).map_err(|err| format!("{err:?}"))?;
    if returned != chunk_before {
        return Err("tool-guard mutated PTY port 0 output; interposition must not reach it".into());
    }
    Ok(())
}

// see: cans/plugins/tool-guard.md#Immutable-schema-locking
#[test]
fn tool_guard_locks_after_first_advertisement() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    let registry = plugin.context().registry();
    if registry
        .locked_digest()
        .map_err(|err| format!("{err:?}"))?
        .is_some()
    {
        return Err(
            "catalog locked before any advertisement; the boot-time lock must not fire".into(),
        );
    }
    let schema = ToolSchema {
        name: "execute_command".into(),
        description: "run a command".into(),
        input_schema: b"{}".to_vec(),
        signature: vec![1, 2, 3],
    };
    let advertised = ToolsListResult {
        tools: vec![schema.clone()],
    };
    block_on(plugin.ingest_advertisement(&advertised))
        .map_err(|err| format!("first advertisement rejected: {err:?}"))?;
    if registry
        .locked_digest()
        .map_err(|err| format!("{err:?}"))?
        .is_none()
    {
        return Err("first advertisement did not lock the catalog".into());
    }
    let drifted = ToolSchema {
        description: "run a command with extra parameters".into(),
        ..schema
    };
    let drifted_list = ToolsListResult {
        tools: vec![drifted],
    };
    match block_on(plugin.ingest_advertisement(&drifted_list)) {
        Err(PluginError::Reject {
            errno: Errno::Esecurityviolation,
            ..
        }) => (),
        Err(err) => {
            return Err(format!(
                "post-lock drift must halt with ESecurityViolation, got {err:?}"
            ));
        }
        Ok(()) => {
            return Err("post-lock schema drift was accepted; the locked catalog must halt".into());
        }
    }
    block_on(plugin.ingest_advertisement(&advertised))
        .map_err(|err| format!("identical advertisement must stay accepted: {err:?}"))?;
    Ok(())
}

fn tools_call_frame(name: &str, id: u64) -> Vec<u8> {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"tools/call\",\"params\":{{\"name\":\"{name}\",\"arguments\":{{}}}}}}\n"
    )
    .into_bytes()
}

fn pin_tool(plugin: &ToolGuardPlugin, name: &str) -> Result<(), String> {
    let registry = plugin.context().registry();
    let schema = ToolSchema {
        name: name.into(),
        description: "pinned".into(),
        input_schema: b"{}".to_vec(),
        signature: vec![1, 2, 3],
    };
    let advertisement = ToolsListResult {
        tools: vec![schema],
    };
    block_on(registry.ingest_tools_list(&advertisement)).map_err(|err| format!("{err:?}"))?;
    block_on(registry.lock_catalog()).map_err(|err| format!("{err:?}"))
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_04_token_bucket_call_quota_etoolquota.yaml#case_id
#[test]
#[ignore]
fn gt_04_token_bucket_call_quota_etoolquota() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "shell")?;
    for call in 1..=5u64 {
        let frame = tools_call_frame("shell", call);
        match block_on(plugin.on_console_input(0x0012, &frame)) {
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "burst call {call} under bucket capacity 5 must pass, got {e:?}"
                ));
            }
        }
    }
    let sixth = tools_call_frame("shell", 6);
    match block_on(plugin.on_console_input(0x0012, &sixth)) {
        Err(PluginError::Halt(message)) if message.starts_with("ETOOLQUOTA:") => {}
        Err(e) => {
            return Err(format!("6th burst call must reject ETOOLQUOTA, got {e:?}"));
        }
        Ok(_) => {
            return Err("6th burst call passed; the token-bucket governor must exhaust".into());
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_06_payload_secret_pattern_scrub.yaml#case_id
#[test]
#[ignore]
fn gt_06_payload_secret_pattern_scrub() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    let chunk = b"Authentication failed with token: ghp_secret_token_1234567890abcdef".to_vec();
    let scrubbed = block_on(plugin.on_console_output(0x0012, &chunk)).map_err(|e| e.to_string())?;
    let text = String::from_utf8(scrubbed).map_err(|e| e.to_string())?;
    assert!(
        !text.contains("ghp_secret_token_1234567890abcdef"),
        "host secret must be purged from the scrubbed output, got: {text}"
    );
    assert_eq!(
        text, "Authentication failed with token: [REDACTED]",
        "scrubbed output must carry the literal redaction"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_08_schema_drift_method_not_found.yaml#case_id
#[test]
#[ignore]
fn gt_08_schema_drift_method_not_found() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "read_file")?;
    let mut frame = tools_call_frame("unregistered_tool", 1);
    match block_on(plugin.on_console_input(0x0012, &frame)) {
        Ok(returned) => {
            frame = returned;
        }
        Err(e) => {
            return Err(format!(
                "unpinned tool call must return a framed JSON-RPC error, got {e:?}"
            ));
        }
    }
    let text = String::from_utf8(frame).map_err(|e| e.to_string())?;
    assert!(
        text.contains("-32601"),
        "unpinned tool must synthesize JSON-RPC error code -32601 (Method not found), got: {text}"
    );
    assert!(
        text.contains("unregistered_tool"),
        "the -32601 error must name the unpinned tool, got: {text}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_09_verdict_drop_jsonrpc_error_synthesis.yaml#case_id
#[test]
#[ignore]
fn gt_09_verdict_drop_jsonrpc_error_synthesis() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "read_file")?;
    let mut frame = b"\x00not-a-json-rpc-frame".to_vec();
    match block_on(plugin.on_console_input(0x0012, &frame)) {
        Ok(returned) => {
            frame = returned;
        }
        Err(e) => {
            return Err(format!(
                "Drop verdict must keep the stream open with a framed error, got {e:?}"
            ));
        }
    }
    let text = String::from_utf8(frame).map_err(|e| e.to_string())?;
    assert!(
        text.contains("-32001"),
        "EPERM Drop must synthesize JSON-RPC error code -32001, got: {text}"
    );
    assert!(
        text.contains("unparsed"),
        "the framed error must carry the drop reason, got: {text}"
    );
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_10_tool_call_depth_recursion_ceiling.yaml#case_id
#[test]
#[ignore]
fn gt_10_tool_call_depth_recursion_ceiling() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "shell")?;
    let mut saw_pass = false;
    for call in 1..=51u64 {
        let frame = tools_call_frame("shell", call);
        match block_on(plugin.on_console_input(0x0012, &frame)) {
            Ok(_) => {
                if call == 51 {
                    return Err("51st call passed; the per-trace ceiling must reject".into());
                }
                saw_pass = true;
            }
            Err(PluginError::Halt(message)) if message.starts_with("ETOOLQUOTA:") => {
                if call == 51 {
                    assert!(
                        saw_pass,
                        "no call in the first 50 passed; the ceiling test needs live calls"
                    );
                    return Ok(());
                }
            }
            Err(e) => {
                if call == 51 {
                    return Err(format!("51st call must reject ETOOLQUOTA, got {e:?}"));
                }
            }
        }
    }
    assert!(
        saw_pass,
        "no call in the first 50 passed; the ceiling test needs live calls"
    );
    Err("the 51st call never rejected with ETOOLQUOTA".into())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_01_jsonrpc_tool_call_interposition.yaml#case_id
#[test]
#[ignore]
fn gt_01_jsonrpc_tool_call_interposition() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "execute_command")?;
    let frame = tools_call_frame("execute_command", 1);
    match block_on(plugin.on_console_input(0x0012, &frame)) {
        Ok(_) => {}
        Err(e) => {
            return Err(format!(
                "tools/call frame for pinned execute_command must be intercepted and forwarded, got {e:?}"
            ));
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_03_taint_tracking_mutation_suspend.yaml#case_id
#[test]
#[ignore]
fn gt_03_taint_tracking_mutation_suspend() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "write_file")?;
    let registry = plugin.context().registry();
    registry
        .mark_tainted("write_file")
        .map_err(|err| format!("{err:?}"))?;
    let frame = br#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"write_file","arguments":{"path":"deploy.sh","content":"rm -rf /"}}}"#
        .to_vec();
    match block_on(plugin.on_console_input(0x0012, &frame)) {
        Err(PluginError::Halt(message))
            if message.contains("approval") || message.contains("suspend") => {}
        Err(e) => {
            return Err(format!(
                "tainted write_file must suspend for approval, got {e:?}"
            ));
        }
        Ok(_) => {
            return Err("tainted write_file passed without the approval suspend".into());
        }
    }
    Ok(())
}

// see: cans/artifacts/test-plan/cases/governance_tools/gt_12_model_controlled_flag_taint_override.yaml#case_id
#[test]
#[ignore]
fn gt_12_model_controlled_flag_taint_override() -> Result<(), String> {
    let plugin = ToolGuardPlugin::new();
    pin_tool(&plugin, "write_file")?;
    let registry = plugin.context().registry();
    registry
        .mark_tainted("write_file")
        .map_err(|err| format!("{err:?}"))?;
    let frame = br#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"write_file","arguments":{"path":"a.txt","override_taint":true,"trusted":true}}}"#
        .to_vec();
    match block_on(plugin.on_console_input(0x0012, &frame)) {
        Err(PluginError::Halt(message))
            if message.contains("approval") || message.contains("suspend") => {}
        Err(e) => {
            return Err(format!(
                "override_taint arguments must not bypass the approval gate, got {e:?}"
            ));
        }
        Ok(_) => {
            return Err("override_taint arguments bypassed the taint gate".into());
        }
    }
    match registry.taint("write_file") {
        Ok(ToolTaint::Tainted) => Ok(()),
        Ok(state) => Err(format!(
            "host taint flag must stay Tainted after the call, got {state:?}"
        )),
        Err(err) => Err(format!("taint read failed: {err:?}")),
    }
}

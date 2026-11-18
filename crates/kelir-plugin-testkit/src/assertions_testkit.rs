// see: cans/02-plugin-model.md#Unified-verdict
// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Verdict;

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_pass(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_pass pending; slop: testkit verdict assertion unchecked"
            .into(),
    ))
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_deny pending; slop: testkit verdict assertion unchecked"
            .into(),
    ))
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_patch(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_patch pending; slop: testkit verdict assertion unchecked".into(),
    ))
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_suspend(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_suspend pending; slop: testkit verdict assertion unchecked".into(),
    ))
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_abort(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_abort pending; slop: testkit verdict assertion unchecked".into(),
    ))
}

// see: cans/02-plugin-model.md#Hook-return-semantics
pub fn assert_rewrite(input: &[u8], output: &[u8]) -> Result<(), PluginError> {
    let _ = (input, output);
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_rewrite pending; slop: rewrite assertion unchecked"
            .into(),
    ))
}

pub fn assert_env_injected(env: &[(String, String)], key: &str) -> Result<(), PluginError> {
    let _ = (env, key);
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_env_injected pending; slop: env injection assertion unchecked".into(),
    ))
}

// see: cans/plugins/vault.md#Plugin-vault
pub fn assert_disclosure(output: &[u8], secret: &[u8]) -> Result<(), PluginError> {
    let _ = (output, secret);
    Err(PluginError::Halt(
        "TODO (L3) [plugin_engine]: assert_disclosure pending; slop: leak-scan assertion unchecked"
            .into(),
    ))
}

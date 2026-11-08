// see: cans/02-plugin-model.md#Unified-verdict
// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::Verdict;

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_pass(verdict: &Verdict) -> Result<(), PluginError> {
    if matches!(verdict, Verdict::Pass) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_pass: verdict is not Pass: {verdict:?}"
        )))
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> {
    if matches!(verdict, Verdict::Drop { .. }) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_deny: verdict is not Drop: {verdict:?}"
        )))
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_patch(verdict: &Verdict) -> Result<(), PluginError> {
    if matches!(verdict, Verdict::Patch(_)) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_patch: verdict is not Patch: {verdict:?}"
        )))
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_suspend(verdict: &Verdict) -> Result<(), PluginError> {
    if matches!(verdict, Verdict::Suspend { .. }) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_suspend: verdict is not Suspend: {verdict:?}"
        )))
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub fn assert_abort(verdict: &Verdict) -> Result<(), PluginError> {
    if matches!(verdict, Verdict::Abort { .. }) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_abort: verdict is not Abort: {verdict:?}"
        )))
    }
}

// see: cans/02-plugin-model.md#Hook-return-semantics
pub fn assert_rewrite(input: &[u8], output: &[u8]) -> Result<(), PluginError> {
    if output != input {
        Ok(())
    } else {
        Err(PluginError::Halt(
            "assert_rewrite: output is byte-identical to input; no rewrite occurred".into(),
        ))
    }
}

pub fn assert_env_injected(env: &[(String, String)], key: &str) -> Result<(), PluginError> {
    if env.iter().any(|(k, _)| k == key) {
        Ok(())
    } else {
        Err(PluginError::Halt(format!(
            "assert_env_injected: key `{key}` is absent from the injected environment"
        )))
    }
}

// see: cans/plugins/vault.md#Plugin-vault
pub fn assert_disclosure(output: &[u8], secret: &[u8]) -> Result<(), PluginError> {
    if secret.is_empty() {
        return Err(PluginError::Halt(
            "assert_disclosure: the secret is empty; nothing to assert".into(),
        ));
    }
    let leaked = output.windows(secret.len()).any(|window| window == secret);
    if leaked {
        Err(PluginError::Halt(
            "assert_disclosure: the secret appears verbatim in the output".into(),
        ))
    } else {
        Ok(())
    }
}

// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::Verdict;

pub fn assert_rewrite(input: &[u8], output: &[u8]) -> Result<(), PluginError> {
    if input.is_empty() {
        return Err(PluginError::Halt("assert_rewrite: empty input".into()));
    }
    if output.is_empty() {
        return Err(PluginError::Halt("assert_rewrite: empty output".into()));
    }
    if output == input {
        return Err(PluginError::Halt(
            "assert_rewrite: output is byte-identical to input, no rewrite happened".into(),
        ));
    }
    Ok(())
}

pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> {
    if let Verdict::Drop { .. } = verdict {
        return Ok(());
    }
    if let Verdict::Abort { .. } = verdict {
        return Ok(());
    }
    // TODO (L3): assert_deny over a live execution receipt once the testkit runs plugins against the mock bus; verdict-shape check only at L2.
    Err(PluginError::Halt(
        "assert_deny: verdict does not deny the operation".into(),
    ))
}

pub fn assert_pass(verdict: &Verdict) -> Result<(), PluginError> {
    if let Verdict::Pass = verdict {
        return Ok(());
    }
    Err(PluginError::Halt(
        "assert_pass: verdict does not pass the operation".into(),
    ))
}

pub fn assert_env_injected(env: &[(String, String)], key: &str) -> Result<(), PluginError> {
    if env.iter().any(|(k, _)| k == key) {
        return Ok(());
    }
    Err(PluginError::Halt(format!(
        "assert_env_injected: key `{key}` absent from injected environment"
    )))
}

pub fn assert_disclosure(output: &[u8], secret: &[u8]) -> Result<(), PluginError> {
    if secret.is_empty() {
        return Ok(());
    }
    if output.windows(secret.len()).any(|w| w == secret) {
        return Err(PluginError::Halt(
            "assert_disclosure: secret bytes present in output".into(),
        ));
    }
    Ok(())
}

// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::errors::PluginError;
use kelir_interface::protocol::Verdict;

pub fn assert_rewrite(input: &[u8], output: &[u8]) -> Result<(), PluginError> {
    let _ = (input, output);
    // TODO: compare output against the expected rewrite of input.
    Err(PluginError::Halt("TODO: assert_rewrite".into()))
}

pub fn assert_deny(verdict: &Verdict) -> Result<(), PluginError> {
    let _ = verdict;
    // TODO: require the verdict to deny the operation.
    Err(PluginError::Halt("TODO: assert_deny".into()))
}

pub fn assert_env_injected(env: &[(String, String)], key: &str) -> Result<(), PluginError> {
    let _ = (env, key);
    // TODO: require key to be present in the injected environment.
    Err(PluginError::Halt("TODO: assert_env_injected".into()))
}

pub fn assert_disclosure(output: &[u8], secret: &[u8]) -> Result<(), PluginError> {
    let _ = (output, secret);
    // TODO: require secret to be absent from output.
    Err(PluginError::Halt("TODO: assert_disclosure".into()))
}

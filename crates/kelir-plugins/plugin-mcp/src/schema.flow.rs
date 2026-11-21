// see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
use kelir_interface::plugin_model::PluginError;
use serde_json::Value;

// see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithParamsPredicate {
    pub tool: String,
    pub schema: Vec<u8>,
}

impl WithParamsPredicate {
    // see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
    pub fn compile(tool: &str, schema: &[u8]) -> Result<Self, PluginError> {
        serde_json::from_slice::<Value>(schema).map_err(|err| {
            PluginError::Halt(format!(
                "EConfig: with_params schema for tool {tool} is not valid JSON: {err}"
            ))
        })?;
        Ok(Self {
            tool: tool.to_string(),
            schema: schema.to_vec(),
        })
    }

    // see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
    pub fn matches(&self, arguments: &[u8]) -> Result<bool, PluginError> {
        if serde_json::from_slice::<Value>(arguments).is_err() {
            return Ok(false);
        }
        let schema: Value = serde_json::from_slice(&self.schema).map_err(|err| {
            PluginError::Halt(format!(
                "EConfig: with_params schema for tool {} is not valid JSON: {err}",
                self.tool
            ))
        })?;
        if unconstrained(&schema) {
            return Ok(true);
        }
        // TODO: [governance_tools]: validate arguments against the compiled JSON schema; slop: tool calls run with unvalidated arguments; schema drift passes through. (deferred to L3)
        Err(PluginError::Halt(format!(
            "with_params schema validation pending: tool {}",
            self.tool
        )))
    }
}

// see: cans/plugins/mcp.md#Invariants-and-policy-(mcp)
fn unconstrained(schema: &Value) -> bool {
    let Some(object) = schema.as_object() else {
        return true;
    };
    !object.contains_key("properties") && !object.contains_key("required")
}

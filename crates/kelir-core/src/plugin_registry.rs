// see: cans/02-plugin-model.md#Catalog-tiering
use kelir_interface::plugin::PluginError;

// see: cans/02-plugin-model.md#Catalog-tiering
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRegistration {
    pub name: String,
    pub enabled: bool,
}

impl PluginRegistration {
    pub fn new(name: impl Into<String>, enabled: bool) -> Self {
        Self {
            name: name.into(),
            enabled,
        }
    }
}

// see: cans/02-plugin-model.md#Catalog-tiering
pub const KNOWN_CATALOG_PLUGINS: &[&str] = &[
    "chaos",
    "container",
    "db-proxy",
    "ledger",
    "mcp",
    "mesh",
    "profile",
    "replay",
    "supply-chain",
    "tool-guard",
    "transaction",
    "vault",
    "wire-ingress",
];

pub fn is_known_catalog_plugin(name: &str) -> bool {
    KNOWN_CATALOG_PLUGINS.contains(&name)
}

// see: cans/02-plugin-model.md#Catalog-tiering
pub fn resolve_plugin_registrations(
    registrations: &[PluginRegistration],
) -> Result<Vec<PluginRegistration>, PluginError> {
    let mut resolved: Vec<PluginRegistration> = Vec::new();
    for registration in registrations {
        if registration.name.trim().is_empty() {
            return Err(PluginError::Halt(
                "boot: plugin name must not be empty".to_string(),
            ));
        }
        if !registration.enabled {
            continue;
        }
        if !is_known_catalog_plugin(&registration.name) {
            return Err(PluginError::Halt(format!(
                "boot: unknown plugin `{}`: not in the kelir catalog, refusing to boot without it",
                registration.name
            )));
        }
        if resolved.iter().any(|item| item.name == registration.name) {
            continue;
        }
        resolved.push(registration.clone());
    }
    Ok(resolved)
}

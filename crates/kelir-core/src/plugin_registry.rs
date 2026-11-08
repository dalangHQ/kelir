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
        // TODO (L3): dynamic discovery resolves a registered name to its plugin entry point; the registry only validates shape and dedupes.
        if resolved.iter().any(|item| item.name == registration.name) {
            continue;
        }
        resolved.push(registration.clone());
    }
    Ok(resolved)
}

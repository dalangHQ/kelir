// see: cans/02-plugin-model.md#Catalog-tiering
use kelir_interface::plugin_model::{PluginError, PluginRegistration};

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
        // TODO (L3) [pe_02_cyclic_dependency_graph_abort]: dynamic discovery resolves a registered name to its plugin entry point; the registry only validates shape and dedupes; slop: cyclic graph boots Ok, recursion instead of abort
        if resolved.iter().any(|item| item.name == registration.name) {
            // see: cans/02-plugin-model.md#Static-dispatch
            tracing::warn!(
                "registry: duplicate plugin registration `{}` deduplicated; deepest config wins",
                registration.name
            );
            continue;
        }
        resolved.push(registration.clone());
    }
    Ok(resolved)
}

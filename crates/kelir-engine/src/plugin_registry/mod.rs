// see: cans/02-plugin-model.md#Catalog-tiering
#[path = "manifest.flow.rs"]
mod manifest;
#[path = "resolve.flow.rs"]
mod resolve;

pub use kelir_interface::plugin_model::PluginRegistration;
pub use manifest::{PluginManifest, PluginTier, parse_plugin_manifest};
pub use resolve::resolve_plugin_registrations;

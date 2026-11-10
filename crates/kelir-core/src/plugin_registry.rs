// see: cans/02-plugin-model.md#Catalog-tiering
use std::collections::BTreeMap;

use kelir_interface::plugin_model::PluginError;

pub use kelir_interface::plugin_model::PluginRegistration;

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

// see: cans/02-plugin-model.md#Plugin-tiering
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTier {
    pub crate_name: String,
    pub name: String,
    pub level: u8,
}

// see: cans/02-plugin-model.md#Auto-generated-manifest-contract
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub tier: PluginTier,
    pub deps: Vec<String>,
    pub dev_deps: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ManifestToml {
    package: PackageToml,
    #[serde(default)]
    dependencies: Option<BTreeMap<String, serde::de::IgnoredAny>>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: Option<BTreeMap<String, serde::de::IgnoredAny>>,
}

#[derive(serde::Deserialize)]
struct PackageToml {
    name: String,
    #[serde(default)]
    metadata: Option<MetadataToml>,
}

#[derive(serde::Deserialize)]
struct MetadataToml {
    kelir: KelirToml,
}

#[derive(serde::Deserialize)]
struct KelirToml {
    tier: TierToml,
}

#[derive(serde::Deserialize)]
struct TierToml {
    level: u8,
    name: String,
}

// see: cans/02-plugin-model.md#Plugin-tiering
pub fn parse_plugin_manifest(manifest: &str) -> Result<PluginManifest, PluginError> {
    let raw: ManifestToml = toml::from_str(manifest).map_err(|e| {
        PluginError::Halt(format!("registry: plugin manifest is not valid TOML: {e}"))
    })?;
    let Some(metadata) = raw.package.metadata else {
        return Err(PluginError::Halt(format!(
            "registry: plugin `{}` declares no [package.metadata.kelir.tier]",
            raw.package.name
        )));
    };
    Ok(PluginManifest {
        tier: PluginTier {
            crate_name: raw.package.name,
            name: metadata.kelir.tier.name,
            level: metadata.kelir.tier.level,
        },
        deps: raw
            .dependencies
            .map(|deps| deps.into_keys().collect())
            .unwrap_or_default(),
        dev_deps: raw
            .dev_dependencies
            .map(|deps| deps.into_keys().collect())
            .unwrap_or_default(),
    })
}

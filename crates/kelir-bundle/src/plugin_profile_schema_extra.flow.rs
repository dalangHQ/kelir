// see: cans/02-plugin-model.md#Facade-blindness
use serde::{Deserialize, Serialize};

use plugin_db_proxy::{DbProxyConfig, PgSchemaRewrite, PgWireRewrite, SqliteOverVsock};
use plugin_egress_proxy::EgressProxyConfig;
use plugin_l7_http::{DeclaredServiceConfig, L7HttpConfig};
use plugin_redaction::RedactionConfig;
use plugin_supply_chain::{PackageGuardConfig, SupplyChainConfig, WxJitConfig};
use plugin_vault::{CredentialDescriptor, CredentialKind, VaultConfig};

use super::PluginsProfile;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy.postgres.schema_rewrites
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PgSchemaRewriteProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy.postgres
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PgWireRewriteProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rewrites_per_conn: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ast_parser: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_where_on_mutations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_mode_in_tx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_unsafe_io: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_resolution: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthetic_cancel_keys: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_rewrites: Option<Vec<PgSchemaRewriteProfile>>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy.sqlite
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqliteProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guest_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_path: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.db_proxy
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbProxyPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postgres: Option<PgWireRewriteProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sqlite: Option<SqliteProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain.wx_jit
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WxJitProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_memfd: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_mprotect_exec: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_jit_binaries: Option<Vec<String>>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain.package_guard
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGuardProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_lifecycle_scripts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_lockfiles: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisories: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip_private_scopes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vet_skills: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.supply_chain
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wx_jit: Option<WxJitProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_guard: Option<PackageGuardProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultMetadataProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_vsock_port: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_ttl_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zeroize_on_drop: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_closed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credsub_fake_tokens: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrub_responses: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.credentials
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultCredentialProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_service: Option<VaultMetadataProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<VaultCredentialProfile>>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.l7_http.declared_services
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredServiceProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_direct_egress: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_dialect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dlp_redact_prompts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspect_sse_tool_use: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.l7_http
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct L7HttpPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threat_feeds: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_only_llm_retention: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_services: Option<Vec<DeclaredServiceProfile>>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.egress_proxy
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressProxyPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_tor: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_socks5_resolve: Option<bool>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.redaction.soft_delete
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftDeleteProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_dir: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.redaction
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_delete: Option<SoftDeleteProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/plugins/db-proxy.md#Plugin-db-proxy
pub fn to_db_proxy_config(plugins: &PluginsProfile) -> DbProxyConfig {
    let mut config = DbProxyConfig::default();
    if let Some(db_proxy) = plugins.db_proxy.as_ref() {
        if let Some(postgres) = db_proxy.postgres.as_ref() {
            let rewrite = PgWireRewrite {
                listen_port: postgres.listen_port.unwrap_or(5432),
                upstream: postgres.upstream.clone().unwrap_or_default(),
                schema_rewrites: postgres
                    .schema_rewrites
                    .as_ref()
                    .map(|rewrites| {
                        rewrites
                            .iter()
                            .map(|rewrite| PgSchemaRewrite {
                                from_schema: rewrite.from.clone().unwrap_or_default(),
                                to_schema: rewrite.to.clone().unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                max_rewrites_per_conn: postgres.max_rewrites_per_conn.unwrap_or(3),
                ast_parser: postgres
                    .ast_parser
                    .clone()
                    .unwrap_or_else(|| "libpgquery".to_string()),
                require_where_on_mutations: postgres.require_where_on_mutations.unwrap_or(false),
                deny_mode_in_tx: postgres
                    .deny_mode_in_tx
                    .clone()
                    .unwrap_or_else(|| "terminate".to_string()),
                block_unsafe_io: postgres.block_unsafe_io.unwrap_or(false),
                catalog_resolution: postgres.catalog_resolution.unwrap_or(false),
                synthetic_cancel_keys: postgres.synthetic_cancel_keys.unwrap_or(false),
            };
            config.postgres = rewrite;
        }
        if let Some(sqlite) = db_proxy.sqlite.as_ref() {
            config.sqlite = SqliteOverVsock {
                guest_socket: sqlite
                    .guest_socket
                    .clone()
                    .unwrap_or_else(|| "/run/kelir/db.sock".to_string()),
                host_path: sqlite.host_path.clone().unwrap_or_default(),
            };
        }
    }
    config
}

// see: cans/plugins/supply-chain.md#Plugin-supply-chain
pub fn to_supply_chain_config(plugins: &PluginsProfile) -> SupplyChainConfig {
    let mut config = SupplyChainConfig::default();
    if let Some(supply_chain) = plugins.supply_chain.as_ref() {
        if let Some(wx_jit) = supply_chain.wx_jit.as_ref() {
            // TODO: map allowed_jit_binaries once the profile carries BLAKE3 hashes for AuthorizedJitEngine (Track C).
            config.wx_jit = WxJitConfig {
                trap_memfd: wx_jit.trap_memfd.unwrap_or(false),
                trap_mprotect_exec: wx_jit.trap_mprotect_exec.unwrap_or(false),
                allowed_jit_binaries: Vec::new(),
            };
        }
        if let Some(guard) = supply_chain.package_guard.as_ref() {
            config.package_guard = PackageGuardConfig {
                deny_lifecycle_scripts: guard.deny_lifecycle_scripts.unwrap_or(false),
                managers: guard.managers.clone().unwrap_or_default(),
                bypass_runners: Vec::new(),
                resolve_lockfiles: guard.resolve_lockfiles.unwrap_or(false),
                advisories: guard.advisories.clone().unwrap_or_default(),
                strip_private_scopes: guard.strip_private_scopes.unwrap_or(false),
                vet_skills: guard.vet_skills.unwrap_or(false),
            };
        }
    }
    config
}

// see: cans/plugins/vault.md#Plugin-vault
pub fn to_vault_config(plugins: &PluginsProfile) -> VaultConfig {
    let mut config = VaultConfig {
        metadata_vsock_port: 80,
        token_ttl_seconds: 60,
        credentials: Vec::new(),
        credsub_fake_tokens: true,
        scrub_responses: true,
    };
    if let Some(vault) = plugins.vault.as_ref() {
        if let Some(metadata) = vault.metadata_service.as_ref() {
            if let Some(port) = metadata.listen_vsock_port {
                config.metadata_vsock_port = port;
            }
            if let Some(ttl) = metadata.token_ttl_seconds {
                config.token_ttl_seconds = ttl;
            }
            if let Some(fake) = metadata.credsub_fake_tokens {
                config.credsub_fake_tokens = fake;
            }
            if let Some(scrub) = metadata.scrub_responses {
                config.scrub_responses = scrub;
            }
        }
        if let Some(credentials) = vault.credentials.as_ref() {
            config.credentials = credentials
                .iter()
                .map(|credential| CredentialDescriptor {
                    name: credential.name.clone().unwrap_or_default(),
                    kind: match credential.kind.as_deref() {
                        Some("sigv4") => CredentialKind::SigV4,
                        Some("mtls_bundle") => CredentialKind::MtlsBundle,
                        _ => CredentialKind::BearerToken,
                    },
                    audience: credential.audience.clone().unwrap_or_default(),
                    placeholder: credential.placeholder.clone().unwrap_or_default(),
                })
                .collect();
        }
    }
    config
}

// see: cans/plugins/l7-http.md#Plugin-l7-http
pub fn to_l7_http_config(plugins: &PluginsProfile) -> L7HttpConfig {
    let mut config = L7HttpConfig::default();
    if let Some(l7_http) = plugins.l7_http.as_ref() {
        if let Some(feeds) = l7_http.threat_feeds.clone() {
            config.threat_feeds = feeds;
        }
        if let Some(hash_only) = l7_http.hash_only_llm_retention {
            config.hash_only_llm_retention = hash_only;
        }
        if let Some(services) = l7_http.declared_services.as_ref() {
            config.declared_services = services
                .iter()
                .map(|service| DeclaredServiceConfig {
                    name: service.name.clone().unwrap_or_default(),
                    env_var: service.env_var.clone().unwrap_or_default(),
                    upstream: service.upstream.clone().unwrap_or_default(),
                    block_direct_egress: service.block_direct_egress.unwrap_or(false),
                    llm_dialect: service.llm_dialect.clone().unwrap_or_default(),
                    dlp_redact_prompts: service.dlp_redact_prompts.unwrap_or(false),
                    inspect_sse_tool_use: service.inspect_sse_tool_use.unwrap_or(false),
                })
                .collect();
        }
    }
    config
}

// see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
pub fn to_egress_proxy_config(plugins: &PluginsProfile) -> EgressProxyConfig {
    let mut config = EgressProxyConfig::default();
    if let Some(egress_proxy) = plugins.egress_proxy.as_ref() {
        if let Some(block) = egress_proxy.block_tor {
            config.block_tor = block;
        }
        if let Some(deny) = egress_proxy.deny_socks5_resolve {
            config.deny_socks5_resolve = deny;
        }
    }
    config
}

// see: cans/plugins/redaction.md#Plugin-redaction
pub fn to_redaction_config(plugins: &PluginsProfile) -> RedactionConfig {
    let mut config = RedactionConfig::default();
    if let Some(redaction) = plugins.redaction.as_ref()
        && let Some(soft_delete) = redaction.soft_delete.as_ref()
    {
        if let Some(enabled) = soft_delete.enabled {
            config.soft_delete.enabled = enabled;
        }
        if let Some(trash_dir) = soft_delete.trash_dir.clone() {
            config.soft_delete.trash_dir = trash_dir;
        }
    }
    config
}

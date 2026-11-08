// see: cans/plugins/profile.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_profile.rs per cans/artifacts/config/project-structure.txt.
// TODO (L3): TOML/YAML profile parsers (no toml/serde_yaml in the workspace); JSON is the canonical interchange form.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder, StreamInterposer,
};
use kelir_interface::protocol::{StreamDirection, StreamKind, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ConfigurablePlugin: Send + Sync {
    fn configure(&mut self, fragment: &[u8]) -> Result<(), PluginError>;
}

// see: cans/plugins/profile.md
pub struct NamespaceRouter {
    namespaces: HashMap<String, Box<dyn ConfigurablePlugin>>,
}

impl NamespaceRouter {
    pub fn new() -> Self {
        Self {
            namespaces: HashMap::new(),
        }
    }

    pub fn register(&mut self, namespace: &str, plugin: Box<dyn ConfigurablePlugin>) {
        self.namespaces.insert(namespace.to_string(), plugin);
    }

    pub fn route(&mut self, namespace: &str, fragment: &[u8]) -> Result<(), PluginError> {
        match self.namespaces.get_mut(namespace) {
            Some(plugin) => plugin.configure(fragment),
            None => Err(PluginError::reject(
                "EConfig",
                format!("profile namespace `{namespace}` has no registered owner"),
            )),
        }
    }

    pub fn namespaces(&self) -> Vec<String> {
        self.namespaces.keys().cloned().collect()
    }
}

impl Default for NamespaceRouter {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/plugins/profile.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchOp {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

// see: cans/plugins/profile.md
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonPatch {
    pub op: PatchOp,
    pub path: String,
    pub from: Option<String>,
    pub value: Option<Value>,
}

impl JsonPatch {
    pub fn namespace(&self) -> Option<&str> {
        let path = self.path.strip_prefix('/')?;
        path.split('/').next()
    }

    pub fn validate(&self) -> Result<(), PluginError> {
        match self.namespace() {
            Some(namespace) if !namespace.is_empty() => {}
            _ => {
                return Err(PluginError::reject(
                    "EConfig",
                    format!(
                        "RFC 6902 patch path `{}` carries no namespace segment",
                        self.path
                    ),
                ));
            }
        }
        match self.op {
            PatchOp::Add | PatchOp::Replace | PatchOp::Test if self.value.is_none() => {
                return Err(PluginError::reject(
                    "EConfig",
                    format!(
                        "RFC 6902 {:?} patch at `{}` requires a value",
                        self.op, self.path
                    ),
                ));
            }
            PatchOp::Move | PatchOp::Copy if self.from.is_none() => {
                return Err(PluginError::reject(
                    "EConfig",
                    format!(
                        "RFC 6902 {:?} patch at `{}` requires a from path",
                        self.op, self.path
                    ),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

// see: cans/plugins/profile.md
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdiConfig {
    pub enabled: bool,
    pub spec_dir: String,
}

// see: cans/plugins/profile.md
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileConfig {
    pub rfc6902_dynamic_patching: bool,
    pub enforce_monotonic_patches: bool,
    pub cdi: Option<CdiConfig>,
}

// see: cans/plugins/profile.md
#[derive(Clone)]
pub struct ProfilePlugin {
    config: ProfileConfig,
    router: Arc<Mutex<NamespaceRouter>>,
}

impl ProfilePlugin {
    pub fn new() -> Self {
        Self::with_config(ProfileConfig::default())
    }

    pub fn with_config(config: ProfileConfig) -> Self {
        Self {
            config,
            router: Arc::new(Mutex::new(NamespaceRouter::new())),
        }
    }

    pub fn register_namespace(
        &self,
        namespace: &str,
        plugin: Box<dyn ConfigurablePlugin>,
    ) -> Result<(), PluginError> {
        let mut router = self
            .router
            .lock()
            .map_err(|_| PluginError::Halt("profile namespace router poisoned".into()))?;
        router.register(namespace, plugin);
        Ok(())
    }

    pub fn ingest_json(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = serde_json::from_str(document).map_err(|err| {
            PluginError::reject("EConfig", format!("profile JSON parse failed: {err}"))
        })?;
        let Some(document) = value.as_object() else {
            return Err(PluginError::reject(
                "EConfig",
                "profile document must be a JSON object keyed by namespace",
            ));
        };
        let mut routed = Vec::new();
        for (namespace, fragment) in document {
            let fragment = serde_json::to_vec(fragment).map_err(|err| {
                PluginError::reject(
                    "EConfig",
                    format!("profile fragment for `{namespace}` failed to serialize: {err}"),
                )
            })?;
            self.route(namespace, &fragment)?;
            routed.push(namespace.clone());
        }
        Ok(routed)
    }

    pub fn dispatch_patch(&self, payload: &[u8]) -> Result<Vec<String>, PluginError> {
        if !self.config.rfc6902_dynamic_patching {
            return Err(PluginError::reject(
                "EPERM",
                "RFC 6902 dynamic patching is disabled in the profile config",
            ));
        }
        let patches: Vec<JsonPatch> = serde_json::from_slice(payload).map_err(|err| {
            PluginError::reject(
                "EConfig",
                format!("PROFILE_SYNC patch payload failed to parse: {err}"),
            )
        })?;
        let mut routed = Vec::new();
        for patch in &patches {
            patch.validate()?;
            let Some(namespace) = patch.namespace() else {
                return Err(PluginError::reject(
                    "EConfig",
                    "RFC 6902 patch path carries no namespace segment",
                ));
            };
            // TODO (L3): monotonic patch validation against the active base document per cans/plugins/profile.md.
            let fragment = serde_json::to_vec(patch).map_err(|err| {
                PluginError::reject(
                    "EConfig",
                    format!("RFC 6902 patch for `{namespace}` failed to serialize: {err}"),
                )
            })?;
            self.route(namespace, &fragment)?;
            routed.push(namespace.to_string());
        }
        Ok(routed)
    }

    fn route(&self, namespace: &str, fragment: &[u8]) -> Result<(), PluginError> {
        let mut router = self
            .router
            .lock()
            .map_err(|_| PluginError::Halt("profile namespace router poisoned".into()))?;
        router.route(namespace, fragment)
    }
}

impl Default for ProfilePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ProfilePlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "profile"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let stage: Arc<dyn StreamInterposer> = Arc::new(self.clone());
        pipeline.bind_stream_scoped(StreamKind::ProfileSync, stage, self.default_scope());
    }
}

impl LifecycleParticipant for ProfilePlugin {}

#[async_trait]
impl StreamInterposer for ProfilePlugin {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, tags, ctx);
        Ok(Verdict::Pass)
    }

    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, ctx);
        match direction {
            StreamDirection::GuestToHost => match self.dispatch_patch(payload) {
                Ok(routed) => {
                    let _ = routed;
                    Ok(Verdict::Pass)
                }
                Err(err) => Ok(Verdict::Drop {
                    errno: None,
                    reason: err.to_string(),
                }),
            },
            StreamDirection::HostToGuest => {
                let _ = payload;
                Ok(Verdict::Pass)
            }
        }
    }

    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = (stream_id, ctx);
        Ok(())
    }
}

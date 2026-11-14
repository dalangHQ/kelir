// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router:
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    KelirPlugin, LifecycleParticipant, MountScope, NoDeps, PipelineBinder, PluginDeps,
    StreamFrameSink,
};
use kelir_interface::protocol_wire::StreamKind;
use serde_json::Value;

use crate::patch::{
    check_value_against_base, contains_number, pointer_namespace, remove_pointer, resolve_pointer,
    set_pointer,
};
use crate::{ConfigurablePlugin, JsonPatch, PatchOp, ProfileConfig};

// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router:
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
                Errno::Econfig,
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

// see: cans/plugins/profile.md#Plugin-profile
struct ProfileState {
    config: ProfileConfig,
    router: Mutex<NamespaceRouter>,
    bases: Mutex<HashMap<String, Value>>,
}

impl ProfileState {
    fn new(config: ProfileConfig) -> Self {
        Self {
            config,
            router: Mutex::new(NamespaceRouter::new()),
            bases: Mutex::new(HashMap::new()),
        }
    }

    fn register_namespace(
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

    fn config(&self) -> &ProfileConfig {
        &self.config
    }

    fn ingest_json(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = serde_json::from_str(document).map_err(|err| {
            PluginError::reject(Errno::Econfig, format!("profile JSON parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_toml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = toml::from_str(document).map_err(|err| {
            PluginError::reject(Errno::Econfig, format!("profile TOML parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_yaml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = serde_yaml::from_str(document).map_err(|err| {
            PluginError::reject(Errno::Econfig, format!("profile YAML parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_value(&self, value: Value) -> Result<Vec<String>, PluginError> {
        let Some(document) = value.as_object() else {
            return Err(PluginError::reject(
                Errno::Econfig,
                "profile document must be an object keyed by namespace",
            ));
        };
        let mut routed = Vec::new();
        for (namespace, fragment) in document {
            let fragment_bytes = serde_json::to_vec(fragment).map_err(|err| {
                PluginError::reject(
                    Errno::Econfig,
                    format!("profile fragment for `{namespace}` failed to serialize: {err}"),
                )
            })?;
            self.route(namespace, &fragment_bytes)?;
            let mut bases = self
                .bases
                .lock()
                .map_err(|_| PluginError::Halt("profile base document table poisoned".into()))?;
            bases.insert(namespace.clone(), fragment.clone());
            routed.push(namespace.clone());
        }
        Ok(routed)
    }

    fn dispatch_patch(&self, payload: &[u8]) -> Result<Vec<String>, PluginError> {
        if !self.config.rfc6902_dynamic_patching {
            return Err(PluginError::reject(
                Errno::Eperm,
                "RFC 6902 dynamic patching is disabled in the profile config",
            ));
        }
        let patches: Vec<JsonPatch> = serde_json::from_slice(payload).map_err(|err| {
            PluginError::reject(
                Errno::Econfig,
                format!("PROFILE_SYNC patch payload failed to parse: {err}"),
            )
        })?;
        let mut routed = Vec::new();
        for patch in &patches {
            patch.validate()?;
            let Some(namespace) = patch.namespace() else {
                return Err(PluginError::reject(
                    Errno::Econfig,
                    "RFC 6902 patch path carries no namespace segment",
                ));
            };
            self.check_monotonic(patch)?;
            let fragment = serde_json::to_vec(patch).map_err(|err| {
                PluginError::reject(
                    Errno::Econfig,
                    format!("RFC 6902 patch for `{namespace}` failed to serialize: {err}"),
                )
            })?;
            self.route(namespace, &fragment)?;
            self.apply_patch_to_base(patch);
            routed.push(namespace.to_string());
        }
        Ok(routed)
    }

    // see: cans/plugins/profile.md#Monotonic-limits
    fn check_monotonic(&self, patch: &JsonPatch) -> Result<(), PluginError> {
        let Some(namespace) = patch.namespace() else {
            return Ok(());
        };
        let remainder = patch.path[1 + namespace.len()..].to_string();
        let (base, from_base) = {
            let bases = self
                .bases
                .lock()
                .map_err(|_| PluginError::Halt("profile base document table poisoned".into()))?;
            let base = bases.get(namespace).cloned();
            let from_base = patch.from.as_ref().and_then(|from| {
                let from_namespace = pointer_namespace(from)?;
                let from_remainder = &from[1 + from_namespace.len()..];
                let document = bases.get(from_namespace)?;
                resolve_pointer(document, from_remainder).cloned()
            });
            (base, from_base)
        };
        match patch.op {
            PatchOp::Add | PatchOp::Replace => {
                let Some(value) = &patch.value else {
                    return Ok(());
                };
                match &base {
                    None => check_value_against_base(None, value, &patch.path),
                    Some(document) => {
                        let base_at = resolve_pointer(document, &remainder);
                        check_value_against_base(base_at, value, &patch.path)
                    }
                }
            }
            PatchOp::Remove => {
                let Some(document) = &base else {
                    return Ok(());
                };
                match resolve_pointer(document, &remainder) {
                    None => Ok(()),
                    Some(target) => {
                        if contains_number(target) {
                            return Err(PluginError::reject(
                                Errno::Esecurityviolation,
                                format!(
                                    "RFC 6902 patch at `{}` removes a numeric limit",
                                    patch.path
                                ),
                            ));
                        }
                        Ok(())
                    }
                }
            }
            PatchOp::Move | PatchOp::Copy => {
                let Some(value) = from_base else {
                    return Ok(());
                };
                match &base {
                    None => check_value_against_base(None, &value, &patch.path),
                    Some(document) => {
                        let base_at = resolve_pointer(document, &remainder);
                        check_value_against_base(base_at, &value, &patch.path)
                    }
                }
            }
            PatchOp::Test => Ok(()),
        }
    }

    // see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
    fn apply_patch_to_base(&self, patch: &JsonPatch) {
        let Some(namespace) = patch.namespace() else {
            return;
        };
        let remainder = patch.path[1 + namespace.len()..].to_string();
        let Ok(mut bases) = self.bases.lock() else {
            return;
        };
        let Some(document) = bases.get_mut(namespace) else {
            return;
        };
        match patch.op {
            PatchOp::Add | PatchOp::Replace => {
                if let Some(value) = patch.value.clone() {
                    set_pointer(document, &remainder, value);
                }
            }
            PatchOp::Remove => {
                remove_pointer(document, &remainder);
            }
            // TODO (L3) [governance_vault]: Move/Copy leave the stored base stale (Test is read-only); cross-path base update needs owner-state verification; see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching; slop: stale base after Move/Copy, cross-path update without owner verification
            PatchOp::Move | PatchOp::Copy | PatchOp::Test => {}
        }
    }

    fn route(&self, namespace: &str, fragment: &[u8]) -> Result<(), PluginError> {
        let mut router = self
            .router
            .lock()
            .map_err(|_| PluginError::Halt("profile namespace router poisoned".into()))?;
        router.route(namespace, fragment)
    }
}

// see: cans/plugins/profile.md#Plugin-profile
#[derive(Clone)]
pub struct ProfilePlugin {
    state: Arc<ProfileState>,
}

impl ProfilePlugin {
    pub fn new() -> Self {
        Self::with_config(ProfileConfig::default())
    }

    pub fn with_config(config: ProfileConfig) -> Self {
        Self {
            state: Arc::new(ProfileState::new(config)),
        }
    }

    pub fn register_namespace(
        &self,
        namespace: &str,
        plugin: Box<dyn ConfigurablePlugin>,
    ) -> Result<(), PluginError> {
        self.state.register_namespace(namespace, plugin)
    }

    pub fn ingest_json(&self, document: &str) -> Result<Vec<String>, PluginError> {
        self.state.ingest_json(document)
    }

    pub fn ingest_toml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        self.state.ingest_toml(document)
    }

    pub fn ingest_yaml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        self.state.ingest_yaml(document)
    }

    pub fn dispatch_patch(&self, payload: &[u8]) -> Result<Vec<String>, PluginError> {
        self.state.dispatch_patch(payload)
    }
}

impl Default for ProfilePlugin {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/plugins/profile.md#Registers-stream-kind-PROFILE_SYNC-on-the-mux
// see: cans/11-protocol.md#Kind-registration
#[derive(Clone)]
pub struct ProfileCtx {
    state: Arc<ProfileState>,
}

impl ProfileCtx {
    // see: cans/11-protocol.md#Kind-registration
    pub fn dispatch_profile_sync(&self, payload: &[u8]) -> Result<Vec<String>, PluginError> {
        self.state.dispatch_patch(payload)
    }

    // see: cans/plugins/profile.md#Inversion-of-Control-configuration-router:
    pub fn register_namespace(
        &self,
        namespace: &str,
        plugin: Box<dyn ConfigurablePlugin>,
    ) -> Result<(), PluginError> {
        self.state.register_namespace(namespace, plugin)
    }

    pub fn config(&self) -> &ProfileConfig {
        self.state.config()
    }
}

// see: cans/plugins/profile.md#Registers-stream-kind-PROFILE_SYNC-on-the-mux
// see: cans/11-protocol.md#Kind-registration
struct ProfileCtxStreamSink {
    ctx: ProfileCtx,
}

// see: cans/11-protocol.md#Kind-registration
impl StreamFrameSink for ProfileCtxStreamSink {
    fn on_stream_frame(
        &self,
        _stream_id: u32,
        kind: u16,
        _flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError> {
        if kind != StreamKind::ProfileSync as u16 {
            return Ok(());
        }
        self.ctx.dispatch_profile_sync(payload)?;
        Ok(())
    }
}

impl KelirPlugin for ProfilePlugin {
    type Context = ProfileCtx;
    fn context(&self) -> ProfileCtx {
        ProfileCtx {
            state: Arc::clone(&self.state),
        }
    }
    fn name(&self) -> &'static str {
        "profile"
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/11-protocol.md#Kind-registration
        let sink: Arc<dyn StreamFrameSink> = Arc::new(ProfileCtxStreamSink {
            ctx: self.context(),
        });
        pipeline.bind_stream_kind_scoped(
            StreamKind::ProfileSync as u16,
            sink,
            MountScope::default().with_stream_kinds([StreamKind::ProfileSync]),
        );
    }
}

// see: cans/plugins/profile.md#Plugin-profile
impl PluginDeps for ProfilePlugin {
    type Deps = NoDeps;
}

impl LifecycleParticipant for ProfilePlugin {}

// see: cans/plugins/profile.md#Plugin-profile
#![forbid(unsafe_code)]

// TODO (L3): tests/test_profile.rs per cans/artifacts/config/project-structure.txt.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ConfigurablePlugin: Send + Sync {
    fn configure(&mut self, fragment: &[u8]) -> Result<(), PluginError>;
}

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

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
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

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
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

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn pointer_namespace(pointer: &str) -> Option<&str> {
    let path = pointer.strip_prefix('/')?;
    path.split('/').next()
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn unescape_pointer_segment(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn resolve_pointer<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for raw in pointer.split('/').skip(1) {
        let segment = unescape_pointer_segment(raw);
        match current {
            Value::Object(map) => {
                current = map.get(&segment)?;
            }
            Value::Array(items) => {
                let index: usize = segment.parse().ok()?;
                current = items.get(index)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

// see: cans/plugins/profile.md#Monotonic-limits
fn contains_number(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::Object(map) => map.values().any(contains_number),
        Value::Array(items) => items.iter().any(contains_number),
        _ => false,
    }
}

// see: cans/plugins/profile.md#Monotonic-limits
fn check_value_against_base(
    base: Option<&Value>,
    value: &Value,
    path: &str,
) -> Result<(), PluginError> {
    if matches!(base, Some(Value::Number(_))) && !value.is_number() {
        return Err(PluginError::reject(
            "ESecurityViolation",
            format!("RFC 6902 patch at `{path}` replaces a numeric limit with a non-numeric value"),
        ));
    }
    match value {
        Value::Number(number) => {
            let Some(base_value) = base else {
                return Ok(());
            };
            let Some(base_number) = base_value.as_f64() else {
                return Ok(());
            };
            let Some(new_number) = number.as_f64() else {
                return Ok(());
            };
            if new_number > base_number {
                return Err(PluginError::reject(
                    "ESecurityViolation",
                    format!(
                        "RFC 6902 patch at `{path}` widens a monotonic limit from {base_number} to {new_number}"
                    ),
                ));
            }
            Ok(())
        }
        Value::Object(map) => {
            let Some(Value::Object(base_map)) = base else {
                return Ok(());
            };
            for (key, child) in map {
                let child_path = format!("{path}/{key}");
                check_value_against_base(base_map.get(key), child, &child_path)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            let Some(Value::Array(base_items)) = base else {
                return Ok(());
            };
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}/{index}");
                check_value_against_base(base_items.get(index), child, &child_path)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn set_pointer(base: &mut Value, pointer: &str, value: Value) -> bool {
    if pointer.is_empty() {
        *base = value;
        return true;
    }
    let mut segments: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(unescape_pointer_segment)
        .collect();
    let Some(last) = segments.pop() else {
        return false;
    };
    let mut current = base;
    for segment in &segments {
        match current {
            Value::Object(map) => {
                current = map
                    .entry(segment.clone())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
            }
            Value::Array(items) => {
                if segment == "-" {
                    items.push(Value::Object(serde_json::Map::new()));
                    let Some(next) = items.last_mut() else {
                        return false;
                    };
                    current = next;
                } else {
                    let Ok(index) = segment.parse::<usize>() else {
                        return false;
                    };
                    let Some(next) = items.get_mut(index) else {
                        return false;
                    };
                    current = next;
                }
            }
            _ => return false,
        }
    }
    match current {
        Value::Object(map) => {
            map.insert(last, value);
            true
        }
        Value::Array(items) => {
            if last == "-" {
                items.push(value);
                true
            } else {
                let Ok(index) = last.parse::<usize>() else {
                    return false;
                };
                if index > items.len() {
                    return false;
                }
                items.insert(index, value);
                true
            }
        }
        _ => false,
    }
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn remove_pointer(base: &mut Value, pointer: &str) -> bool {
    if pointer.is_empty() {
        *base = Value::Null;
        return true;
    }
    let mut segments: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(unescape_pointer_segment)
        .collect();
    let Some(last) = segments.pop() else {
        return false;
    };
    let mut current = base;
    for segment in &segments {
        match current {
            Value::Object(map) => {
                let Some(next) = map.get_mut(segment) else {
                    return false;
                };
                current = next;
            }
            Value::Array(items) => {
                let Ok(index) = segment.parse::<usize>() else {
                    return false;
                };
                let Some(next) = items.get_mut(index) else {
                    return false;
                };
                current = next;
            }
            _ => return false,
        }
    }
    match current {
        Value::Object(map) => map.remove(&last).is_some(),
        Value::Array(items) => {
            let Ok(index) = last.parse::<usize>() else {
                return false;
            };
            if index >= items.len() {
                return false;
            }
            items.remove(index);
            true
        }
        _ => false,
    }
}

// see: cans/plugins/profile.md#Container-Device-Interface-(CDI)-device-mapping
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdiConfig {
    pub enabled: bool,
    pub spec_dir: String,
}

// see: cans/plugins/profile.md#Plugin-profile
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileConfig {
    pub rfc6902_dynamic_patching: bool,
    pub enforce_monotonic_patches: bool,
    pub cdi: Option<CdiConfig>,
}

// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router:
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
            PluginError::reject("EConfig", format!("profile JSON parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_toml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = toml::from_str(document).map_err(|err| {
            PluginError::reject("EConfig", format!("profile TOML parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_yaml(&self, document: &str) -> Result<Vec<String>, PluginError> {
        let value: Value = serde_yaml::from_str(document).map_err(|err| {
            PluginError::reject("EConfig", format!("profile YAML parse failed: {err}"))
        })?;
        self.ingest_value(value)
    }

    fn ingest_value(&self, value: Value) -> Result<Vec<String>, PluginError> {
        let Some(document) = value.as_object() else {
            return Err(PluginError::reject(
                "EConfig",
                "profile document must be an object keyed by namespace",
            ));
        };
        let mut routed = Vec::new();
        for (namespace, fragment) in document {
            let fragment_bytes = serde_json::to_vec(fragment).map_err(|err| {
                PluginError::reject(
                    "EConfig",
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
            self.check_monotonic(patch)?;
            let fragment = serde_json::to_vec(patch).map_err(|err| {
                PluginError::reject(
                    "EConfig",
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
                                "ESecurityViolation",
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
            // TODO (L3): Move/Copy leave the stored base stale; cross-path base update needs owner-state verification per cans/plugins/profile.md.
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
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for ProfilePlugin {}

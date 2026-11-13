// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use serde_json::Value;

use crate::{JsonPatch, PatchOp};

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
                    Errno::Econfig,
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
                    Errno::Econfig,
                    format!(
                        "RFC 6902 {:?} patch at `{}` requires a value",
                        self.op, self.path
                    ),
                ));
            }
            PatchOp::Move | PatchOp::Copy if self.from.is_none() => {
                return Err(PluginError::reject(
                    Errno::Econfig,
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
pub(crate) fn pointer_namespace(pointer: &str) -> Option<&str> {
    let path = pointer.strip_prefix('/')?;
    path.split('/').next()
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
fn unescape_pointer_segment(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

// see: cans/plugins/profile.md#RFC-6902-dynamic-policy-patching
pub(crate) fn resolve_pointer<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
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
pub(crate) fn contains_number(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::Object(map) => map.values().any(contains_number),
        Value::Array(items) => items.iter().any(contains_number),
        _ => false,
    }
}

// see: cans/plugins/profile.md#Monotonic-limits
pub(crate) fn check_value_against_base(
    base: Option<&Value>,
    value: &Value,
    path: &str,
) -> Result<(), PluginError> {
    if matches!(base, Some(Value::Number(_))) && !value.is_number() {
        return Err(PluginError::reject(
            Errno::Esecurityviolation,
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
                    Errno::Esecurityviolation,
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
pub(crate) fn set_pointer(base: &mut Value, pointer: &str, value: Value) -> bool {
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
pub(crate) fn remove_pointer(base: &mut Value, pointer: &str) -> bool {
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

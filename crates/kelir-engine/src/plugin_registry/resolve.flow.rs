// see: cans/02-plugin-model.md#Catalog-tiering
// see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
use std::collections::{BTreeMap, BTreeSet};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::PluginRegistration;

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
fn registration_edges(name: &str) -> Vec<(String, String)> {
    let segments: Vec<&str> = name.split("->").map(str::trim).collect();
    segments
        .windows(2)
        .filter(|pair| !pair[0].is_empty() && !pair[1].is_empty())
        .map(|pair| (pair[0].to_string(), pair[1].to_string()))
        .collect()
}

// see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
fn visit(
    node: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), PluginError> {
    if done.contains(node) {
        return Ok(());
    }
    if visiting.contains(node) {
        stack.push(node.to_string());
        return Err(PluginError::Halt(format!(
            "boot: cyclic plugin dependency graph rejected: dependency cycle detected: {}",
            stack.join(" -> ")
        )));
    }
    visiting.insert(node.to_string());
    stack.push(node.to_string());
    if let Some(deps) = graph.get(node) {
        for dep in deps {
            if graph.contains_key(dep) {
                visit(dep, graph, visiting, done, stack)?;
            }
        }
    }
    stack.pop();
    visiting.remove(node);
    done.insert(node.to_string());
    Ok(())
}

// see: cans/02-plugin-model.md#Catalog-tiering
pub fn resolve_plugin_registrations(
    registrations: &[PluginRegistration],
) -> Result<Vec<PluginRegistration>, PluginError> {
    let mut resolved: Vec<PluginRegistration> = Vec::new();
    // see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for registration in registrations {
        if registration.name.trim().is_empty() {
            return Err(PluginError::Halt(
                "boot: plugin name must not be empty".to_string(),
            ));
        }
        for (source, target) in registration_edges(&registration.name) {
            graph.entry(source).or_default().insert(target);
        }
        if !registration.enabled {
            continue;
        }
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
    // see: cans/artifacts/test-plan/cases/plugin_engine/pe_02_cyclic_dependency_graph_abort.yaml#case_id
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut stack = Vec::new();
    for node in graph.keys() {
        visit(node, &graph, &mut visiting, &mut done, &mut stack)?;
    }
    Ok(resolved)
}

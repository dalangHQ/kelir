// see: cans/15-operations.md#Coverage-gates
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
struct PluginManifestToml {
    package: PluginPackageToml,
    #[serde(default)]
    dependencies: BTreeMap<String, DepSpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, DepSpec>,
}

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
struct PluginPackageToml {
    name: String,
}

// see: cans/15-operations.md#Coverage-gates
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DepSpec {
    Table {
        path: Option<String>,
    },
    #[expect(dead_code)]
    Version(String),
}

// see: cans/15-operations.md#Coverage-gates
fn parse_manifest(text: &str) -> Result<PluginManifestToml, Box<dyn std::error::Error>> {
    toml::from_str(text).map_err(|e| format!("graph: manifest is not valid TOML: {e}").into())
}

// see: cans/15-operations.md#Coverage-gates
fn path_deps(manifest: &PluginManifestToml) -> Vec<String> {
    let mut deps = Vec::new();
    for table in [&manifest.dependencies, &manifest.dev_dependencies] {
        for (name, spec) in table {
            if matches!(spec, DepSpec::Table { path: Some(_) }) {
                deps.push(name.clone());
            }
        }
    }
    deps
}

pub fn plugin_graph() -> Result<HashMap<String, Vec<String>>, Box<dyn std::error::Error>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for entry in fs::read_dir(Path::new("crates/kelir-plugins"))? {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        let text = fs::read_to_string(&manifest)?;
        let parsed = parse_manifest(&text)?;
        let deps = path_deps(&parsed);
        graph.insert(parsed.package.name, deps);
    }
    Ok(graph)
}

pub(crate) fn visit(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
    done: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if done.contains(node) {
        return Ok(());
    }
    if visiting.contains(node) {
        stack.push(node.to_string());
        return Err(format!("graph: dependency cycle detected: {}", stack.join(" -> ")).into());
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

pub(crate) fn assert_acyclic(
    graph: &HashMap<String, Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut visiting = HashSet::new();
    let mut done = HashSet::new();
    let mut stack = Vec::new();
    for name in graph.keys() {
        visit(name, graph, &mut visiting, &mut done, &mut stack)?;
    }
    Ok(())
}

pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    let graph = plugin_graph()?;
    for (name, deps) in &graph {
        for dep in deps {
            if dep == "kelir-core" || dep == "kelir-drivers" {
                return Err(
                    format!("graph: plugin `{name}` depends on forbidden crate `{dep}`").into(),
                );
            }
        }
    }
    assert_acyclic(&graph)
}

// see: cans/15-operations.md#Coverage-gates
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

fn manifest_name(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("name = "))
        .map(|value| value.trim().trim_matches('"').to_string())
}

fn path_deps(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            let name = name.trim();
            if rest.contains("path = ") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn visit(
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

pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for entry in fs::read_dir(Path::new("crates/kelir-plugins"))? {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        let text = fs::read_to_string(&manifest)?;
        let Some(name) = manifest_name(&text) else {
            continue;
        };
        graph.insert(name, path_deps(&text));
    }
    for (name, deps) in &graph {
        for dep in deps {
            if dep == "kelir-core" || dep == "kelir-drivers" {
                return Err(
                    format!("graph: plugin `{name}` depends on forbidden crate `{dep}`").into(),
                );
            }
        }
    }
    let mut visiting = HashSet::new();
    let mut done = HashSet::new();
    let mut stack = Vec::new();
    for name in graph.keys() {
        visit(name, &graph, &mut visiting, &mut done, &mut stack)?;
    }
    Ok(())
}

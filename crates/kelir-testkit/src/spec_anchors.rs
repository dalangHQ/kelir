// see: cans/_rules.yaml#slug
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// see: cans/artifacts/test-plan/manifest.yaml#plan
pub const INDEX_SCHEMA: &str = "kelir-testkit/anchors/v1";

// see: cans/_rules.yaml#slug
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AnchorIndex {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub files: BTreeMap<String, Vec<String>>,
}

// see: cans/_rules.yaml#slug
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if is_separator(ch) {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else {
            out.push(ch);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

// see: cans/_rules.yaml#slug
pub fn norm(text: &str) -> String {
    slugify(text).to_lowercase()
}

// see: cans/_rules.yaml#slug
fn is_separator(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '-' | '_' | '\u{2010}'
                ..='\u{2015}' | '\u{2043}' | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}'
        )
}

// see: cans/_rules.yaml#slug
fn strip_pointer_suffix(text: &str) -> String {
    let Some(see_pos) = text.find("see:") else {
        return text.to_string();
    };
    let after = &text[see_pos + "see:".len()..];
    let leading_ws = after.len() - after.trim_start().len();
    if leading_ws == 0 {
        return text.to_string();
    }
    let Some(token) = after.split_whitespace().next() else {
        return text.to_string();
    };
    let is_pointer = token.contains('#')
        || token.contains(".md")
        || token.starts_with('[')
        || token.starts_with("artifacts/");
    if !is_pointer {
        return text.to_string();
    }
    text[..see_pos]
        .trim_end_matches([' ', ':', '('])
        .trim()
        .to_string()
}

// see: cans/_rules.yaml#slug
fn bullet_title(line: &str) -> Option<String> {
    let after_dash = line.trim_start().strip_prefix('-')?;
    if !after_dash.starts_with(char::is_whitespace) {
        return None;
    }
    let title = after_dash.trim();
    if title.is_empty() {
        return None;
    }
    Some(strip_pointer_suffix(title))
}

// see: cans/_rules.yaml#slug
fn heading_title(line: &str) -> Option<String> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let body = rest.trim();
    if body.is_empty() {
        return None;
    }
    let core = body.trim_end_matches('#').trim_end();
    if core.is_empty() {
        return None;
    }
    Some(strip_pointer_suffix(core))
}

// see: cans/_rules.yaml#slug
pub fn anchor_titles(markdown: &str) -> Vec<String> {
    let mut titles = Vec::new();
    let mut fenced = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(title) = bullet_title(line).or_else(|| heading_title(line)) {
            titles.push(title);
        }
    }
    titles
}

// see: cans/_rules.yaml#slug
fn corpus_files(repo_root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for dir in ["cans", "cans/plugins"] {
        for entry in fs::read_dir(repo_root.join(dir))? {
            let path = entry?.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                out.push(path);
            }
        }
    }
    Ok(out)
}

// see: cans/_rules.yaml#slug
pub fn build_index(repo_root: &Path) -> Result<AnchorIndex, Box<dyn std::error::Error>> {
    let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in corpus_files(repo_root)? {
        let rel = path
            .strip_prefix(repo_root)
            .map_err(|err| format!("{} not under repo root: {err}", path.display()))?
            .to_string_lossy()
            .to_string();
        let raw = fs::read_to_string(&path).map_err(|err| format!("cannot read {rel}: {err}"))?;
        let mut anchors: Vec<String> = anchor_titles(&raw)
            .iter()
            .map(|title| slugify(title))
            .filter(|slug| !slug.is_empty())
            .collect();
        anchors.sort();
        anchors.dedup();
        files.insert(rel, anchors);
    }
    Ok(AnchorIndex {
        schema: INDEX_SCHEMA.to_string(),
        files,
    })
}

// see: cans/_rules.yaml#slug
pub fn render_index(index: &AnchorIndex) -> Result<String, serde_json::Error> {
    let mut rendered = serde_json::to_string_pretty(index)?;
    rendered.push('\n');
    Ok(rendered)
}

// see: cans/_rules.yaml#slug
pub fn parse_index(raw: &str) -> Result<AnchorIndex, serde_json::Error> {
    serde_json::from_str(raw)
}

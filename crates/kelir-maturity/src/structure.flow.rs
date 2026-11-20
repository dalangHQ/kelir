// see: cans/01-architecture.md#Codebase-invariants
use crate::tree::{is_tooling_file, line_has_guest_io};
use crate::{CheckReport, ForbidMode, Level, Policy, crate_of, level_of, read_allowed_levels};
use std::fs;
use std::path::Path;

pub fn check_file(path: &Path, policy: &Policy, report: &mut CheckReport) {
    let Ok(src) = fs::read_to_string(path) else {
        return;
    };
    let level = level_of(path);
    let tooling = is_tooling_file(path);
    let lines = src.lines().count();
    let at = path.display().to_string();
    if level == Level::L1 && has_runtime_concern(&src) {
        record(
            report,
            policy.forbid_mode,
            "S1",
            &at,
            "L1 file carries runtime concern (async fn/.await/tokio::/spawn); move to <stem>.flow.rs",
        );
    }
    if is_aggregator(path) && lines > 120 && has_mutating_impl(&src) {
        record(
            report,
            policy.forbid_mode,
            "S2",
            &format!("{at}:{lines}"),
            "lib.rs/mod.rs is not an aggregator; extract the impl to <Type>.flow.rs",
        );
    }
    if level == Level::L2 && !tooling && spawns_runtime_outside_block_on(&src) {
        record(
            report,
            policy.forbid_mode,
            "S3",
            &at,
            "L2 file spawns a runtime; move to <stem>.run.rs (requires allowed_levels += L3)",
        );
    }
    if level == Level::L2
        && !tooling
        && crate_allows_l3(&crate_of(path))
        && has_runtime_concern(&src)
        && has_io_concern(&src)
    {
        record(
            report,
            policy.forbid_mode,
            "S4",
            &at,
            ".flow.rs carries I/O (std::fs/tokio::fs/libc/std::process) inside an L3-allowed crate; move to <stem>.run.rs",
        );
    }
    if is_aggregator(path) && src.contains("async fn") && has_mutating_impl(&src) {
        record(
            report,
            policy.forbid_mode,
            "S5",
            &at,
            "lib.rs/mod.rs carries async impl bodies; extract to <impl>.flow.rs",
        );
    }
    if level == Level::L3 {
        for (name, line_no) in stub_shape_violations(&src) {
            record(
                report,
                policy.forbid_mode,
                "S6",
                &format!("{at}:{line_no}"),
                &format!(
                    "fn {name} in .run.rs has a non-stub body; L3 files hold only signatures and L3 deferral markers"
                ),
            );
        }
    }
}

fn record(report: &mut CheckReport, mode: ForbidMode, rule: &str, at: &str, detail: &str) {
    if mode == ForbidMode::Error {
        report
            .fails
            .push(format!("{at}: [structure:{rule}] {detail}"));
    } else {
        report.warns.push(format!(
            "{at}: {detail} (structure:{rule}, warn-mode rollout)"
        ));
    }
}

fn runtime_spawn_present(text: &str) -> bool {
    text.contains(concat!("std::thread::", "spawn"))
        || text.contains(concat!("#[tokio", "::main]"))
        || text.contains(concat!("tokio", "::runtime::Runtime::new"))
        || text.contains(concat!("tokio", "::runtime::Builder"))
}

fn spawns_runtime_outside_block_on(src: &str) -> bool {
    let mut stripped = src.to_string();
    let mut cursor = 0;
    for item in fn_items(src) {
        if item.name != "block_on" {
            continue;
        }
        let Some(body) = item.body else {
            continue;
        };
        if let Some(off) = stripped[cursor..].find(body).map(|o| o + cursor) {
            stripped.replace_range(off..off + body.len(), &" ".repeat(body.len()));
            cursor = off + body.len();
        }
    }
    runtime_spawn_present(&stripped)
}

fn has_runtime_concern(src: &str) -> bool {
    src.contains("async fn")
        || src.contains(".await")
        || src.contains(concat!("tokio", "::"))
        || src.contains(concat!("std::thread::", "spawn"))
        || src.contains(concat!("#[tokio", "::main]"))
        || src.contains(concat!("#[tokio", "::test]"))
}

fn has_io_concern(src: &str) -> bool {
    src.lines().any(line_has_guest_io)
}

fn has_mutating_impl(src: &str) -> bool {
    src.split("impl").skip(1).any(|block| {
        let (_, body) = block.split_once('{').unwrap_or(("", ""));
        body.contains("&mut self")
    })
}

fn is_aggregator(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("lib.rs" | "mod.rs")
    )
}

fn crate_allows_l3(crate_dir: &Path) -> bool {
    read_allowed_levels(&crate_dir.join("Cargo.toml")).contains(&Level::L3)
}

// see: cans/01-architecture.md#Codebase-invariants
const S6_TODO_MARK: &str = concat!("/", "/ ", "TODO (L3)");

struct FnItem<'a> {
    name: String,
    line: usize,
    body: Option<&'a str>,
}

fn stub_shape_violations(src: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for item in fn_items(src) {
        let Some(body) = item.body else {
            continue;
        };
        if is_stub_fn(&item.name, body) {
            continue;
        }
        out.push((item.name, item.line));
    }
    out
}

fn is_stub_fn(name: &str, body: &str) -> bool {
    if body.contains(S6_TODO_MARK) {
        return true;
    }
    let text = scrub(body);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    if is_single_expression(&text) {
        return true;
    }
    if is_ctor(name) && is_field_init_only(&text) {
        return true;
    }
    false
}

fn is_ctor(name: &str) -> bool {
    name == "new" || name.starts_with("build_")
}

fn is_single_expression(text: &str) -> bool {
    !text.contains(';')
        && !has_word(text, "let")
        && !has_word(text, "if")
        && !has_word(text, "match")
        && !has_word(text, "for")
        && !has_word(text, "while")
        && !has_word(text, "loop")
        && !has_word(text, "return")
        && !has_word(text, "await")
        && !has_word(text, "unsafe")
        && !has_word(text, "fn")
        && !text.contains("=>")
}

fn is_field_init_only(text: &str) -> bool {
    if has_word(text, "if")
        || has_word(text, "match")
        || has_word(text, "for")
        || has_word(text, "while")
        || has_word(text, "loop")
        || has_word(text, "return")
        || has_word(text, "await")
        || has_word(text, "unsafe")
        || has_word(text, "fn")
        || text.contains("=>")
    {
        return false;
    }
    let parts = split_top_level(text, b';');
    let mut segments: Vec<&str> = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return true;
    }
    let last = segments.pop().unwrap_or("");
    if segments.is_empty() {
        return true;
    }
    segments.iter().all(|s| s.starts_with("let ")) && !last.is_empty()
}

fn has_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + word.len() <= bytes.len() {
        if text[i..].starts_with(word) {
            let before = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after = i + word.len() >= bytes.len() || !is_ident_byte(bytes[i + word.len()]);
            if before && after {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn split_top_level(text: &str, sep: u8) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => depth = depth.saturating_sub(1),
            b if b == sep && depth == 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&text[start..]);
    out
}

fn fn_items(src: &str) -> Vec<FnItem<'_>> {
    let clean = scrub(src);
    let bytes = clean.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= bytes.len() {
        if clean[i..].starts_with("fn")
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
            && (i + 2 >= bytes.len() || bytes[i + 2].is_ascii_whitespace())
        {
            let line = 1 + clean[..i].bytes().filter(|b| *b == b'\n').count();
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let name_start = j;
            while j < bytes.len() && is_ident_byte(bytes[j]) {
                j += 1;
            }
            if j == name_start {
                i += 2;
                continue;
            }
            let name = clean[name_start..j].to_string();
            let mut k = j;
            let mut depth = 0;
            let mut body: Option<&str> = None;
            while k < bytes.len() {
                match bytes[k] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    b';' if depth == 0 => break,
                    b'{' if depth == 0 => {
                        let mut d = 1;
                        let mut m = k + 1;
                        while m < bytes.len() && d > 0 {
                            match bytes[m] {
                                b'{' => d += 1,
                                b'}' => d -= 1,
                                _ => {}
                            }
                            m += 1;
                        }
                        if d == 0 {
                            body = Some(&src[k + 1..m - 1]);
                            k = m;
                        } else {
                            k = bytes.len();
                        }
                        break;
                    }
                    _ => {}
                }
                k += 1;
            }
            out.push(FnItem { name, line, body });
            i = if k > i { k } else { i + 2 };
            continue;
        }
        i += 1;
    }
    out
}

fn scrub(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = vec![b' '; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        if src[i..].starts_with("//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if src[i..].starts_with("/*") {
            let mut depth = 1;
            i += 2;
            while i < bytes.len() && depth > 0 {
                if src[i..].starts_with("/*") {
                    depth += 1;
                    i += 2;
                } else if src[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out[i] = b'\n';
                    }
                    i += 1;
                }
            }
            continue;
        }
        if let Some(hashes) = raw_string_prefix(src, i) {
            let mut j = i + hashes + 2;
            let mut closed = false;
            while j < bytes.len() {
                if bytes[j] == b'\n' {
                    out[j] = b'\n';
                }
                if bytes[j] == b'"' && src[j + 1..].starts_with(&"#".repeat(hashes)) {
                    j += 1 + hashes;
                    closed = true;
                    break;
                }
                j += 1;
            }
            i = if closed { j } else { bytes.len() };
            continue;
        }
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'\n' {
                    out[i] = b'\n';
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'\'' && is_char_literal(src, i) {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\'' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        out[i] = bytes[i];
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn raw_string_prefix(src: &str, i: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut j = i;
    if j < bytes.len() && bytes[j] == b'b' {
        j += 1;
    }
    if j < bytes.len() && bytes[j] == b'r' {
        j += 1;
        let mut hashes = 0;
        while j < bytes.len() && bytes[j] == b'#' {
            hashes += 1;
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'"' {
            return Some(hashes);
        }
    }
    None
}

fn is_char_literal(src: &str, i: usize) -> bool {
    let bytes = src.as_bytes();
    if i + 2 >= bytes.len() {
        return false;
    }
    if bytes[i + 1] == b'\\' {
        let mut j = i + 2;
        while j < bytes.len() && bytes[j] != b'\'' {
            j += 1;
        }
        j < bytes.len()
    } else {
        bytes[i + 2] == b'\''
    }
}

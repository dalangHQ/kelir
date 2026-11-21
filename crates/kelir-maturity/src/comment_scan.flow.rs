// see: cans/01-architecture.md#Codebase-invariants
// see: cans/02-plugin-model.md#Hook-return-semantics
//! Raw-source comment scanning: syn does not expose comments, so the
//! l3-marker ban scans the raw source for comment lines; string literals
//! come from syn spans (span-locations).
const L3_TOKENS: [&str; 3] = ["TODO (L3)", "L3-GAP", "(L3)"];

pub fn has_l3_marker_token(text: &str) -> bool {
    L3_TOKENS.iter().any(|t| text.contains(t))
}

// see: cans/01-architecture.md#Codebase-invariants
pub struct CommentScan {
    lines: Vec<(usize, String)>,
}

impl CommentScan {
    pub fn new(src: &str) -> CommentScan {
        CommentScan {
            lines: src.lines().enumerate().map(|(i, t)| (i + 1, t.to_string())).collect(),
        }
    }

    // see: cans/01-architecture.md#Codebase-invariants
    pub fn marker_comment_lines(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (no, text) in &self.lines {
            if let Some(cs) = comment_start(text) {
                if has_l3_marker_token(&text[cs..]) {
                    out.push(*no);
                }
            }
        }
        out
    }

    // True when the fn starting at fn_line has a TODO (L3)/L3-GAP marker
    // attached: a comment on the same line, or a comment-only line directly
    // above it.
    pub fn marker_attached(&self, fn_line: usize) -> bool {
        if fn_line == 0 {
            return false;
        }
        if let Some(text) = self.lines.get(fn_line - 1).map(|(_, t)| t.as_str()) {
            if let Some(cs) = comment_start(text) {
                if has_l3_marker_token(&text[cs..]) {
                    return true;
                }
            }
        }
        if fn_line >= 2 {
            if let Some(text) = self.lines.get(fn_line - 2).map(|(_, t)| t.as_str()) {
                let trimmed = text.trim_start();
                if trimmed.starts_with("//") && has_l3_marker_token(trimmed) {
                    return true;
                }
            }
        }
        false
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn comment_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i + 1 < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'/' && bytes[i + 1] == b'/' {
            return Some(i);
        }
        i += 1;
    }
    None
}

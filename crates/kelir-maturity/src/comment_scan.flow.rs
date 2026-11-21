// see: cans/01-architecture.md#Codebase-invariants
// TODO (L2): drop the raw comment scan once a comment-aware parser lands
const L3_TOKENS: [&str; 3] = [
    concat!("TODO ", "(", "L3", ")"),
    concat!("L3", "-", "GAP"),
    concat!("(", "L3", ")"),
];

pub const MARKER_TOKEN: &str = concat!("(", "L3", ")");

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
            lines: src
                .lines()
                .enumerate()
                .map(|(i, t)| (i + 1, t.to_string()))
                .collect(),
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

    // see: cans/01-architecture.md#Codebase-invariants
    pub fn marker_attached(&self, fn_line: usize, end_line: usize) -> bool {
        let start = fn_line.saturating_sub(2).max(1);
        let end = end_line.max(fn_line);
        for (no, text) in &self.lines {
            if *no < start || *no > end {
                continue;
            }
            if let Some(cs) = comment_start(text) {
                if has_l3_marker_token(&text[cs..]) {
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

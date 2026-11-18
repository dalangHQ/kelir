// see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)

pub(crate) fn render_notice_box(title: &str, body: &str) -> Vec<u8> {
    let width = title.chars().count().max(
        body.lines()
            .map(str::chars)
            .map(Iterator::count)
            .max()
            .unwrap_or_default(),
    );
    let mut out = String::new();
    out.push_str("\u{1b}[1;33m");
    out.push('\u{250c}');
    out.push_str(&"\u{2500}".repeat(width + 2));
    out.push('\u{2510}');
    out.push('\n');
    out.push('\u{2502}');
    out.push(' ');
    out.push_str(title);
    out.push_str(&" ".repeat(width - title.chars().count() + 1));
    out.push('\u{2502}');
    out.push('\n');
    for line in body.lines() {
        out.push('\u{2502}');
        out.push(' ');
        out.push_str(line);
        out.push_str(&" ".repeat(width - line.chars().count() + 1));
        out.push('\u{2502}');
        out.push('\n');
    }
    out.push('\u{2514}');
    out.push_str(&"\u{2500}".repeat(width + 2));
    out.push('\u{2518}');
    out.push_str("\u{1b}[0m\n");
    out.into_bytes()
}

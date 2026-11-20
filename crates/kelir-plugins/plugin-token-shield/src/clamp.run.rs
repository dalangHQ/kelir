// see: cans/plugins/token-shield.md#Invariants-and-policy-(token-shield)
// TODO (L3) [terminal_guard_prompt_match]: implement token-shield scrollback clamping with saturating math and token redaction.; slop: scrollback is never clamped, token spans are never redacted, secrets stay visible in scrollback.

pub(crate) fn apply_token_shield(grid: &[u8], max_lines: usize, max_bytes: usize) -> Vec<u8> {
    // L3-GAP: [tp_05_token_shield_scrollback_clamping]: truncate grid to max_lines/max_bytes with saturating math and redact token spans; slop: clamp slices mid-char, invalid UTF-8, tokens leak
    let _ = (max_lines, max_bytes);
    // TODO (L3) [terminal_guard_prompt_match]: truncate grid to max_lines/max_bytes and redact token spans.; slop: the grid is never truncated, token spans are never redacted, secrets leak through the terminal grid.
    grid.to_vec()
}

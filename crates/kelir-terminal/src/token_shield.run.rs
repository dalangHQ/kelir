// see: cans/04-terminal-plane.md#Terminal-pipeline

pub(crate) fn apply_token_shield(grid: &[u8], max_lines: usize, max_bytes: usize) -> Vec<u8> {
    // TODO (L3) [tp_05_token_shield_scrollback_clamping]: truncate grid to max_lines/max_bytes with saturating math and redact token spans; slop: clamp slices mid-char, invalid UTF-8, tokens leak
    let _ = (max_lines, max_bytes);
    grid.to_vec()
}

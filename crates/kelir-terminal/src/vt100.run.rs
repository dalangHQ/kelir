// see: cans/04-terminal-plane.md#Terminal-pipeline
use kelir_interface::plugin_model::PluginError;

use crate::vt100::Vt100StateMachine;

// TODO (L3) [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green.
#[allow(dead_code)]
impl Vt100StateMachine {
    pub(crate) fn feed(&mut self, _bytes: &[u8]) -> Result<(), PluginError> {
        // TODO (L3) [terminal_plane]: parse VT100 escape sequences in bytes and update matrix + cursor; slop: escape sequences unparsed, matrix and cursor wrong
        Ok(())
    }
}

#[allow(dead_code)]
pub(crate) fn filter_ansi_shadow(buf: &[u8]) -> Vec<u8> {
    // TODO (L3) [tp_03_ansi_sanitizer_osc52_clipboard_drop]: strip ANSI shadow sequences from buf; slop: OSC52 payload reaches host terminal, bytes not purged
    buf.to_vec()
}

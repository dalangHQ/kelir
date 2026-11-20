// see: cans/04-terminal-plane.md#PTY-line-discipline-initialization-(kelir-shim)

use crate::trap_protocol::ShimError;

pub fn open_pty_slave() -> Result<(i32, i32), ShimError> {
    // TODO (L3) [terminal_plane]: open a pty master/slave pair and return both fds.; slop: open returns the same fd twice, so master and slave writes echo back as input.
    Err(ShimError::PtyOpen { errno: 0 })
}

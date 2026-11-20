// see: cans/04-terminal-plane.md#PTY-line-discipline-initialization-(kelir-shim)

use crate::trap_protocol::ShimError;

pub fn open_pty_slave() -> Result<(i32, i32), ShimError> {
    crate::pty_terminal_run::open_pty_slave()
}

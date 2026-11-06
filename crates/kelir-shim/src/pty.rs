// see: cans/04-terminal-plane.md#PTY-line-discipline-initialization-(kelir-shim)

#[allow(dead_code)]
pub fn open_pty_slave() -> Result<(i32, i32), ()> {
    let mut master: libc::c_int = -1;
    let mut slave: libc::c_int = -1;
    // SAFETY: openpty writes valid fds into master/slave on success; null name/termios/winsize request kernel defaults.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if rc == 0 {
        Ok((master, slave))
    } else {
        Err(())
    }
}

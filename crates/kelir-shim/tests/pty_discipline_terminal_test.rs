#![cfg(target_os = "linux")]

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_15_shim_openpty_returns_distinct_master_slave.yaml#case_id
use kelir_shim::open_pty_slave;

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_15_shim_openpty_returns_distinct_master_slave.yaml#case_id
fn open_fd_count() -> Result<usize, String> {
    let entries =
        std::fs::read_dir("/proc/self/fd").map_err(|e| format!("read /proc/self/fd: {e}"))?;
    Ok(entries.count())
}

// see: cans/artifacts/test-plan/cases/terminal_plane/tp_15_shim_openpty_returns_distinct_master_slave.yaml#case_id
#[test]
fn tp_15_shim_openpty_returns_distinct_master_slave() -> Result<(), String> {
    let before = open_fd_count()?;
    let (master, slave) = match open_pty_slave() {
        Ok(pair) => pair,
        Err(e) => return Err(format!("open_pty_slave failed: {e}")),
    };
    assert!(master >= 0 && slave >= 0 && master != slave);
    // SAFETY: tcgetattr fills tio on success; zeroed termios is a valid initial value.
    let mut tio: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: slave is a valid open fd from open_pty_slave; tio is a valid out-pointer.
    let rc = unsafe { libc::tcgetattr(slave, &mut tio) };
    assert_eq!(rc, 0);
    // SAFETY: tio was filled by tcgetattr; cfmakeraw only writes its fields.
    unsafe { libc::cfmakeraw(&mut tio) };
    // SAFETY: slave is a valid open fd; tio is a valid termios pointer.
    let rc = unsafe { libc::tcsetattr(slave, libc::TCSANOW, &tio) };
    assert_eq!(rc, 0);
    let byte = *b"x";
    // SAFETY: master is a valid open fd; byte points at one readable byte.
    let written = unsafe { libc::write(master, byte.as_ptr() as *const libc::c_void, 1) };
    assert_eq!(written, 1);
    let mut buf = [0u8; 1];
    // SAFETY: slave is a valid open fd in raw mode; buf points at one writable byte.
    let read = unsafe { libc::read(slave, buf.as_mut_ptr() as *mut libc::c_void, 1) };
    assert_eq!(read, 1);
    assert_eq!(buf[0], b'x');
    // SAFETY: master and slave are valid open fds owned by this test.
    let rc = unsafe { libc::close(master) };
    assert_eq!(rc, 0);
    // SAFETY: slave is a valid open fd owned by this test.
    let rc = unsafe { libc::close(slave) };
    assert_eq!(rc, 0);
    assert_eq!(open_fd_count()?, before);
    Ok(())
}

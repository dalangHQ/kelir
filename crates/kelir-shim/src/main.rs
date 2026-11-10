// see: cans/01-architecture.md#Axiomatic-foundations
// see: cans/08-lifecycle.md#Zombie-reaping
// see: cans/artifacts/diagrams/guest-vm.txt#kelir-shim
// TODO (L3): tests/test_anti_toctou.rs + tests/test_reaper.rs per cans/artifacts/config/project-structure.txt#kelir-shim.

mod codec_protocol;
mod introspect_trap;
mod multiplexer_protocol;
mod pty_terminal;
mod trap_protocol;

use kelir_interface::vsock_mesh::VSOCK_CONTROL_PORT;

// see: cans/11-protocol.md#Control-channel
const STREAM_CHANNEL: u16 = 0x0001;

// see: cans/08-lifecycle.md#Zombie-reaping
const POLL_INTERVAL_MS: libc::c_int = 100;

// TODO (L3): surface the exit reason to the host supervisor for attestation receipts (in-band Channel 0x0000 or early-boot UART scrape).
fn main() -> Result<(), trap_protocol::ShimError> {
    let mut mux = multiplexer_protocol::Multiplexer::new();
    // TODO (L3): SPSC ring primary carrier per cans/11-protocol.md#Wire-framing; the AF_VSOCK fallback carrier is wired below.
    let control_fd = connect_channel(trap_protocol::TRAP_CHANNEL)?;
    let data_fd = connect_channel(STREAM_CHANNEL)?;
    // TODO (L3): exchange the Control 0x0000 handshake (version 0x01 + capabilities) before frame traffic.
    let unotify = trap_protocol::init_seccomp_unotify()?;
    let (_pty_master, _pty_slave) = pty_terminal::open_pty_slave()?;
    let mut trap_seq: u64 = 0;
    let mut header = [0u8; codec_protocol::HEADER_LEN];
    let mut payload = [0u8; codec_protocol::MAX_FRAME_PAYLOAD];
    loop {
        let mut status: libc::c_int = 0;
        loop {
            // SAFETY: status is a valid out-pointer; WNOHANG never blocks.
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            // TODO (L3): update the reaped ExecSlot exit status; unmatched pids discard into the zero-allocation sink.
        }
        let (control_ready, data_ready) = wait_channels(unotify.fd(), control_fd, data_fd)?;
        match unotify.dispatch_pending(trap_seq) {
            Ok(Some(inspection)) => {
                trap_seq += 1;
                match introspect_trap::encode_trap_frame(&inspection.event) {
                    Ok(frame) => {
                        write_all(control_fd, &frame, trap_protocol::TRAP_CHANNEL)?;
                        // TODO (L3): await the host verdict, re-freeze for ExecveMutation patch application, then SECCOMP_IOCTL_NOTIF_SEND (continue/errno/patch) releases the trapped syscall.
                    }
                    Err(_reason) => {
                        // TODO (L3): emit in-band trap-frame reject from the structured reason.
                    }
                }
            }
            Ok(None) => {}
            Err(_reason) => {
                // TODO (L3): emit in-band trap-dispatch reject from the structured reason.
            }
        }
        if control_ready {
            dispatch_control_frame(control_fd, &mut mux, &mut header, &mut payload)?;
        }
        if data_ready {
            dispatch_data_frame(data_fd, &mut mux, &mut header, &mut payload)?;
        }
    }
}

// see: cans/11-protocol.md#Control-channel
fn connect_channel(channel: u16) -> Result<libc::c_int, trap_protocol::ShimError> {
    // SAFETY: a fresh AF_VSOCK stream socket has no aliased state; failure returns -1 without side effects.
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(trap_protocol::ShimError::ChannelOpen {
            channel,
            errno: std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
        });
    }
    let addr = libc::sockaddr_vm {
        svm_family: libc::AF_VSOCK as libc::sa_family_t,
        svm_reserved1: 0,
        svm_port: VSOCK_CONTROL_PORT,
        svm_cid: libc::VMADDR_CID_HOST,
        svm_zero: [0; 4],
    };
    // SAFETY: addr is fully initialized for its declared size and connect(2) only reads it.
    let rc = unsafe {
        libc::connect(
            fd,
            &addr as *const libc::sockaddr_vm as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_vm>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        // SAFETY: fd is a live socket descriptor owned by this call and no longer needed on failure.
        unsafe { libc::close(fd) };
        return Err(trap_protocol::ShimError::ChannelConnect { channel, errno });
    }
    Ok(fd)
}

// see: cans/11-protocol.md#Wire-framing
fn wait_channels(
    notify_fd: libc::c_int,
    control_fd: libc::c_int,
    data_fd: libc::c_int,
) -> Result<(bool, bool), trap_protocol::ShimError> {
    let mut pfds = [
        libc::pollfd {
            fd: notify_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: control_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: data_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    // SAFETY: pfds is a valid 3-entry pollfd array; the bounded timeout parks the loop so WNOHANG reaping keeps running.
    let rc = unsafe {
        libc::poll(
            pfds.as_mut_ptr(),
            pfds.len() as libc::c_ulong,
            POLL_INTERVAL_MS,
        )
    };
    if rc < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == libc::EINTR {
            return Ok((false, false));
        }
        return Err(trap_protocol::ShimError::Poll { errno });
    }
    Ok((
        channel_readable(&pfds[1], trap_protocol::TRAP_CHANNEL)?,
        channel_readable(&pfds[2], STREAM_CHANNEL)?,
    ))
}

// see: cans/11-protocol.md#Wire-framing
fn channel_readable(pfd: &libc::pollfd, channel: u16) -> Result<bool, trap_protocol::ShimError> {
    if pfd.revents & libc::POLLNVAL != 0 {
        return Err(trap_protocol::ShimError::ChannelIo {
            channel,
            errno: libc::EBADF,
        });
    }
    if pfd.revents & libc::POLLIN != 0 {
        return Ok(true);
    }
    if pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
        return Err(trap_protocol::ShimError::ChannelEof { channel });
    }
    Ok(false)
}

// see: cans/11-protocol.md#Control-channel
fn dispatch_control_frame(
    fd: libc::c_int,
    mux: &mut multiplexer_protocol::Multiplexer,
    header: &mut [u8; codec_protocol::HEADER_LEN],
    payload: &mut [u8; codec_protocol::MAX_FRAME_PAYLOAD],
) -> Result<(), trap_protocol::ShimError> {
    if let Some((_, _, len)) = read_frame(fd, trap_protocol::TRAP_CHANNEL, header, payload)? {
        // TODO (L3): emit in-band control reject when dispatch_control returns a structured Err reason.
        let _ = mux.dispatch_control(&payload[..len]);
    }
    Ok(())
}

// see: cans/11-protocol.md#Stream-mux-framing
fn dispatch_data_frame(
    fd: libc::c_int,
    mux: &mut multiplexer_protocol::Multiplexer,
    header: &mut [u8; codec_protocol::HEADER_LEN],
    payload: &mut [u8; codec_protocol::MAX_FRAME_PAYLOAD],
) -> Result<(), trap_protocol::ShimError> {
    if let Some((stream_id, flags, len)) = read_frame(fd, STREAM_CHANNEL, header, payload)? {
        // TODO (L3): emit in-band reject when dispatch_frame errs (SlabSaturated => MSG_EXEC_REJECT{ENOMEM}).
        let _ = mux.dispatch_frame(stream_id, flags, &payload[..len]);
    }
    Ok(())
}

// see: cans/11-protocol.md#Wire-framing
fn read_frame(
    fd: libc::c_int,
    channel: u16,
    header: &mut [u8; codec_protocol::HEADER_LEN],
    payload: &mut [u8; codec_protocol::MAX_FRAME_PAYLOAD],
) -> Result<Option<(u32, u8, usize)>, trap_protocol::ShimError> {
    read_exact(fd, channel, header)?;
    let Ok(frame) = codec_protocol::decode_header(header) else {
        // TODO (L3): framing resync after a corrupt header; the frame boundary is unrecoverable until then.
        return Ok(None);
    };
    let len = frame.len as usize;
    read_exact(fd, channel, &mut payload[..len])?;
    Ok(Some((frame.stream_id, frame.flags, len)))
}

// see: cans/11-protocol.md#Wire-framing
fn read_exact(
    fd: libc::c_int,
    channel: u16,
    buf: &mut [u8],
) -> Result<(), trap_protocol::ShimError> {
    let mut filled = 0;
    while filled < buf.len() {
        // SAFETY: buf+filled is valid for buf.len()-filled writable bytes; read(2) writes at most that span and only returns n>=0 on success.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().add(filled).cast(), buf.len() - filled) };
        if n == 0 {
            return Err(trap_protocol::ShimError::ChannelEof { channel });
        }
        if n < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::EINTR {
                continue;
            }
            return Err(trap_protocol::ShimError::ChannelIo { channel, errno });
        }
        filled += n as usize;
    }
    Ok(())
}

// see: cans/11-protocol.md#Trap-fast-path
fn write_all(fd: libc::c_int, buf: &[u8], channel: u16) -> Result<(), trap_protocol::ShimError> {
    let mut written = 0;
    while written < buf.len() {
        // SAFETY: buf+written is valid for buf.len()-written readable bytes; write(2) reads at most that span.
        let n = unsafe { libc::write(fd, buf.as_ptr().add(written).cast(), buf.len() - written) };
        if n == 0 {
            return Err(trap_protocol::ShimError::ChannelEof { channel });
        }
        if n < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::EINTR {
                continue;
            }
            return Err(trap_protocol::ShimError::ChannelIo { channel, errno });
        }
        written += n as usize;
    }
    Ok(())
}

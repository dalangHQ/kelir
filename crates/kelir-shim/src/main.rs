// see: cans/08-lifecycle.md#Zombie-reaping
// see: cans/artifacts/diagrams/guest-vm.txt

// see: cans/artifacts/config/project-structure.txt#kelir-shim
mod codec;
mod multiplexer;
mod pty;
mod trap;

fn main() -> Result<(), ()> {
    let mut mux = multiplexer::Multiplexer::new();
    // see: cans/11-protocol.md#Wire-framing
    let _unotify_fd = trap::init_seccomp_unotify()?;
    let (_pty_master, _pty_slave) = pty::open_pty_slave()?;
    let rx_header = [0u8; codec::HEADER_LEN];
    let mut tx_frame = [0u8; codec::HEADER_LEN];
    loop {
        let mut status: libc::c_int = 0;
        loop {
            // SAFETY: status is a valid out-pointer; WNOHANG never blocks.
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
        }
        // see: cans/11-protocol.md#Wire-framing
        match codec::decode_header(&rx_header) {
            Ok((stream_id, flags, _len)) => {
                // see: cans/11-protocol.md#Wire-framing
                if codec::encode_frame(stream_id, flags, &[], &mut tx_frame).is_err() {
                    // see: cans/11-protocol.md#Wire-framing
                }
                if mux.dispatch_frame(stream_id, flags, &[]).is_err() {
                    // see: cans/11-protocol.md#Binary-exec-channel
                }
            }
            Err(()) => {
                // see: cans/11-protocol.md#Wire-framing
            }
        }
    }
}

// see: cans/08-lifecycle.md#Zombie-reaping
// see: cans/artifacts/diagrams/guest-vm.txt
// TODO (L3): tests/test_anti_toctou.rs + test_reaper.rs per cans/artifacts/config/project-structure.txt#kelir-shim.

// TODO (L3): tests/test_anti_toctou.rs + tests/test_reaper.rs per cans/artifacts/config/project-structure.txt.
mod codec;
mod multiplexer;
mod pty;
mod trap;

fn main() -> Result<(), ()> {
    let mut mux = multiplexer::Multiplexer::new();
    // TODO: attach virtio-vsock port 5252 rings; split channels 0x0000/0x0001
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
        // TODO: poll transport ring; copy next 9 header bytes into rx_header
        match codec::decode_header(&rx_header) {
            Ok((stream_id, flags, _len)) => {
                // TODO: slice _len payload bytes from ring into the dispatch payload
                if codec::encode_frame(stream_id, flags, &[], &mut tx_frame).is_err() {
                    // TODO: tx ring overflow accounting
                }
                if mux.dispatch_frame(stream_id, flags, &[]).is_err() {
                    // TODO: emit MSG_EXEC_REJECT{ENOMEM} in-band on slab saturation
                }
            }
            Err(()) => {
                // TODO: framing resync after corrupt header
            }
        }
    }
}

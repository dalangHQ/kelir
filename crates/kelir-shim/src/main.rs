// see: cans/01-architecture.md#Axiomatic-foundations
// see: cans/08-lifecycle.md#Zombie-reaping
// see: cans/artifacts/diagrams/guest-vm.txt#kelir-shim
// TODO (L3): tests/test_anti_toctou.rs + tests/test_reaper.rs per cans/artifacts/config/project-structure.txt#kelir-shim.

mod codec;
mod introspect;
mod multiplexer;
mod pty;
mod trap;

// TODO (L3): surface the exit reason to the host supervisor for attestation receipts (in-band Channel 0x0000 or early-boot UART scrape).
fn main() -> Result<(), trap::ShimError> {
    let mut mux = multiplexer::Multiplexer::new();
    // TODO (L3): attach virtio-vsock port 5252 rings; split channels 0x0000/0x0001.
    let unotify = trap::init_seccomp_unotify()?;
    let (_pty_master, _pty_slave) = pty::open_pty_slave()?;
    let mut trap_seq: u64 = 0;
    let rx_control_header = [0u8; codec::HEADER_LEN];
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
        // TODO (L3): cgroup v2 freeze (cans/11) must cover only the trapped thread and lift before /proc mem patch injection or the microVM deadlocks.
        // TODO (L3): poll control ring; copy next 9 header bytes into rx_control_header.
        match unotify.dispatch_pending(trap_seq) {
            Ok(Some(inspection)) => {
                trap_seq += 1;
                match introspect::encode_trap_frame(&inspection.event) {
                    Ok(_frame) => {
                        // TODO (L3): copy the MSG_TRAP frame bytes onto the vsock ring once the rings attach.
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
        match codec::decode_header(&rx_control_header) {
            Ok((_stream_id, _flags, _len)) => {
                // TODO (L3): slice _len payload bytes from the control ring into the dispatch payload.
                if let Err(_reason) = mux.dispatch_control(&[]) {
                    // TODO (L3): emit in-band control reject from the structured reason.
                }
            }
            Err(()) => {
                // TODO (L3): framing resync after corrupt control header.
            }
        }
        // TODO (L3): poll transport ring; copy next 9 header bytes into rx_header.
        match codec::decode_header(&rx_header) {
            Ok((stream_id, flags, _len)) => {
                // TODO (L3): slice _len payload bytes from ring into the dispatch payload.
                if codec::encode_frame(stream_id, flags, &[], &mut tx_frame).is_err() {
                    // TODO (L3): tx ring overflow accounting.
                }
                if let Err(_reason) = mux.dispatch_frame(stream_id, flags, &[]) {
                    // TODO (L3): emit in-band reject from the structured reason (SlabSaturated => MSG_EXEC_REJECT{ENOMEM}).
                }
            }
            Err(()) => {
                // TODO (L3): framing resync after corrupt header.
            }
        }
    }
}

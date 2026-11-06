// see: cans/08-lifecycle.md#Zombie-reaping

mod codec;
mod multiplexer;
mod pty;
mod trap;

fn main() -> Result<(), ()> {
    let mut mux = multiplexer::Multiplexer::new();
    let _ = &mut mux;
    loop {
        let mut status: libc::c_int = 0;
        loop {
            // SAFETY: status is a valid out-pointer; WNOHANG never blocks.
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
        }
        // TODO: transport read + mux.dispatch_frame event loop (L3).
        // SAFETY: sleep takes a plain seconds count; no pointer involved.
        unsafe {
            libc::sleep(1);
        }
    }
}

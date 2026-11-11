// see: cans/07-wire-plane.md#Port-forwarding
// see: cans/07-wire-plane.md#Socket-lifecycle
use std::io::Error;
use std::os::fd::{FromRawFd, OwnedFd};

// see: cans/07-wire-plane.md#Socket-lifecycle
#[derive(Debug)]
pub struct HostListener {
    fd: OwnedFd,
    port: u16,
}

impl HostListener {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn fd(&self) -> &OwnedFd {
        &self.fd
    }
}

// see: cans/07-wire-plane.md#Port-forwarding
pub fn bind_host_listener(port: u16) -> std::io::Result<HostListener> {
    // SAFETY: fd is a freshly created socket owned exclusively by this call; every libc call below receives valid pointers to stack storage that outlive the call.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0);
        if fd < 0 {
            return Err(Error::last_os_error());
        }
        let close_on_error = |error: Error| -> Error {
            libc::close(fd);
            error
        };
        let reuse: libc::c_int = 1;
        if libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &reuse as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            return Err(close_on_error(Error::last_os_error()));
        }
        if libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            &reuse as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            return Err(close_on_error(Error::last_os_error()));
        }
        let addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: port.to_be(),
            sin_addr: libc::in_addr {
                s_addr: libc::htonl(libc::INADDR_LOOPBACK),
            },
            sin_zero: [0; 8],
        };
        if libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        ) < 0
        {
            return Err(close_on_error(Error::last_os_error()));
        }
        if libc::listen(fd, 128) < 0 {
            return Err(close_on_error(Error::last_os_error()));
        }
        let mut bound: libc::sockaddr_in = std::mem::zeroed();
        let mut bound_len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
        if libc::getsockname(
            fd,
            &mut bound as *mut libc::sockaddr_in as *mut libc::sockaddr,
            &mut bound_len,
        ) < 0
        {
            return Err(close_on_error(Error::last_os_error()));
        }
        Ok(HostListener {
            fd: OwnedFd::from_raw_fd(fd),
            port: u16::from_be(bound.sin_port),
        })
    }
}

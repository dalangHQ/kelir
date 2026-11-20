// see: cans/08-lifecycle.md#Detached-lifecycle
use crate::MemoryHandoff;

impl MemoryHandoff {
    pub fn new(fd: std::os::unix::io::OwnedFd, len: u64) -> Self {
        Self { fd, len }
    }

    pub fn fd(&self) -> &std::os::unix::io::OwnedFd {
        &self.fd
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

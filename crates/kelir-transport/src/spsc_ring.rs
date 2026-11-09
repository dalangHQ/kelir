// see: cans/01-architecture.md#Transport-engine-abstraction
use std::sync::atomic::{AtomicU32, Ordering};

use crate::Transport;

pub struct RingHeader {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    pub capacity: u32,
}

pub struct SpscRingTransport {
    pub header: *mut RingHeader,
    pub buffer: *mut u8,
}

// SAFETY: single-producer/single-consumer ring; header and buffer are mapped once and owned exclusively by the transport pair.
unsafe impl Send for SpscRingTransport {}

impl SpscRingTransport {
    pub fn new(header: *mut RingHeader, buffer: *mut u8) -> Self {
        Self { header, buffer }
    }

    // SAFETY: header points at the ring header mapped once before sharing; capacity is written once at setup and never mutated after.
    fn header_ref(&self) -> &RingHeader {
        unsafe { &*self.header }
    }

    pub fn used(&self) -> u32 {
        let header = self.header_ref();
        header
            .head
            .load(Ordering::Acquire)
            .wrapping_sub(header.tail.load(Ordering::Acquire))
    }

    pub fn free(&self) -> u32 {
        self.header_ref().capacity.saturating_sub(self.used())
    }
}

// see: cans/01-architecture.md#Transport-engine-abstraction
// see: cans/01-architecture.md#Shared-memory-channel
use kelir_interface::driver_interface::SharedMemoryHandle;

enum RingBacking {
    Owned {
        header: Box<RingHeader>,
        buffer: Box<[u8]>,
    },
    Mapped {
        header: *mut RingHeader,
        buffer: *mut u8,
        guest_phys_addr: u64,
    },
}

// SAFETY: a mapped backing is mapped once by the driver and owned exclusively by the transport pair, same as the owned heap backing.
unsafe impl Send for OwnedSpscRing {}

pub struct OwnedSpscRing {
    backing: RingBacking,
    capacity: u32,
}

impl OwnedSpscRing {
    pub fn new(capacity: u32) -> Self {
        Self {
            backing: RingBacking::Owned {
                header: Box::new(RingHeader {
                    head: AtomicU32::new(0),
                    tail: AtomicU32::new(0),
                    capacity,
                }),
                buffer: vec![0u8; capacity as usize].into_boxed_slice(),
            },
            capacity,
        }
    }

    // see: cans/01-architecture.md#Shared-memory-channel
    pub fn from_shared_region(handle: &SharedMemoryHandle) -> Option<Self> {
        let header_size = std::mem::size_of::<RingHeader>();
        if handle.host_ptr.is_null() || handle.size_bytes <= header_size {
            return None;
        }
        let capacity = handle.size_bytes.saturating_sub(header_size) as u32;
        if capacity == 0 {
            return None;
        }
        let header = handle.host_ptr as *mut RingHeader;
        // SAFETY: the driver mapped size_bytes at host_ptr; the header occupies the first header_size bytes and the buffer follows it.
        let buffer = unsafe { handle.host_ptr.add(header_size) };
        // SAFETY: from_shared_region claims a freshly mapped region; no producer or consumer reads the header before this write.
        unsafe {
            std::ptr::write(
                header,
                RingHeader {
                    head: AtomicU32::new(0),
                    tail: AtomicU32::new(0),
                    capacity,
                },
            );
        }
        Some(Self {
            backing: RingBacking::Mapped {
                header,
                buffer,
                guest_phys_addr: handle.guest_phys_addr,
            },
            capacity,
        })
    }

    pub fn is_shared_backed(&self) -> bool {
        matches!(self.backing, RingBacking::Mapped { .. })
    }

    pub fn guest_phys_addr(&self) -> Option<u64> {
        match &self.backing {
            RingBacking::Mapped {
                guest_phys_addr, ..
            } => Some(*guest_phys_addr),
            RingBacking::Owned { .. } => None,
        }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn transport(&mut self) -> SpscRingTransport {
        match &mut self.backing {
            RingBacking::Owned { header, buffer } => {
                SpscRingTransport::new(header.as_mut() as *mut RingHeader, buffer.as_mut_ptr())
            }
            RingBacking::Mapped { header, buffer, .. } => SpscRingTransport::new(*header, *buffer),
        }
    }
}

impl Transport for SpscRingTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let header = self.header_ref();
        let tail = header.tail.load(Ordering::Acquire);
        let head = header.head.load(Ordering::Acquire);
        let used = head.wrapping_sub(tail);
        if used == 0 {
            return Ok(0);
        }
        let count = (used as usize).min(buf.len());
        let capacity = header.capacity as usize;
        let tail_offset = (tail as usize) % capacity;
        // SAFETY: buffer maps capacity bytes once at setup; count <= used <= capacity keeps tail_offset..+count in-bounds with wraparound.
        unsafe {
            if tail_offset + count <= capacity {
                buf[..count].copy_from_slice(std::slice::from_raw_parts(
                    self.buffer.add(tail_offset),
                    count,
                ));
            } else {
                let first = capacity - tail_offset;
                buf[..first].copy_from_slice(std::slice::from_raw_parts(
                    self.buffer.add(tail_offset),
                    first,
                ));
                buf[first..count]
                    .copy_from_slice(std::slice::from_raw_parts(self.buffer, count - first));
            }
        }
        // TODO (L3): MMIO/PCI guest doorbell wakes this consumer via host eventfd instead of caller polling.
        header
            .tail
            .store(tail.wrapping_add(count as u32), Ordering::Release);
        Ok(count)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let header = self.header_ref();
        let head = header.head.load(Ordering::Relaxed);
        let tail = header.tail.load(Ordering::Acquire);
        let used = head.wrapping_sub(tail);
        let free = header.capacity.saturating_sub(used);
        if free == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "spsc ring full: producer must park on the doorbell",
            ));
        }
        let count = (free as usize).min(buf.len());
        let capacity = header.capacity as usize;
        let head_offset = (head as usize) % capacity;
        // SAFETY: buffer maps capacity bytes once at setup; count <= free keeps head_offset..+count in-bounds with wraparound.
        unsafe {
            if head_offset + count <= capacity {
                std::slice::from_raw_parts_mut(self.buffer.add(head_offset), count)
                    .copy_from_slice(&buf[..count]);
            } else {
                let first = capacity - head_offset;
                std::slice::from_raw_parts_mut(self.buffer.add(head_offset), first)
                    .copy_from_slice(&buf[..first]);
                std::slice::from_raw_parts_mut(self.buffer, count - first)
                    .copy_from_slice(&buf[first..count]);
            }
        }
        // TODO (L3): host doorbell injects a paravirtual virtual interrupt into the guest vCPU on enqueue.
        header
            .head
            .store(head.wrapping_add(count as u32), Ordering::Release);
        Ok(count)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // TODO (L3): explicit host memory fence + doorbell notification under the virtio-shm device model.
        Ok(())
    }
}

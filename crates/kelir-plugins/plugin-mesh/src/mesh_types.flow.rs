// see: cans/02-plugin-model.md#Facade-blindness
use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// see: cans/plugins/mesh.md#SPIFFE-identity-(mesh)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiffeId {
    pub trust_domain: String,
    pub workload_prefix: String,
}

// see: cans/plugins/mesh.md#SPIFFE-identity-(mesh)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiffeSvid {
    pub spiffe_id: SpiffeId,
    pub credential: Vec<u8>,
    pub expires_at_ns: u64,
}

// see: cans/plugins/mesh.md#SPIFFE-identity-(mesh)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSessionHandshake {
    pub svid: SpiffeSvid,
    pub call_hops: u32,
    pub traceparent: Option<String>,
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtioShmRegion {
    pub tag: String,
    pub size_bytes: usize,
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmRingError {
    Full { needed: usize, free: usize },
}

impl std::fmt::Display for ShmRingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full { needed, free } => {
                write!(f, "shm ring holds {free} free bytes; push needs {needed}")
            }
        }
    }
}

impl std::error::Error for ShmRingError {}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
pub struct ShmRing {
    pub region: VirtioShmRegion,
    // TODO (L3) [protocol_mux]: back the ring with the virtio_shm memfd mapping instead of heap bytes.; slop: heap ring shared only in-process, no cross-VM zero-copy
    buf: Mutex<Vec<u8>>,
    head: Mutex<usize>,
    tail: Mutex<usize>,
    len: Mutex<usize>,
}

impl ShmRing {
    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn new(region: VirtioShmRegion) -> Self {
        let capacity = region.size_bytes;
        Self {
            region,
            buf: Mutex::new(vec![0; capacity]),
            head: Mutex::new(0),
            tail: Mutex::new(0),
            len: Mutex::new(0),
        }
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn capacity(&self) -> usize {
        self.region.size_bytes
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn len(&self) -> usize {
        self.len.lock().map(|guard| *guard).unwrap_or(0)
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn push(&self, bytes: &[u8]) -> Result<(), ShmRingError> {
        let mut len = self.len.lock().map_err(|_| ShmRingError::Full {
            needed: bytes.len(),
            free: 0,
        })?;
        let free = self.capacity().saturating_sub(*len);
        if bytes.len() > free {
            return Err(ShmRingError::Full {
                needed: bytes.len(),
                free,
            });
        }
        let mut buf = self.buf.lock().map_err(|_| ShmRingError::Full {
            needed: bytes.len(),
            free,
        })?;
        let mut tail = self.tail.lock().map_err(|_| ShmRingError::Full {
            needed: bytes.len(),
            free,
        })?;
        for byte in bytes {
            buf[*tail] = *byte;
            *tail = (*tail + 1) % self.capacity().max(1);
        }
        *len += bytes.len();
        Ok(())
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn pop(&self, len: usize) -> Option<Vec<u8>> {
        let mut held = self.len.lock().ok()?;
        if *held < len {
            return None;
        }
        let buf = self.buf.lock().ok()?;
        let mut head = self.head.lock().ok()?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(buf[*head]);
            *head = (*head + 1) % self.capacity().max(1);
        }
        *held -= len;
        Some(out)
    }
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    pub entries: HashMap<u32, u64>,
}

impl VectorClock {
    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn tick(&mut self, peer: u32) {
        *self.entries.entry(peer).or_insert(0) += 1;
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn get(&self, peer: u32) -> u64 {
        self.entries.get(&peer).copied().unwrap_or(0)
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn merge(&mut self, other: &VectorClock) {
        for (peer, count) in &other.entries {
            let entry = self.entries.entry(*peer).or_insert(0);
            if *count > *entry {
                *entry = *count;
            }
        }
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut strictly_less = false;
        for (peer, count) in &self.entries {
            let other_count = other.get(*peer);
            if *count > other_count {
                return false;
            }
            if *count < other_count {
                strictly_less = true;
            }
        }
        for (peer, count) in &other.entries {
            if !self.entries.contains_key(peer) && *count > 0 {
                strictly_less = true;
            }
        }
        strictly_less
    }
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
#[derive(Debug, Clone, Default)]
pub struct MeshElection {
    claims: HashMap<u32, VectorClock>,
    clock: VectorClock,
}

impl MeshElection {
    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn broadcast_winner(&mut self, shard_id: u32, clock: &VectorClock) {
        self.clock.merge(clock);
        self.clock.tick(shard_id);
        self.claims.insert(shard_id, self.clock.clone());
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn winner(&self) -> Option<u32> {
        self.claims.keys().min().copied()
    }

    // see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    pub fn claim_clock(&self, shard_id: u32) -> Option<&VectorClock> {
        self.claims.get(&shard_id)
    }
}

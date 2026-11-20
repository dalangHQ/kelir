// see: cans/07-wire-plane.md#Pre-policy-drop-zones
// see: cans/08-lifecycle.md#Lifecycle-state-machine
use std::collections::HashMap;

use crate::driver_interface::ExternalSuspendKind;

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RollbackPolicy {
    #[default]
    Reset,
    Drop,
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionParkingConfig {
    pub enabled: bool,
    pub listen_port: u16,
    pub max_buffered_bytes: usize,
    pub idle_timeout_ms: u64,
    pub wake_on_packet: bool,
    pub rollback_policy: RollbackPolicy,
}

impl Default for ConnectionParkingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_port: 8443,
            max_buffered_bytes: 65_536,
            idle_timeout_ms: 30_000,
            wake_on_packet: true,
            rollback_policy: RollbackPolicy::Reset,
        }
    }
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkedConnection {
    pub id: u64,
    pub peer: String,
    pub buffered: Vec<u8>,
    pub parked_for: ExternalSuspendKind,
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
pub struct ConnectionParking {
    config: ConnectionParkingConfig,
    parked: HashMap<u64, ParkedConnection>,
    next_id: u64,
}

impl ConnectionParking {
    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn new(config: ConnectionParkingConfig) -> Self {
        Self {
            config,
            parked: HashMap::new(),
            next_id: 0,
        }
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn config(&self) -> &ConnectionParkingConfig {
        &self.config
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn parked_count(&self) -> usize {
        self.parked.len()
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn park(
        &mut self,
        peer: String,
        buffered: Vec<u8>,
        hold: ExternalSuspendKind,
    ) -> Option<u64> {
        if !self.config.enabled || buffered.len() > self.config.max_buffered_bytes {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.parked.insert(
            id,
            ParkedConnection {
                id,
                peer,
                buffered,
                parked_for: hold,
            },
        );
        Some(id)
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn resume(&mut self, id: u64) -> Option<ParkedConnection> {
        self.parked.remove(&id)
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn resume_all(&mut self) -> Vec<ParkedConnection> {
        self.parked
            .drain()
            .map(|(_, connection)| connection)
            .collect()
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn reset_on_rollback(&mut self) -> Vec<u64> {
        if self.config.rollback_policy != RollbackPolicy::Reset {
            return Vec::new();
        }
        self.parked.drain().map(|(id, _)| id).collect()
    }
}

impl Default for ConnectionParking {
    fn default() -> Self {
        Self::new(ConnectionParkingConfig::default())
    }
}

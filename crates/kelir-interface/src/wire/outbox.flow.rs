// see: cans/07-wire-plane.md#Egress-side-effect-outbox
use std::collections::VecDeque;

// see: cans/07-wire-plane.md#Egress-side-effect-outbox
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxError {
    Disabled,
    CapacityExceeded { buffered: usize, capacity: usize },
}

impl std::fmt::Display for OutboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => f.write_str("egress outbox deferral is disabled"),
            Self::CapacityExceeded { buffered, capacity } => write!(
                f,
                "egress outbox holds {buffered} bytes against a {capacity} byte cap"
            ),
        }
    }
}

impl std::error::Error for OutboxError {}

// see: cans/07-wire-plane.md#Egress-side-effect-outbox
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxDeferralConfig {
    pub enabled: bool,
    pub buffer_capacity_bytes: usize,
    pub release_on_step_verification: bool,
    pub drop_on_rollback: bool,
    pub compensator_webhook_timeout_ms: u64,
}

impl Default for OutboxDeferralConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_capacity_bytes: 67_108_864,
            release_on_step_verification: true,
            drop_on_rollback: true,
            compensator_webhook_timeout_ms: 5000,
        }
    }
}

// see: cans/07-wire-plane.md#Egress-side-effect-outbox
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxFrame {
    pub id: u64,
    pub payload: Vec<u8>,
    pub irreversible: bool,
}

impl OutboxFrame {
    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

// see: cans/07-wire-plane.md#Egress-side-effect-outbox
pub struct OutboxBuffer {
    config: OutboxDeferralConfig,
    frames: VecDeque<OutboxFrame>,
    buffered_bytes: usize,
    next_id: u64,
}

impl OutboxBuffer {
    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn new(config: OutboxDeferralConfig) -> Self {
        Self {
            config,
            frames: VecDeque::new(),
            buffered_bytes: 0,
            next_id: 0,
        }
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn config(&self) -> &OutboxDeferralConfig {
        &self.config
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn buffered_bytes(&self) -> usize {
        self.buffered_bytes
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn buffered_frames(&self) -> usize {
        self.frames.len()
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn buffer(&mut self, payload: Vec<u8>, irreversible: bool) -> Result<u64, OutboxError> {
        if !self.config.enabled {
            return Err(OutboxError::Disabled);
        }
        let Some(next) = self.buffered_bytes.checked_add(payload.len()) else {
            return Err(OutboxError::CapacityExceeded {
                buffered: self.buffered_bytes,
                capacity: self.config.buffer_capacity_bytes,
            });
        };
        if next > self.config.buffer_capacity_bytes {
            return Err(OutboxError::CapacityExceeded {
                buffered: self.buffered_bytes,
                capacity: self.config.buffer_capacity_bytes,
            });
        }
        let id = self.next_id;
        self.next_id += 1;
        self.buffered_bytes = next;
        self.frames.push_back(OutboxFrame {
            id,
            payload,
            irreversible,
        });
        Ok(id)
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn release_verified(&mut self) -> Vec<OutboxFrame> {
        if !self.config.release_on_step_verification {
            return Vec::new();
        }
        self.buffered_bytes = 0;
        self.frames.drain(..).collect()
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn purge_rollback(&mut self) -> Vec<OutboxFrame> {
        if !self.config.drop_on_rollback {
            return Vec::new();
        }
        self.buffered_bytes = 0;
        self.frames.drain(..).collect()
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn compensator_payload(frame: &OutboxFrame) -> Vec<u8> {
        format!(
            "{{\"outbox_frame_id\":{},\"irreversible\":{},\"bytes\":{}}}",
            frame.id,
            frame.irreversible,
            frame.payload.len()
        )
        .into_bytes()
    }
}

impl Default for OutboxBuffer {
    fn default() -> Self {
        Self::new(OutboxDeferralConfig::default())
    }
}

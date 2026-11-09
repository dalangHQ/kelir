// see: cans/02-plugin-model.md#Plugin-testkit
use kelir_interface::protocol_wire::{MsgType, StreamKind};

#[derive(Debug, Clone)]
pub struct RecordedMessage {
    pub stream_id: u32,
    pub kind: StreamKind,
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct MockBusDispatcher {
    messages: Vec<RecordedMessage>,
}

impl MockBusDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        stream_id: u32,
        kind: StreamKind,
        msg_type: MsgType,
        payload: Vec<u8>,
    ) {
        self.messages.push(RecordedMessage {
            stream_id,
            kind,
            msg_type,
            payload,
        });
    }

    pub fn messages(&self) -> &[RecordedMessage] {
        &self.messages
    }

    pub fn messages_for_kind(&self, kind: StreamKind) -> Vec<&RecordedMessage> {
        self.messages.iter().filter(|m| m.kind == kind).collect()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

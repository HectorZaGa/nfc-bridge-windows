use bytes::Bytes;
use crate::constants::PROTOCOL_VERSION;
use crate::message_type::MessageType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfcBridgeFrame {
    pub version: u8,
    pub message_type: MessageType,
    pub sequence_id: u16,
    pub payload: Bytes,
}

impl NfcBridgeFrame {
    pub fn new(message_type: MessageType, sequence_id: u16, payload: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            message_type,
            sequence_id,
            payload,
        }
    }

    pub fn heartbeat(sequence_id: u16) -> Self {
        Self::new(MessageType::Heartbeat, sequence_id, Bytes::new())
    }

    pub fn heartbeat_ack(sequence_id: u16) -> Self {
        Self::new(MessageType::HeartbeatAck, sequence_id, Bytes::new())
    }

    pub fn disconnect(sequence_id: u16) -> Self {
        Self::new(MessageType::Disconnect, sequence_id, Bytes::new())
    }
}

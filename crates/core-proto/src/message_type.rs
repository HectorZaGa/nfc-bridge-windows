//! Catálogo de tipos de mensaje (Opcodes) del protocolo NFC-Bridge.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    // Control de Conexión (0x01 - 0x0F)
    HelloSyn = 0x01,
    HelloAck = 0x02,
    Heartbeat = 0x03,
    HeartbeatAck = 0x04,
    Disconnect = 0x05,

    // Eventos y Estados del Lector NFC (0x10 - 0x1F)
    TagAttached = 0x10,
    TagDetached = 0x11,
    ReaderStatus = 0x12,

    // Transcepción de Datos APDU / Raw (0x20 - 0x2F)
    TransceiveReq = 0x20,
    TransceiveResp = 0x21,

    // Estado del Dispositivo y Prioridad (0x30 - 0x4F)
    DeviceState = 0x30,
    UsbPriorityHint = 0x40,
    SwitchTransport = 0x41,
    ProfileRevoked = 0x42,

    // Errores Genéricos (0xF0 - 0xFF)
    ErrorFrame = 0xFF,
}

impl MessageType {
    pub fn from_u8(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::HelloSyn),
            0x02 => Some(Self::HelloAck),
            0x03 => Some(Self::Heartbeat),
            0x04 => Some(Self::HeartbeatAck),
            0x05 => Some(Self::Disconnect),
            0x10 => Some(Self::TagAttached),
            0x11 => Some(Self::TagDetached),
            0x12 => Some(Self::ReaderStatus),
            0x20 => Some(Self::TransceiveReq),
            0x21 => Some(Self::TransceiveResp),
            0x30 => Some(Self::DeviceState),
            0x40 => Some(Self::UsbPriorityHint),
            0x41 => Some(Self::SwitchTransport),
            0x42 => Some(Self::ProfileRevoked),
            0xFF => Some(Self::ErrorFrame),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransceiveStatus {
    Success = 0x00,
    Timeout = 0x01,
    TransceiveFailed = 0x02,
    TagLost = 0x03,
    UnsupportedTechnology = 0x04,
}

impl TransceiveStatus {
    pub fn from_u8(code: u8) -> Self {
        match code {
            0x00 => Self::Success,
            0x01 => Self::Timeout,
            0x02 => Self::TransceiveFailed,
            0x03 => Self::TagLost,
            0x04 => Self::UnsupportedTechnology,
            _ => Self::TransceiveFailed,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

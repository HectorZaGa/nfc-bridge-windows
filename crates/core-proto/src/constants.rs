//! Constantes fundamentales del protocolo binario NFC-Bridge (v1).

pub const MAGIC_BYTE_0: u8 = 0x4E; // 'N'
pub const MAGIC_BYTE_1: u8 = 0x42; // 'B'
pub const PROTOCOL_VERSION: u8 = 0x01;

pub const HEADER_SIZE: usize = 8;
pub const CRC_SIZE: usize = 2;
pub const OVERHEAD_SIZE: usize = HEADER_SIZE + CRC_SIZE; // 10 bytes
pub const MAX_PAYLOAD_SIZE: usize = 65535;

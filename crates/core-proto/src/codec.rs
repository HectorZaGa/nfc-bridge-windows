use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::constants::{
    CRC_SIZE, HEADER_SIZE, MAGIC_BYTE_0, MAGIC_BYTE_1, OVERHEAD_SIZE, PROTOCOL_VERSION,
};
use crate::crc16;
use crate::frame::NfcBridgeFrame;
use crate::message_type::MessageType;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("Trama incompleta: se requieren al menos {0} bytes, disponibles {1}")]
    Incomplete(usize, usize),
    #[error("Magic bytes inválidos: 0x{0:02X} 0x{1:02X}")]
    InvalidMagic(u8, u8),
    #[error("Versión no soportada: 0x{0:02X}")]
    UnsupportedVersion(u8),
    #[error("Tipo de mensaje desconocido: 0x{0:02X}")]
    UnknownMessageType(u8),
    #[error("CRC-16 inválido: esperado 0x{expected:04X}, calculado 0x{calculated:04X}")]
    CrcMismatch { expected: u16, calculated: u16 },
}

pub struct FrameCodec;

impl FrameCodec {
    /// Codifica una trama a su representación binaria lista para transmitir.
    /// Formato: [Magic: 2B][Version: 1B][Type: 1B][Seq: 2B][Length: 2B][Payload: NB][CRC: 2B]
    pub fn encode(frame: &NfcBridgeFrame) -> Bytes {
        let payload_len = frame.payload.len();
        let total_size = HEADER_SIZE + payload_len + CRC_SIZE;
        let mut buf = BytesMut::with_capacity(total_size);

        // Header (8 bytes)
        buf.put_u8(MAGIC_BYTE_0);
        buf.put_u8(MAGIC_BYTE_1);
        buf.put_u8(frame.version);
        buf.put_u8(frame.message_type.to_u8());
        buf.put_u16(frame.sequence_id);
        buf.put_u16(payload_len as u16);

        // Payload
        if payload_len > 0 {
            buf.put_slice(&frame.payload);
        }

        // CRC-16 calculado sobre Header + Payload
        let calculated_crc = crc16::compute(&buf[..HEADER_SIZE + payload_len]);
        buf.put_u16(calculated_crc);

        buf.freeze()
    }

    /// Intenta decodificar una trama a partir de un buffer de bytes.
    pub fn decode(src: &mut BytesMut) -> Result<Option<NfcBridgeFrame>, CodecError> {
        if src.len() < OVERHEAD_SIZE {
            return Ok(None);
        }

        // 1. Validar Magic Bytes
        if src[0] != MAGIC_BYTE_0 || src[1] != MAGIC_BYTE_1 {
            return Err(CodecError::InvalidMagic(src[0], src[1]));
        }

        // 2. Validar versión
        let version = src[2];
        if version != PROTOCOL_VERSION {
            return Err(CodecError::UnsupportedVersion(version));
        }

        // 3. Validar tipo de mensaje
        let type_code = src[3];
        let message_type = MessageType::from_u8(type_code)
            .ok_or(CodecError::UnknownMessageType(type_code))?;

        // 4. Longitud de payload
        let payload_len = u16::from_be_bytes([src[6], src[7]]) as usize;
        let total_frame_len = HEADER_SIZE + payload_len + CRC_SIZE;

        if src.len() < total_frame_len {
            // Se necesitan más bytes
            return Ok(None);
        }

        // 5. Validar CRC
        let expected_crc = u16::from_be_bytes([
            src[HEADER_SIZE + payload_len],
            src[HEADER_SIZE + payload_len + 1],
        ]);
        let calculated_crc = crc16::compute(&src[..HEADER_SIZE + payload_len]);

        if expected_crc != calculated_crc {
            return Err(CodecError::CrcMismatch {
                expected: expected_crc,
                calculated: calculated_crc,
            });
        }

        // 6. Extraer frame
        let sequence_id = u16::from_be_bytes([src[4], src[5]]);
        src.advance(HEADER_SIZE);
        let payload = src.split_to(payload_len).freeze();
        src.advance(CRC_SIZE);

        Ok(Some(NfcBridgeFrame {
            version,
            message_type,
            sequence_id,
            payload,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original_frame = NfcBridgeFrame::new(
            MessageType::HelloSyn,
            42,
            Bytes::from_static(b"HELLO-NFC-BRIDGE"),
        );

        let encoded = FrameCodec::encode(&original_frame);
        let mut buf = BytesMut::from(&encoded[..]);

        let decoded = FrameCodec::decode(&mut buf).unwrap().expect("Debe decodificar");
        assert_eq!(decoded.version, original_frame.version);
        assert_eq!(decoded.message_type, original_frame.message_type);
        assert_eq!(decoded.sequence_id, original_frame.sequence_id);
        assert_eq!(decoded.payload, original_frame.payload);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = BytesMut::from(&[0x00, 0x01, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00][..]);
        assert!(matches!(FrameCodec::decode(&mut buf), Err(CodecError::InvalidMagic(0x00, 0x01))));
    }
}

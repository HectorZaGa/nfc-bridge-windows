//! # Core Proto: Protocolo Binario BAP (Bridge APDU Protocol) v1
//!
//! Framing binario con comprobación CRC-16 CCITT, compatible 1:1 con la app Android.

pub mod codec;
pub mod constants;
pub mod crc16;
pub mod frame;
pub mod message_type;

pub use codec::{CodecError, FrameCodec};
pub use constants::*;
pub use frame::NfcBridgeFrame;
pub use message_type::{MessageType, TransceiveStatus};

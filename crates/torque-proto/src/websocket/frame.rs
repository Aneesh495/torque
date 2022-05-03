//! WebSocket (RFC 6455) frame structures, framing codec, and payload unmasking.

use bytes::Bytes;
use torque_core::error::{Result, TorqueError};

/// WebSocket frame opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x0 => Some(Opcode::Continuation),
            0x1 => Some(Opcode::Text),
            0x2 => Some(Opcode::Binary),
            0x8 => Some(Opcode::Close),
            0x9 => Some(Opcode::Ping),
            0xA => Some(Opcode::Pong),
            _ => None,
        }
    }

    pub fn is_control(&self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

/// Assembled WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsFrame {
    pub fin: bool,
    pub rsv: u8,
    pub opcode: Opcode,
    pub mask: Option<[u8; 4]>,
    pub payload: Bytes,
}

impl WsFrame {
    /// Construct a text frame.
    pub fn text(payload: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            rsv: 0,
            opcode: Opcode::Text,
            mask: None,
            payload: payload.into(),
        }
    }

    /// Construct a binary frame.
    pub fn binary(payload: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            rsv: 0,
            opcode: Opcode::Binary,
            mask: None,
            payload: payload.into(),
        }
    }

    /// Construct a ping frame.
    pub fn ping(payload: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            rsv: 0,
            opcode: Opcode::Ping,
            mask: None,
            payload: payload.into(),
        }
    }

    /// Construct a pong frame.
    pub fn pong(payload: impl Into<Bytes>) -> Self {
        Self {
            fin: true,
            rsv: 0,
            opcode: Opcode::Pong,
            mask: None,
            payload: payload.into(),
        }
    }

    /// Construct a close frame.
    pub fn close(code: u16, reason: &str) -> Self {
        let mut payload = Vec::with_capacity(2 + reason.len());
        payload.extend_from_slice(&code.to_be_bytes());
        payload.extend_from_slice(reason.as_bytes());
        Self {
            fin: true,
            rsv: 0,
            opcode: Opcode::Close,
            mask: None,
            payload: Bytes::from(payload),
        }
    }

    /// Parse a WebSocket frame from raw bytes.
    /// Returns `Ok(Some((frame, consumed)))` if a complete frame was parsed,
    /// or `Ok(None)` if more data is required.
    pub fn parse(src: &[u8]) -> Result<Option<(Self, usize)>> {
        if src.len() < 2 {
            return Ok(None);
        }

        let byte0 = src[0];
        let fin = (byte0 & 0x80) != 0;
        let rsv = (byte0 >> 4) & 0x07;
        let opcode_raw = byte0 & 0x0F;
        let opcode = Opcode::from_u8(opcode_raw).ok_or_else(|| TorqueError::WebSocket {
            detail: format!("Invalid opcode: 0x{opcode_raw:x}"),
        })?;

        let byte1 = src[1];
        let is_masked = (byte1 & 0x80) != 0;
        let len_indicator = (byte1 & 0x7F) as usize;

        let mut cursor = 2;

        let payload_len: usize = match len_indicator {
            0..=125 => len_indicator,
            126 => {
                if src.len() < cursor + 2 {
                    return Ok(None);
                }
                let len = u16::from_be_bytes([src[cursor], src[cursor + 1]]) as usize;
                cursor += 2;
                len
            }
            127 => {
                if src.len() < cursor + 8 {
                    return Ok(None);
                }
                let len = u64::from_be_bytes(src[cursor..cursor + 8].try_into().unwrap()) as usize;
                cursor += 8;
                len
            }
            _ => unreachable!(),
        };

        let mask = if is_masked {
            if src.len() < cursor + 4 {
                return Ok(None);
            }
            let mut m = [0u8; 4];
            m.copy_from_slice(&src[cursor..cursor + 4]);
            cursor += 4;
            Some(m)
        } else {
            None
        };

        if src.len() < cursor + payload_len {
            return Ok(None);
        }

        let mut payload_bytes = src[cursor..cursor + payload_len].to_vec();
        cursor += payload_len;

        // Unmask payload in-place if mask is present
        if let Some(m) = mask {
            apply_mask(&mut payload_bytes, &m);
        }

        let frame = Self {
            fin,
            rsv,
            opcode,
            mask,
            payload: Bytes::from(payload_bytes),
        };

        Ok(Some((frame, cursor)))
    }

    /// Serialize WebSocket frame into byte buffer.
    pub fn encode(&self, dst: &mut Vec<u8>) {
        let mut byte0 = if self.fin { 0x80 } else { 0x00 };
        byte0 |= (self.rsv & 0x07) << 4;
        byte0 |= (self.opcode as u8) & 0x0F;
        dst.push(byte0);

        let len = self.payload.len();
        let mask_bit = if self.mask.is_some() { 0x80 } else { 0x00 };

        if len <= 125 {
            dst.push(mask_bit | (len as u8));
        } else if len <= 0xFFFF {
            dst.push(mask_bit | 126);
            dst.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            dst.push(mask_bit | 127);
            dst.extend_from_slice(&(len as u64).to_be_bytes());
        }

        if let Some(m) = self.mask {
            dst.extend_from_slice(&m);
            let mut masked = self.payload.to_vec();
            apply_mask(&mut masked, &m);
            dst.extend_from_slice(&masked);
        } else {
            dst.extend_from_slice(&self.payload);
        }
    }
}

/// Apply 4-octet cyclical XOR mask in-place.
#[inline]
pub fn apply_mask(payload: &mut [u8], mask: &[u8; 4]) {
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}

//! HPACK header block decoder (RFC 7541 Section 5 and 6).

use torque_core::error::{Result, TorqueError};

use super::huffman::huffman_decode;
use super::table::{HeaderField, HeaderTable};

/// Decodes HPACK-compressed header blocks into key-value pairs.
pub struct HpackDecoder {
    table: HeaderTable,
}

impl HpackDecoder {
    /// Create a new decoder with default 4096-byte dynamic table size.
    pub fn new() -> Self {
        Self::with_max_table_size(4096)
    }

    /// Create a new decoder with custom dynamic table size.
    pub fn with_max_table_size(max_size: usize) -> Self {
        Self {
            table: HeaderTable::new(max_size),
        }
    }

    /// Decode an entire HPACK header block into header fields.
    pub fn decode(&mut self, src: &[u8]) -> Result<Vec<HeaderField>> {
        let mut fields = Vec::new();
        let mut cursor = 0;

        while cursor < src.len() {
            let first = src[cursor];

            if (first & 0x80) != 0 {
                // 1xxxxxxx: Indexed Header Field Representation (Section 6.1)
                let idx = decode_integer(src, &mut cursor, 7)?;
                let field = self.table.get(idx).ok_or_else(|| {
                    TorqueError::hpack(format!("Indexed header out of bounds: {}", idx))
                })?;
                fields.push(field);
            } else if (first & 0x40) != 0 {
                // 01xxxxxx: Literal Header Field with Incremental Indexing (Section 6.2.1)
                let idx = decode_integer(src, &mut cursor, 6)?;
                let (name, value) = if idx == 0 {
                    let name = decode_string(src, &mut cursor)?;
                    let value = decode_string(src, &mut cursor)?;
                    (name, value)
                } else {
                    let field = self.table.get(idx).ok_or_else(|| {
                        TorqueError::hpack(format!("Name index out of bounds: {}", idx))
                    })?;
                    let value = decode_string(src, &mut cursor)?;
                    (field.name, value)
                };

                self.table.dynamic.insert(name.clone(), value.clone());
                fields.push(HeaderField::new(name, value));
            } else if (first & 0x20) != 0 {
                // 001xxxxx: Dynamic Table Size Update (Section 6.3)
                let new_size = decode_integer(src, &mut cursor, 5)?;
                self.table.dynamic.set_max_size(new_size);
            } else {
                // 0000xxxx or 0001xxxx: Literal without Indexing or Never Indexed (Section 6.2.2, 6.2.3)
                let idx = decode_integer(src, &mut cursor, 4)?;
                let (name, value) = if idx == 0 {
                    let name = decode_string(src, &mut cursor)?;
                    let value = decode_string(src, &mut cursor)?;
                    (name, value)
                } else {
                    let field = self.table.get(idx).ok_or_else(|| {
                        TorqueError::hpack(format!("Name index out of bounds: {}", idx))
                    })?;
                    let value = decode_string(src, &mut cursor)?;
                    (field.name, value)
                };
                fields.push(HeaderField::new(name, value));
            }
        }

        Ok(fields)
    }
}

impl Default for HpackDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode an HPACK variable-length integer with N-bit prefix (Section 5.1).
pub fn decode_integer(src: &[u8], cursor: &mut usize, prefix_bits: u8) -> Result<usize> {
    if *cursor >= src.len() {
        return Err(TorqueError::hpack("Unexpected EOF decoding integer"));
    }

    let mask = (1usize << prefix_bits) - 1;
    let mut val = (src[*cursor] as usize) & mask;
    *cursor += 1;

    if val < mask {
        return Ok(val);
    }

    let mut shift = 0;
    while *cursor < src.len() {
        let byte = src[*cursor] as usize;
        *cursor += 1;
        val += (byte & 0x7F) << shift;
        if (byte & 0x80) == 0 {
            return Ok(val);
        }
        shift += 7;
        if shift > 64 {
            return Err(TorqueError::hpack("HPACK integer overflow"));
        }
    }

    Err(TorqueError::hpack("Incomplete multi-byte HPACK integer"))
}

/// Decode an HPACK string literal, with or without Huffman coding (Section 5.2).
pub fn decode_string(src: &[u8], cursor: &mut usize) -> Result<String> {
    if *cursor >= src.len() {
        return Err(TorqueError::hpack("Unexpected EOF decoding string"));
    }

    let is_huffman = (src[*cursor] & 0x80) != 0;
    let len = decode_integer(src, cursor, 7)?;

    if *cursor + len > src.len() {
        return Err(TorqueError::hpack("String length exceeds buffer"));
    }

    let raw = &src[*cursor..*cursor + len];
    *cursor += len;

    let bytes = if is_huffman {
        huffman_decode(raw)?
    } else {
        raw.to_vec()
    };

    String::from_utf8(bytes).map_err(|e| TorqueError::protocol_with_source("Invalid header string UTF-8", e))
}

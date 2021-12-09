//! HPACK header block encoder (RFC 7541 Section 5 and 6).

use super::huffman::huffman_encode;
use super::table::HeaderTable;

/// Encodes header fields into compressed HPACK byte representations.
pub struct HpackEncoder {
    table: HeaderTable,
}

impl HpackEncoder {
    /// Create a new encoder with default 4096-byte dynamic table size.
    pub fn new() -> Self {
        Self::with_max_table_size(4096)
    }

    /// Create a new encoder with custom dynamic table size.
    pub fn with_max_table_size(max_size: usize) -> Self {
        Self {
            table: HeaderTable::new(max_size),
        }
    }

    /// Encode a slice of header fields into a target byte buffer.
    pub fn encode<'a, I>(&mut self, headers: I, dst: &mut Vec<u8>)
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        for (name, value) in headers {
            if let Some((idx, exact)) = self.table.find(name, value) {
                if exact {
                    // Indexed Header Field Representation: 1xxxxxxx
                    encode_integer(idx, 7, 0x80, dst);
                } else {
                    // Literal with Incremental Indexing - Indexed Name: 01xxxxxx
                    encode_integer(idx, 6, 0x40, dst);
                    encode_string(value, true, dst);
                    self.table.dynamic.insert(name.to_string(), value.to_string());
                }
            } else {
                // Literal with Incremental Indexing - New Name: 01000000
                dst.push(0x40);
                encode_string(name, true, dst);
                encode_string(value, true, dst);
                self.table.dynamic.insert(name.to_string(), value.to_string());
            }
        }
    }
}

impl Default for HpackEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode an integer into N-bit prefix format (Section 5.1).
pub fn encode_integer(mut value: usize, prefix_bits: u8, prefix_mask: u8, dst: &mut Vec<u8>) {
    let max_prefix = (1usize << prefix_bits) - 1;
    if value < max_prefix {
        dst.push(prefix_mask | (value as u8));
    } else {
        dst.push(prefix_mask | (max_prefix as u8));
        value -= max_prefix;
        while value >= 128 {
            dst.push(((value % 128) as u8) | 0x80);
            value /= 128;
        }
        dst.push(value as u8);
    }
}

/// Encode a string literal with optional Huffman coding (Section 5.2).
pub fn encode_string(s: &str, use_huffman: bool, dst: &mut Vec<u8>) {
    let bytes = s.as_bytes();
    if use_huffman {
        let mut encoded = Vec::new();
        huffman_encode(bytes, &mut encoded);
        if encoded.len() < bytes.len() {
            // Huffman flag = 1 (0x80)
            encode_integer(encoded.len(), 7, 0x80, dst);
            dst.extend_from_slice(&encoded);
            return;
        }
    }

    // Uncompressed string literal (Huffman flag = 0)
    encode_integer(bytes.len(), 7, 0x00, dst);
    dst.extend_from_slice(bytes);
}

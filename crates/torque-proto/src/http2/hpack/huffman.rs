//! Canonical Huffman code encoder and decoder for HPACK (RFC 7541 Appendix B).

use torque_core::error::{Result, TorqueError};

/// Huffman code entry: (bit_pattern, bit_length).
#[derive(Clone, Copy)]
pub struct HuffmanCode {
    pub bits: u32,
    pub len: u8,
}

/// Static Huffman code table defined in RFC 7541 Appendix B.
pub static HUFFMAN_CODES: &[HuffmanCode] = &[
    HuffmanCode { bits: 0x1ff8, len: 13 },
    HuffmanCode { bits: 0x7fffd8, len: 23 },
    HuffmanCode { bits: 0xfffffe2, len: 28 },
    HuffmanCode { bits: 0xfffffe3, len: 28 },
    HuffmanCode { bits: 0xfffffe4, len: 28 },
    HuffmanCode { bits: 0xfffffe5, len: 28 },
    HuffmanCode { bits: 0xfffffe6, len: 28 },
    HuffmanCode { bits: 0xfffffe7, len: 28 },
    HuffmanCode { bits: 0xfffffe8, len: 28 },
    HuffmanCode { bits: 0xffffea, len: 24 },
    HuffmanCode { bits: 0x3ffffffc, len: 30 },
    HuffmanCode { bits: 0xfffffe9, len: 28 },
    HuffmanCode { bits: 0xfffffea, len: 28 },
    HuffmanCode { bits: 0x3ffffffd, len: 30 },
    HuffmanCode { bits: 0xfffffeb, len: 28 },
    HuffmanCode { bits: 0xfffffec, len: 28 },
    HuffmanCode { bits: 0xfffffed, len: 28 },
    HuffmanCode { bits: 0xfffffee, len: 28 },
    HuffmanCode { bits: 0xfffffef, len: 28 },
    HuffmanCode { bits: 0xffffff0, len: 28 },
    HuffmanCode { bits: 0xffffff1, len: 28 },
    HuffmanCode { bits: 0xffffff2, len: 28 },
    HuffmanCode { bits: 0x3ffffffe, len: 30 },
    HuffmanCode { bits: 0xffffff3, len: 28 },
    HuffmanCode { bits: 0xffffff4, len: 28 },
    HuffmanCode { bits: 0xffffff5, len: 28 },
    HuffmanCode { bits: 0xffffff6, len: 28 },
    HuffmanCode { bits: 0xffffff7, len: 28 },
    HuffmanCode { bits: 0xffffff8, len: 28 },
    HuffmanCode { bits: 0xffffff9, len: 28 },
    HuffmanCode { bits: 0xffffffa, len: 28 },
    HuffmanCode { bits: 0xffffffb, len: 28 },
    HuffmanCode { bits: 0x14, len: 6 },
    HuffmanCode { bits: 0x3f8, len: 10 },
    HuffmanCode { bits: 0x3f9, len: 10 },
    HuffmanCode { bits: 0xffa, len: 12 },
    HuffmanCode { bits: 0x1ff9, len: 13 },
    HuffmanCode { bits: 0x15, len: 6 },
    HuffmanCode { bits: 0xf8, len: 8 },
    HuffmanCode { bits: 0x7fa, len: 11 },
    HuffmanCode { bits: 0x3fa, len: 10 },
    HuffmanCode { bits: 0x3fb, len: 10 },
    HuffmanCode { bits: 0xf9, len: 8 },
    HuffmanCode { bits: 0x7fb, len: 11 },
    HuffmanCode { bits: 0xfa, len: 8 },
    HuffmanCode { bits: 0x16, len: 6 },
    HuffmanCode { bits: 0x17, len: 6 },
    HuffmanCode { bits: 0x18, len: 6 },
    HuffmanCode { bits: 0x0, len: 5 },
    HuffmanCode { bits: 0x1, len: 5 },
    HuffmanCode { bits: 0x2, len: 5 },
    HuffmanCode { bits: 0x19, len: 6 },
    HuffmanCode { bits: 0x1a, len: 6 },
    HuffmanCode { bits: 0x1b, len: 6 },
    HuffmanCode { bits: 0x1c, len: 6 },
    HuffmanCode { bits: 0x1d, len: 6 },
    HuffmanCode { bits: 0x1e, len: 6 },
    HuffmanCode { bits: 0x1f, len: 6 },
    HuffmanCode { bits: 0x5c, len: 7 },
    HuffmanCode { bits: 0xfb, len: 8 },
    HuffmanCode { bits: 0x7ffc, len: 15 },
    HuffmanCode { bits: 0x20, len: 6 },
    HuffmanCode { bits: 0xffb, len: 12 },
    HuffmanCode { bits: 0x3fc, len: 10 },
    HuffmanCode { bits: 0x1ffa, len: 13 },
    HuffmanCode { bits: 0x21, len: 6 },
    HuffmanCode { bits: 0x5d, len: 7 },
    HuffmanCode { bits: 0x5e, len: 7 },
    HuffmanCode { bits: 0x5f, len: 7 },
    HuffmanCode { bits: 0x60, len: 7 },
    HuffmanCode { bits: 0x61, len: 7 },
    HuffmanCode { bits: 0x62, len: 7 },
    HuffmanCode { bits: 0x63, len: 7 },
    HuffmanCode { bits: 0x64, len: 7 },
    HuffmanCode { bits: 0x65, len: 7 },
    HuffmanCode { bits: 0x66, len: 7 },
    HuffmanCode { bits: 0x67, len: 7 },
    HuffmanCode { bits: 0x68, len: 7 },
    HuffmanCode { bits: 0x69, len: 7 },
    HuffmanCode { bits: 0x6a, len: 7 },
    HuffmanCode { bits: 0x6b, len: 7 },
    HuffmanCode { bits: 0x6c, len: 7 },
    HuffmanCode { bits: 0x6d, len: 7 },
    HuffmanCode { bits: 0x6e, len: 7 },
    HuffmanCode { bits: 0x6f, len: 7 },
    HuffmanCode { bits: 0x70, len: 7 },
    HuffmanCode { bits: 0x71, len: 7 },
    HuffmanCode { bits: 0x72, len: 7 },
    HuffmanCode { bits: 0xfc, len: 8 },
    HuffmanCode { bits: 0x73, len: 7 },
    HuffmanCode { bits: 0xfd, len: 8 },
    HuffmanCode { bits: 0x1ffb, len: 13 },
    HuffmanCode { bits: 0x7fff0, len: 19 },
    HuffmanCode { bits: 0x1ffc, len: 13 },
    HuffmanCode { bits: 0x3ffc, len: 14 },
    HuffmanCode { bits: 0x22, len: 6 },
    HuffmanCode { bits: 0x7ffd9, len: 19 },
    HuffmanCode { bits: 0x3, len: 5 },
    HuffmanCode { bits: 0x23, len: 6 },
    HuffmanCode { bits: 0x4, len: 5 },
    HuffmanCode { bits: 0x24, len: 6 },
    HuffmanCode { bits: 0x5, len: 5 },
    HuffmanCode { bits: 0x25, len: 6 },
    HuffmanCode { bits: 0x26, len: 6 },
    HuffmanCode { bits: 0x27, len: 6 },
    HuffmanCode { bits: 0x6, len: 5 },
    HuffmanCode { bits: 0x74, len: 7 },
    HuffmanCode { bits: 0x75, len: 7 },
    HuffmanCode { bits: 0x28, len: 6 },
    HuffmanCode { bits: 0x29, len: 6 },
    HuffmanCode { bits: 0x2a, len: 6 },
    HuffmanCode { bits: 0x7, len: 5 },
    HuffmanCode { bits: 0x2b, len: 6 },
    HuffmanCode { bits: 0x76, len: 7 },
    HuffmanCode { bits: 0x2c, len: 6 },
    HuffmanCode { bits: 0x8, len: 5 },
    HuffmanCode { bits: 0x9, len: 5 },
    HuffmanCode { bits: 0x2d, len: 6 },
    HuffmanCode { bits: 0x77, len: 7 },
    HuffmanCode { bits: 0x78, len: 7 },
    HuffmanCode { bits: 0x79, len: 7 },
    HuffmanCode { bits: 0x7a, len: 7 },
    HuffmanCode { bits: 0x7b, len: 7 },
    HuffmanCode { bits: 0x7ffe, len: 15 },
    HuffmanCode { bits: 0x7fc, len: 11 },
    HuffmanCode { bits: 0x3ffd, len: 14 },
    HuffmanCode { bits: 0x1ffd, len: 13 },
    HuffmanCode { bits: 0xffffffc, len: 28 },
    HuffmanCode { bits: 0xfffe6, len: 20 },
    HuffmanCode { bits: 0x3fffd2, len: 22 },
    HuffmanCode { bits: 0xfffe7, len: 20 },
    HuffmanCode { bits: 0xfffe8, len: 20 },
    HuffmanCode { bits: 0x3fffd3, len: 22 },
    HuffmanCode { bits: 0x3fffd4, len: 22 },
    HuffmanCode { bits: 0x3fffd5, len: 22 },
    HuffmanCode { bits: 0x7fffd9, len: 23 },
    HuffmanCode { bits: 0x3fffd6, len: 22 },
    HuffmanCode { bits: 0x7fffda, len: 23 },
    HuffmanCode { bits: 0x7fffdb, len: 23 },
    HuffmanCode { bits: 0x7fffdc, len: 23 },
    HuffmanCode { bits: 0x7fffdd, len: 23 },
    HuffmanCode { bits: 0x7fffde, len: 23 },
    HuffmanCode { bits: 0xffffeb, len: 24 },
    HuffmanCode { bits: 0x7fffdf, len: 23 },
    HuffmanCode { bits: 0xffffec, len: 24 },
    HuffmanCode { bits: 0xffffed, len: 24 },
    HuffmanCode { bits: 0x3fffd7, len: 22 },
    HuffmanCode { bits: 0x7fffe0, len: 23 },
    HuffmanCode { bits: 0xffffee, len: 24 },
    HuffmanCode { bits: 0x7fffe1, len: 23 },
    HuffmanCode { bits: 0x7fffe2, len: 23 },
    HuffmanCode { bits: 0x7fffe3, len: 23 },
    HuffmanCode { bits: 0x7fffe4, len: 23 },
    HuffmanCode { bits: 0x1fffdc, len: 21 },
    HuffmanCode { bits: 0x3fffd8, len: 22 },
    HuffmanCode { bits: 0x7fffe5, len: 23 },
    HuffmanCode { bits: 0x3fffd9, len: 22 },
    HuffmanCode { bits: 0x7fffe6, len: 23 },
    HuffmanCode { bits: 0x7fffe7, len: 23 },
    HuffmanCode { bits: 0xffffef, len: 24 },
    HuffmanCode { bits: 0x3fffda, len: 22 },
    HuffmanCode { bits: 0x1fffdd, len: 21 },
    HuffmanCode { bits: 0xfffe9, len: 20 },
    HuffmanCode { bits: 0x3fffdb, len: 22 },
    HuffmanCode { bits: 0x3fffdc, len: 22 },
    HuffmanCode { bits: 0x7fffe8, len: 23 },
    HuffmanCode { bits: 0x7fffe9, len: 23 },
    HuffmanCode { bits: 0x1fffde, len: 21 },
    HuffmanCode { bits: 0x7fffea, len: 23 },
    HuffmanCode { bits: 0x3fffdd, len: 22 },
    HuffmanCode { bits: 0x3fffde, len: 22 },
    HuffmanCode { bits: 0xffffffd, len: 28 },
    HuffmanCode { bits: 0xffffff, len: 24 },
    HuffmanCode { bits: 0xfffe0, len: 20 },
    HuffmanCode { bits: 0x1fffdf, len: 21 },
    HuffmanCode { bits: 0x3fffdf, len: 22 },
    HuffmanCode { bits: 0x7fffeb, len: 23 },
    HuffmanCode { bits: 0x7fffec, len: 23 },
    HuffmanCode { bits: 0x1fffe0, len: 21 },
    HuffmanCode { bits: 0x1fffe1, len: 21 },
    HuffmanCode { bits: 0x3fffe0, len: 22 },
    HuffmanCode { bits: 0x1fffe2, len: 21 },
    HuffmanCode { bits: 0x7fffed, len: 23 },
    HuffmanCode { bits: 0x3fffe1, len: 22 },
    HuffmanCode { bits: 0x7fffee, len: 23 },
    HuffmanCode { bits: 0x7fffef, len: 23 },
    HuffmanCode { bits: 0xfffe1, len: 20 },
    HuffmanCode { bits: 0x3fffe2, len: 22 },
    HuffmanCode { bits: 0x3fffe3, len: 22 },
    HuffmanCode { bits: 0x3fffe4, len: 22 },
    HuffmanCode { bits: 0x7ffff0, len: 23 },
    HuffmanCode { bits: 0x3fffe5, len: 22 },
    HuffmanCode { bits: 0x3fffe6, len: 22 },
    HuffmanCode { bits: 0x7ffff1, len: 23 },
    HuffmanCode { bits: 0x3ffffff0, len: 30 },
    HuffmanCode { bits: 0x3ffffff1, len: 30 },
    HuffmanCode { bits: 0x3fffe7, len: 22 },
    HuffmanCode { bits: 0x7ffff2, len: 23 },
    HuffmanCode { bits: 0x3fffe8, len: 22 },
    HuffmanCode { bits: 0x1ffffec, len: 25 },
    HuffmanCode { bits: 0x3ffffff2, len: 30 },
    HuffmanCode { bits: 0x3fffe9, len: 22 },
    HuffmanCode { bits: 0x3fffea, len: 22 },
    HuffmanCode { bits: 0x7ffff3, len: 23 },
    HuffmanCode { bits: 0x3fffeb, len: 22 },
    HuffmanCode { bits: 0x7ffff4, len: 23 },
    HuffmanCode { bits: 0x3fffec, len: 22 },
    HuffmanCode { bits: 0x7ffff5, len: 23 },
    HuffmanCode { bits: 0x3fffed, len: 22 },
    HuffmanCode { bits: 0x7ffff6, len: 23 },
    HuffmanCode { bits: 0x7ffff7, len: 23 },
    HuffmanCode { bits: 0x7ffff8, len: 23 },
    HuffmanCode { bits: 0x7ffff9, len: 23 },
    HuffmanCode { bits: 0x7ffffa, len: 23 },
    HuffmanCode { bits: 0x7ffffb, len: 23 },
    HuffmanCode { bits: 0x7ffffc, len: 23 },
    HuffmanCode { bits: 0x1ffffed, len: 25 },
    HuffmanCode { bits: 0x7ffffd, len: 23 },
    HuffmanCode { bits: 0x1ffffee, len: 25 },
    HuffmanCode { bits: 0x7ffffe, len: 23 },
    HuffmanCode { bits: 0x1ffffef, len: 25 },
    HuffmanCode { bits: 0x7fffff, len: 23 },
    HuffmanCode { bits: 0x3ffffff3, len: 30 },
    HuffmanCode { bits: 0x3ffffff4, len: 30 },
    HuffmanCode { bits: 0x3ffffff5, len: 30 },
    HuffmanCode { bits: 0x3ffffff6, len: 30 },
    HuffmanCode { bits: 0x3ffffff7, len: 30 },
    HuffmanCode { bits: 0x3ffffff8, len: 30 },
    HuffmanCode { bits: 0x3ffffff9, len: 30 },
    HuffmanCode { bits: 0x3ffffffa, len: 30 },
    HuffmanCode { bits: 0x3ffffffb, len: 30 },
    HuffmanCode { bits: 0x1fffee, len: 21 },
    HuffmanCode { bits: 0x7fffd0, len: 23 },
    HuffmanCode { bits: 0x7fffd1, len: 23 },
    HuffmanCode { bits: 0x3fffd0, len: 22 },
    HuffmanCode { bits: 0x3fffd1, len: 22 },
    HuffmanCode { bits: 0x1fffd1, len: 21 },
    HuffmanCode { bits: 0x7fffd2, len: 23 },
    HuffmanCode { bits: 0x7fffd3, len: 23 },
    HuffmanCode { bits: 0x7fffd4, len: 23 },
    HuffmanCode { bits: 0x7fffd5, len: 23 },
    HuffmanCode { bits: 0x7fffd6, len: 23 },
    HuffmanCode { bits: 0x7fffd7, len: 23 },
    HuffmanCode { bits: 0x7fffe, len: 19 },
    HuffmanCode { bits: 0x7ffff, len: 19 },
    HuffmanCode { bits: 0x3ffe, len: 14 },
    HuffmanCode { bits: 0x3fff, len: 14 },
    // EOS (index 256)
    HuffmanCode { bits: 0x3fffffff, len: 30 },
];

/// Encode a raw byte slice using the HPACK Huffman code table.
pub fn huffman_encode(input: &[u8], dst: &mut Vec<u8>) {
    let mut current: u64 = 0;
    let mut bits_left = 64;

    for &byte in input {
        let idx = byte as usize;
        let code = if idx < HUFFMAN_CODES.len() {
            HUFFMAN_CODES[idx]
        } else {
            HuffmanCode { bits: 0, len: 0 }
        };
        let n = code.len;
        let bits = code.bits as u64;

        if bits_left >= n {
            bits_left -= n;
            current |= bits << bits_left;
        } else {
            let overflow = n - bits_left;
            current |= bits >> overflow;
            dst.extend_from_slice(&current.to_be_bytes());
            bits_left = 64 - overflow;
            current = bits << bits_left;
        }
    }

    // Flush remaining partial byte with 1s padding
    if bits_left < 64 {
        let used_bits = 64 - bits_left;
        let pad = (8 - (used_bits % 8)) % 8;
        if pad > 0 {
            let padding_mask = (1u64 << pad) - 1;
            current |= padding_mask << (bits_left - pad);
            bits_left -= pad;
        }
        let bytes_to_flush = ((64 - bits_left) / 8) as usize;
        let be_bytes = current.to_be_bytes();
        dst.extend_from_slice(&be_bytes[..bytes_to_flush]);
    }
}

/// Node in binary trie for Huffman decoding.
struct TrieNode {
    children: [Option<Box<TrieNode>>; 2],
    symbol: Option<u8>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: [None, None],
            symbol: None,
        }
    }
}

/// Construct root decoder trie from static table.
fn build_huffman_trie() -> TrieNode {
    let mut root = TrieNode::new();
    for (sym, code) in HUFFMAN_CODES.iter().enumerate().take(256) {
        let mut curr = &mut root;
        for i in (0..code.len).rev() {
            let bit = ((code.bits >> i) & 1) as usize;
            if curr.children[bit].is_none() {
                curr.children[bit] = Some(Box::new(TrieNode::new()));
            }
            curr = curr.children[bit].as_mut().unwrap();
        }
        curr.symbol = Some(sym as u8);
    }
    root
}

/// Decode a Huffman-encoded byte slice back to plain bytes.
pub fn huffman_decode(src: &[u8]) -> Result<Vec<u8>> {
    static ROOT: std::sync::OnceLock<TrieNode> = std::sync::OnceLock::new();
    let root = ROOT.get_or_init(build_huffman_trie);

    let mut out = Vec::new();
    let mut curr = root;

    for &byte in src {
        for bit_idx in (0..8).rev() {
            let bit = ((byte >> bit_idx) & 1) as usize;
            curr = match curr.children[bit].as_ref() {
                Some(next) => next,
                None => {
                    return Err(TorqueError::hpack("Invalid Huffman bit pattern"));
                }
            };
            if let Some(sym) = curr.symbol {
                out.push(sym);
                curr = root;
            }
        }
    }

    Ok(out)
}

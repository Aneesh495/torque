//! HPACK (RFC 7541) header compression and decompression for HTTP/2.

pub mod decoder;
pub mod encoder;
pub mod huffman;
pub mod table;

pub use decoder::HpackDecoder;
pub use encoder::HpackEncoder;
pub use table::{DynamicTable, HeaderField, HeaderTable, STATIC_TABLE};

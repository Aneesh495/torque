//! # Torque Protocol Engine
//!
//! Zero-copy codecs and protocol state machines for HTTP/1.1, HTTP/2 (RFC 7540),
//! HPACK (RFC 7541), and WebSocket (RFC 6455).

pub mod http1;
pub mod http2;
pub mod websocket;

pub use http1::{HeaderMap, Http1Encoder, Http1Parser, Method, ParseResult, Request, Response, Version};
pub use http2::{
    DynamicTable, FlowControl, FrameHeader, FramePayload, FrameType, HeaderField, HeaderTable,
    HpackDecoder, HpackEncoder, Http2Frame, Http2Stream, StreamState,
};
pub use websocket::{compute_accept_key, create_upgrade_response, is_websocket_upgrade, Opcode, WsFrame};

/// Common protocol prelude.
pub mod prelude {
    pub use crate::http1::{HeaderMap, Method, ParseResult, Request, Response, Version};
    pub use crate::http2::{FrameType, HeaderField, Http2Frame, Http2Stream, StreamState};
    pub use crate::websocket::{Opcode, WsFrame};
}

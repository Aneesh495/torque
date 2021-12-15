//! WebSocket (RFC 6455) protocol support.

pub mod frame;
pub mod handshake;

pub use frame::{apply_mask, Opcode, WsFrame};
pub use handshake::{compute_accept_key, create_upgrade_response, is_websocket_upgrade, WS_GUID};

//! WebSocket RFC 6455 upgrade handshake validation and key computation.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use sha1::{Digest, Sha1};
use torque_core::error::{Result, TorqueError};

use crate::http1::types::{Request, Response};

/// RFC 6455 magic GUID constant.
pub const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Compute the Sec-WebSocket-Accept response value from a client's Sec-WebSocket-Key.
pub fn compute_accept_key(sec_websocket_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(sec_websocket_key.trim().as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let result = hasher.finalize();
    BASE64.encode(result)
}

/// Verify if an incoming HTTP request is a valid WebSocket upgrade request.
pub fn is_websocket_upgrade(req: &Request) -> bool {
    let has_upgrade_hdr = req
        .headers
        .get("upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let has_connection_hdr = req
        .headers
        .get("connection")
        .map(|v| {
            v.split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
        })
        .unwrap_or(false);

    let has_key = req.headers.contains_key("sec-websocket-key");

    has_upgrade_hdr && has_connection_hdr && has_key
}

/// Generate a 101 Switching Protocols response for a valid WebSocket upgrade.
pub fn create_upgrade_response(req: &Request) -> Result<Response> {
    if !is_websocket_upgrade(req) {
        return Err(TorqueError::WebSocket {
            detail: "Request is not a valid WebSocket upgrade".to_string(),
        });
    }

    let client_key = req.headers.get("sec-websocket-key").unwrap();
    let accept_key = compute_accept_key(client_key);

    let mut resp = Response::new(101);
    resp.reason = "Switching Protocols".to_string();
    resp.headers.insert("upgrade", "websocket");
    resp.headers.insert("connection", "Upgrade");
    resp.headers.insert("sec-websocket-accept", accept_key);

    Ok(resp)
}

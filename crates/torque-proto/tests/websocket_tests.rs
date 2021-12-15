use bytes::Bytes;
use torque_proto::websocket::{compute_accept_key, Opcode, WsFrame};

#[test]
fn test_ws_accept_key_computation() {
    // RFC 6455 Section 1.3 test vector:
    // Client key: "dGhlIHNhbXBsZSBub25jZQ=="
    // Expected accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let accept = compute_accept_key(client_key);
    assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[test]
fn test_ws_unmasked_text_frame_roundtrip() {
    let frame = WsFrame::text("Hello, WebSocket!");
    let mut buf = Vec::new();
    frame.encode(&mut buf);

    let (parsed, consumed) = WsFrame::parse(&buf).expect("parse ws failed").expect("complete frame");
    assert_eq!(consumed, buf.len());
    assert!(parsed.fin);
    assert_eq!(parsed.opcode, Opcode::Text);
    assert_eq!(parsed.payload.as_ref(), b"Hello, WebSocket!");
}

#[test]
fn test_ws_masked_binary_frame_roundtrip() {
    let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let mut frame = WsFrame::binary(Bytes::from(data.clone()));
    frame.mask = Some([0x12, 0x34, 0x56, 0x78]);

    let mut buf = Vec::new();
    frame.encode(&mut buf);

    let (parsed, consumed) = WsFrame::parse(&buf).expect("parse ws masked failed").expect("complete frame");
    assert_eq!(consumed, buf.len());
    assert_eq!(parsed.opcode, Opcode::Binary);
    assert_eq!(parsed.payload.as_ref(), &data[..]);
}

#[test]
fn test_ws_ping_pong_frames() {
    let ping = WsFrame::ping(Bytes::from_static(b"heartbeat"));
    assert_eq!(ping.opcode, Opcode::Ping);
    assert!(ping.opcode.is_control());

    let mut buf = Vec::new();
    ping.encode(&mut buf);

    let (parsed, _) = WsFrame::parse(&buf).expect("parse ping failed").unwrap();
    assert_eq!(parsed.opcode, Opcode::Ping);
    assert_eq!(parsed.payload.as_ref(), b"heartbeat");
}

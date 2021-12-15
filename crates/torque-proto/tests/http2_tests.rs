use bytes::Bytes;
use torque_proto::http2::{FlowControl, FrameHeader, FramePayload, FrameType, Http2Frame, Http2Stream, StreamState};

#[test]
fn test_frame_header_encode_decode() {
    let header = FrameHeader {
        length: 128,
        frame_type: FrameType::Headers,
        flags: 0x5, // END_STREAM | END_HEADERS
        stream_id: 1,
    };

    let encoded = header.encode();
    assert_eq!(encoded.len(), 9);

    let parsed = FrameHeader::parse(&encoded).expect("parse header failed");
    assert_eq!(parsed.length, 128);
    assert_eq!(parsed.frame_type, FrameType::Headers);
    assert_eq!(parsed.flags, 0x5);
    assert_eq!(parsed.stream_id, 1);
    assert!(parsed.has_end_stream());
    assert!(parsed.has_end_headers());
}

#[test]
fn test_data_frame_roundtrip() {
    let payload_data = Bytes::from_static(b"streaming chunk data for http2");
    let frame = Http2Frame::data(3, payload_data.clone(), true);

    let mut buf = Vec::new();
    frame.encode(&mut buf);

    let (parsed_frame, consumed) = Http2Frame::parse(&buf).expect("parse failed").expect("complete frame");
    assert_eq!(consumed, buf.len());
    assert_eq!(parsed_frame.header.stream_id, 3);
    assert_eq!(parsed_frame.header.frame_type, FrameType::Data);

    match parsed_frame.payload {
        FramePayload::Data { data, end_stream } => {
            assert_eq!(data, payload_data);
            assert!(end_stream);
        }
        _ => panic!("Expected Data payload"),
    }
}

#[test]
fn test_settings_frame_roundtrip() {
    let settings = vec![(0x1, 8192), (0x2, 0), (0x4, 131072)];
    let frame = Http2Frame::settings(settings.clone(), false);

    let mut buf = Vec::new();
    frame.encode(&mut buf);

    let (parsed_frame, _) = Http2Frame::parse(&buf).expect("parse settings failed").unwrap();
    match parsed_frame.payload {
        FramePayload::Settings { ack, settings: parsed_settings } => {
            assert!(!ack);
            assert_eq!(parsed_settings, settings);
        }
        _ => panic!("Expected Settings payload"),
    }
}

#[test]
fn test_stream_state_transitions() {
    let mut stream = Http2Stream::new(1);
    assert_eq!(stream.state, StreamState::Idle);

    // Client receives headers without end_stream -> Open
    assert!(stream.recv_headers(false).is_ok());
    assert_eq!(stream.state, StreamState::Open);

    // Client sends headers with end_stream -> HalfClosedLocal
    assert!(stream.send_headers(true).is_ok());
    assert_eq!(stream.state, StreamState::HalfClosedLocal);

    // Client receives data with end_stream -> Closed
    assert!(stream.recv_data(true).is_ok());
    assert_eq!(stream.state, StreamState::Closed);
    assert!(stream.is_closed());
}

#[test]
fn test_flow_control() {
    let mut fc = FlowControl::new(65535);
    assert_eq!(fc.available(), 65535);

    assert!(fc.consume(20000).is_ok());
    assert_eq!(fc.available(), 45535);
    assert!(fc.needs_update().is_none());

    // Consume down below 50%
    assert!(fc.consume(30000).is_ok());
    assert_eq!(fc.available(), 15535);
    let needed = fc.needs_update().expect("Should need update");
    assert_eq!(needed, 50000);

    // Apply update
    assert!(fc.update(needed).is_ok());
    assert_eq!(fc.available(), 65535);
}

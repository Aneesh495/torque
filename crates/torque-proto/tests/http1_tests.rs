use bytes::Bytes;
use torque_proto::http1::{Http1Encoder, Http1Parser, Method, ParseResult, Response, Version};

#[test]
fn test_http1_request_parse_basic() {
    let raw = b"GET /api/v1/users?page=2 HTTP/1.1\r\nHost: api.example.com\r\nUser-Agent: TorqueClient/1.0\r\nAccept: application/json\r\n\r\n";
    let res = Http1Parser::parse_request(raw).expect("parse failed");

    match res {
        ParseResult::Complete(req, consumed) => {
            assert_eq!(consumed, raw.len());
            assert_eq!(req.method, Method::Get);
            assert_eq!(req.uri, "/api/v1/users?page=2");
            assert_eq!(req.version, Version::Http11);
            assert_eq!(req.headers.get("host"), Some("api.example.com"));
            assert_eq!(req.headers.get("user-agent"), Some("TorqueClient/1.0"));
            assert_eq!(req.headers.get("accept"), Some("application/json"));
            assert!(req.body.is_empty());
        }
        ParseResult::Incomplete => panic!("Expected complete request"),
    }
}

#[test]
fn test_http1_request_with_body() {
    let payload = b"{\"username\":\"jane_street\",\"role\":\"quant\"}";
    let raw = format!(
        "POST /api/v1/auth HTTP/1.1\r\nHost: auth.internal\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        payload.len(),
        std::str::from_utf8(payload).unwrap()
    );

    let res = Http1Parser::parse_request(raw.as_bytes()).expect("parse failed");
    match res {
        ParseResult::Complete(req, consumed) => {
            assert_eq!(consumed, raw.len());
            assert_eq!(req.method, Method::Post);
            assert_eq!(req.body.as_ref(), payload);
        }
        ParseResult::Incomplete => panic!("Expected complete request"),
    }
}

#[test]
fn test_http1_chunked_body_decoding() {
    let raw = b"POST /upload HTTP/1.1\r\nHost: storage.internal\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\nF\r\n in \r\n\r\nchunks.\r\n0\r\n\r\n";

    let res = Http1Parser::parse_request(raw).expect("parse chunked failed");
    match res {
        ParseResult::Complete(req, consumed) => {
            assert_eq!(consumed, raw.len());
            assert_eq!(req.method, Method::Post);
            assert_eq!(req.body.as_ref(), b"Wikipedia in \r\n\r\nchunks.");
        }
        ParseResult::Incomplete => panic!("Expected complete chunked request"),
    }
}

#[test]
fn test_http1_response_encode_and_parse_roundtrip() {
    let mut resp = Response::new(200);
    resp.headers.insert("Content-Type", "text/plain");
    resp.headers.insert("Server", "Torque/1.0");
    resp.body = Bytes::from_static(b"Hello from reverse proxy");

    let mut buf = Vec::new();
    Http1Encoder::encode_response(&resp, &mut buf);

    let parsed = Http1Parser::parse_response(&buf).expect("parse encoded response failed");
    match parsed {
        ParseResult::Complete(r, consumed) => {
            assert_eq!(consumed, buf.len());
            assert_eq!(r.status, 200);
            assert_eq!(r.headers.get("content-type"), Some("text/plain"));
            assert_eq!(r.body.as_ref(), b"Hello from reverse proxy");
        }
        ParseResult::Incomplete => panic!("Expected complete response"),
    }
}

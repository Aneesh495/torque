//! HTTP/1.1 message serializers and chunked framing encoders.

use super::types::{Request, Response};

/// Serializer for HTTP/1.1 wire protocol formatting.
pub struct Http1Encoder;

impl Http1Encoder {
    /// Serialize an HTTP request into a byte buffer.
    pub fn encode_request(req: &Request, dst: &mut Vec<u8>) {
        // Request line: METHOD URI HTTP/1.1\r\n
        dst.extend_from_slice(req.method.as_str().as_bytes());
        dst.push(b' ');
        dst.extend_from_slice(req.uri.as_bytes());
        dst.push(b' ');
        dst.extend_from_slice(req.version.as_str().as_bytes());
        dst.extend_from_slice(b"\r\n");

        // Headers
        let mut has_content_length = false;
        for (name, value) in req.headers.iter() {
            if name.eq_ignore_ascii_case("content-length") {
                has_content_length = true;
            }
            dst.extend_from_slice(name.as_bytes());
            dst.extend_from_slice(b": ");
            dst.extend_from_slice(value.as_bytes());
            dst.extend_from_slice(b"\r\n");
        }

        // Auto-inject content-length if not present and body is not empty
        if !has_content_length && !req.body.is_empty() {
            let cl = format!("content-length: {}\r\n", req.body.len());
            dst.extend_from_slice(cl.as_bytes());
        }

        dst.extend_from_slice(b"\r\n");

        // Body
        if !req.body.is_empty() {
            dst.extend_from_slice(&req.body);
        }
    }

    /// Serialize an HTTP response into a byte buffer.
    pub fn encode_response(resp: &Response, dst: &mut Vec<u8>) {
        // Status line: HTTP/1.1 STATUS REASON\r\n
        dst.extend_from_slice(resp.version.as_str().as_bytes());
        dst.push(b' ');
        dst.extend_from_slice(resp.status.to_string().as_bytes());
        dst.push(b' ');
        dst.extend_from_slice(resp.reason.as_bytes());
        dst.extend_from_slice(b"\r\n");

        // Headers
        let mut has_content_length = false;
        for (name, value) in resp.headers.iter() {
            if name.eq_ignore_ascii_case("content-length") {
                has_content_length = true;
            }
            dst.extend_from_slice(name.as_bytes());
            dst.extend_from_slice(b": ");
            dst.extend_from_slice(value.as_bytes());
            dst.extend_from_slice(b"\r\n");
        }

        if !has_content_length && !resp.body.is_empty() {
            let cl = format!("content-length: {}\r\n", resp.body.len());
            dst.extend_from_slice(cl.as_bytes());
        }

        dst.extend_from_slice(b"\r\n");

        // Body
        if !resp.body.is_empty() {
            dst.extend_from_slice(&resp.body);
        }
    }

    /// Format a single HTTP chunk: `<hex_len>\r\n<data>\r\n`.
    pub fn encode_chunk(chunk: &[u8], dst: &mut Vec<u8>) {
        let size_hex = format!("{:x}\r\n", chunk.len());
        dst.extend_from_slice(size_hex.as_bytes());
        dst.extend_from_slice(chunk);
        dst.extend_from_slice(b"\r\n");
    }

    /// Format terminating chunk: `0\r\n\r\n`.
    pub fn encode_final_chunk(dst: &mut Vec<u8>) {
        dst.extend_from_slice(b"0\r\n\r\n");
    }
}

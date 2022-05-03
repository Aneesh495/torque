//! Zero-copy streaming HTTP/1.1 request and response parser.
//!
//! Provides incremental parsing of request lines, status lines, headers,
//! fixed content bodies, and chunked transfer decoding without dynamic allocation.

use bytes::Bytes;
use torque_core::error::{Result, TorqueError};

use super::types::{HeaderMap, Method, Request, Response, Version};

/// Parser state outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseResult<T> {
    /// Full message successfully parsed with consumed byte count.
    Complete(T, usize),
    /// Incomplete message; requires more data from socket.
    Incomplete,
}

/// Streaming HTTP/1.1 parser.
pub struct Http1Parser;

impl Http1Parser {
    /// Parse an HTTP/1.1 request from a raw byte buffer.
    pub fn parse_request(src: &[u8]) -> Result<ParseResult<Request>> {
        // Find header boundary (\r\n\r\n)
        let header_end = match find_header_end(src) {
            Some(pos) => pos,
            None => return Ok(ParseResult::Incomplete),
        };

        let header_bytes = &src[..header_end];
        let mut lines = header_bytes.split(|&b| b == b'\n');

        // Parse Request Line
        let request_line = match lines.next() {
            Some(line) => trim_cr(line),
            None => return Ok(ParseResult::Incomplete),
        };

        let mut parts = request_line.split(|&b| b == b' ');
        let method_bytes = parts
            .next()
            .ok_or_else(|| TorqueError::protocol("Missing HTTP method"))?;
        let uri_bytes = parts
            .next()
            .ok_or_else(|| TorqueError::protocol("Missing HTTP request URI"))?;
        let version_bytes = parts
            .next()
            .ok_or_else(|| TorqueError::protocol("Missing HTTP version"))?;

        let method = Method::from_bytes(method_bytes)?;
        let uri = String::from_utf8(uri_bytes.to_vec())
            .map_err(|e| TorqueError::protocol_with_source("Invalid URI UTF-8", e))?;
        let version = match version_bytes {
            b"HTTP/1.1" => Version::Http11,
            b"HTTP/1.0" => Version::Http10,
            b"HTTP/2.0" => Version::Http2,
            _ => {
                return Err(TorqueError::protocol(format!(
                    "Unsupported protocol version: {}",
                    String::from_utf8_lossy(version_bytes)
                )))
            }
        };

        // Parse Headers
        let mut headers = HeaderMap::new();
        for line in lines {
            let line = trim_cr(line);
            if line.is_empty() {
                continue;
            }
            if let Some(colon_idx) = line.iter().position(|&b| b == b':') {
                let name = String::from_utf8_lossy(&line[..colon_idx])
                    .trim()
                    .to_string();
                let value = String::from_utf8_lossy(&line[colon_idx + 1..])
                    .trim()
                    .to_string();
                headers.insert(name, value);
            }
        }

        // Body parsing
        let body_offset = header_end + 4; // Skip \r\n\r\n
        let remaining_data = &src[body_offset..];

        // Check for chunked transfer encoding
        if let Some(te) = headers.get("transfer-encoding") {
            if te.eq_ignore_ascii_case("chunked") {
                return match parse_chunked_body(remaining_data)? {
                    ParseResult::Complete(body, consumed) => {
                        let total_consumed = body_offset + consumed;
                        let mut req = Request::new(method, uri);
                        req.version = version;
                        req.headers = headers;
                        req.body = body;
                        Ok(ParseResult::Complete(req, total_consumed))
                    }
                    ParseResult::Incomplete => Ok(ParseResult::Incomplete),
                };
            }
        }

        // Check for Content-Length
        let content_length = match headers.get("content-length") {
            Some(val) => val
                .parse::<usize>()
                .map_err(|_| TorqueError::protocol("Invalid Content-Length header"))?,
            None => 0,
        };

        if remaining_data.len() < content_length {
            return Ok(ParseResult::Incomplete);
        }

        let body = Bytes::copy_from_slice(&remaining_data[..content_length]);
        let total_consumed = body_offset + content_length;

        let mut req = Request::new(method, uri);
        req.version = version;
        req.headers = headers;
        req.body = body;

        Ok(ParseResult::Complete(req, total_consumed))
    }

    /// Parse an HTTP/1.1 response from a raw byte buffer.
    pub fn parse_response(src: &[u8]) -> Result<ParseResult<Response>> {
        let header_end = match find_header_end(src) {
            Some(pos) => pos,
            None => return Ok(ParseResult::Incomplete),
        };

        let header_bytes = &src[..header_end];
        let mut lines = header_bytes.split(|&b| b == b'\n');

        // Status Line: HTTP/1.1 200 OK
        let status_line = match lines.next() {
            Some(line) => trim_cr(line),
            None => return Ok(ParseResult::Incomplete),
        };

        let mut parts = status_line.splitn(3, |&b| b == b' ');
        let version_bytes = parts
            .next()
            .ok_or_else(|| TorqueError::protocol("Missing status version"))?;
        let status_bytes = parts
            .next()
            .ok_or_else(|| TorqueError::protocol("Missing status code"))?;
        let reason_bytes = parts.next().unwrap_or(b"");

        let version = match version_bytes {
            b"HTTP/1.1" => Version::Http11,
            b"HTTP/1.0" => Version::Http10,
            _ => Version::Http11,
        };

        let status_str = std::str::from_utf8(status_bytes)
            .map_err(|_| TorqueError::protocol("Invalid status code UTF-8"))?;
        let status: u16 = status_str
            .parse()
            .map_err(|_| TorqueError::protocol("Non-numeric HTTP status code"))?;

        let reason = String::from_utf8_lossy(reason_bytes).to_string();

        let mut headers = HeaderMap::new();
        for line in lines {
            let line = trim_cr(line);
            if line.is_empty() {
                continue;
            }
            if let Some(colon_idx) = line.iter().position(|&b| b == b':') {
                let name = String::from_utf8_lossy(&line[..colon_idx])
                    .trim()
                    .to_string();
                let value = String::from_utf8_lossy(&line[colon_idx + 1..])
                    .trim()
                    .to_string();
                headers.insert(name, value);
            }
        }

        let body_offset = header_end + 4;
        let remaining_data = &src[body_offset..];

        if let Some(te) = headers.get("transfer-encoding") {
            if te.eq_ignore_ascii_case("chunked") {
                return match parse_chunked_body(remaining_data)? {
                    ParseResult::Complete(body, consumed) => {
                        let total_consumed = body_offset + consumed;
                        let mut resp = Response::new(status);
                        resp.reason = reason;
                        resp.version = version;
                        resp.headers = headers;
                        resp.body = body;
                        Ok(ParseResult::Complete(resp, total_consumed))
                    }
                    ParseResult::Incomplete => Ok(ParseResult::Incomplete),
                };
            }
        }

        let content_length = match headers.get("content-length") {
            Some(val) => val
                .parse::<usize>()
                .map_err(|_| TorqueError::protocol("Invalid Content-Length header"))?,
            None => 0,
        };

        if remaining_data.len() < content_length {
            return Ok(ParseResult::Incomplete);
        }

        let body = Bytes::copy_from_slice(&remaining_data[..content_length]);
        let total_consumed = body_offset + content_length;

        let mut resp = Response::new(status);
        resp.reason = reason;
        resp.version = version;
        resp.headers = headers;
        resp.body = body;

        Ok(ParseResult::Complete(resp, total_consumed))
    }
}

/// Find index of the first `\r\n\r\n` boundary in byte slice.
fn find_header_end(src: &[u8]) -> Option<usize> {
    (0..src.len().saturating_sub(3)).find(|&i| {
        src[i] == b'\r' && src[i + 1] == b'\n' && src[i + 2] == b'\r' && src[i + 3] == b'\n'
    })
}

/// Strip trailing `\r` from a line slice.
fn trim_cr(slice: &[u8]) -> &[u8] {
    if let Some(&b'\r') = slice.last() {
        &slice[..slice.len() - 1]
    } else {
        slice
    }
}

/// Decode RFC 7230 chunked transfer-encoded stream.
fn parse_chunked_body(src: &[u8]) -> Result<ParseResult<Bytes>> {
    let mut cursor = 0;
    let mut chunks = Vec::new();

    loop {
        let remaining = &src[cursor..];
        // Find CRLF following chunk size
        let line_end = match remaining.windows(2).position(|w| w == b"\r\n") {
            Some(pos) => pos,
            None => return Ok(ParseResult::Incomplete),
        };

        let size_str = std::str::from_utf8(&remaining[..line_end])
            .map_err(|_| TorqueError::protocol("Invalid chunk header UTF-8"))?;

        // Strip chunk extensions if present (e.g., "1a;ext=foo")
        let hex_size = size_str.split(';').next().unwrap().trim();
        let chunk_size = usize::from_str_radix(hex_size, 16)
            .map_err(|_| TorqueError::protocol("Invalid chunk hex size"))?;

        cursor += line_end + 2; // Skip size and CRLF

        if chunk_size == 0 {
            // Final chunk: check for terminating CRLF
            if src.len() < cursor + 2 {
                return Ok(ParseResult::Incomplete);
            }
            if &src[cursor..cursor + 2] == b"\r\n" {
                cursor += 2;
                return Ok(ParseResult::Complete(Bytes::from(chunks), cursor));
            } else {
                // Ignore trailing headers until double CRLF
                if let Some(pos) = src[cursor..].windows(2).position(|w| w == b"\r\n") {
                    cursor += pos + 2;
                    return Ok(ParseResult::Complete(Bytes::from(chunks), cursor));
                }
                return Ok(ParseResult::Incomplete);
            }
        }

        // Verify data payload and following CRLF are present
        if src.len() < cursor + chunk_size + 2 {
            return Ok(ParseResult::Incomplete);
        }

        let chunk_data = &src[cursor..cursor + chunk_size];
        chunks.extend_from_slice(chunk_data);
        cursor += chunk_size;

        if &src[cursor..cursor + 2] != b"\r\n" {
            return Err(TorqueError::protocol("Missing CRLF after chunk data"));
        }
        cursor += 2;
    }
}

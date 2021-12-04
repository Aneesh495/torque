//! HTTP/1.1 foundational types, methods, status codes, and header collections.

use bytes::Bytes;
use std::collections::HashMap;
use std::fmt;
use torque_core::error::{Result, TorqueError};

/// Standard HTTP request methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Connect,
    Trace,
}

impl Method {
    /// Parse method from byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match bytes {
            b"GET" => Ok(Method::Get),
            b"POST" => Ok(Method::Post),
            b"PUT" => Ok(Method::Put),
            b"DELETE" => Ok(Method::Delete),
            b"PATCH" => Ok(Method::Patch),
            b"HEAD" => Ok(Method::Head),
            b"OPTIONS" => Ok(Method::Options),
            b"CONNECT" => Ok(Method::Connect),
            b"TRACE" => Ok(Method::Trace),
            other => Err(TorqueError::protocol(format!(
                "Unsupported HTTP method: {}",
                String::from_utf8_lossy(other)
            ))),
        }
    }

    /// Return uppercase static string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Connect => "CONNECT",
            Method::Trace => "TRACE",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// HTTP protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Version {
    Http10,
    Http11,
    Http2,
}

impl Version {
    pub fn as_str(&self) -> &'static str {
        match self {
            Version::Http10 => "HTTP/1.0",
            Version::Http11 => "HTTP/1.1",
            Version::Http2 => "HTTP/2.0",
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Case-insensitive HTTP header map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderMap {
    headers: HashMap<String, String>,
}

impl HeaderMap {
    /// Create a new empty header map.
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Insert a header name and value. Names are normalized to lowercase.
    pub fn insert(&mut self, key: impl AsRef<str>, value: impl Into<String>) {
        self.headers.insert(key.as_ref().to_ascii_lowercase(), value.into());
    }

    /// Retrieve a header value by case-insensitive key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }

    /// Check if a header exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.headers.contains_key(&key.to_ascii_lowercase())
    }

    /// Remove a header by name.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.headers.remove(&key.to_ascii_lowercase())
    }

    /// Number of headers stored.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Whether header map is empty.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Iterate over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Parsed HTTP request representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub uri: String,
    pub version: Version,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Request {
    /// Construct a new HTTP request.
    pub fn new(method: Method, uri: impl Into<String>) -> Self {
        Self {
            method,
            uri: uri.into(),
            version: Version::Http11,
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }
}

/// Parsed HTTP response representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub reason: String,
    pub version: Version,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Response {
    /// Construct a new response with status code and default reason phrase.
    pub fn new(status: u16) -> Self {
        let reason = match status {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Status",
        };
        Self {
            status,
            reason: reason.to_string(),
            version: Version::Http11,
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }

    /// Create a 200 OK response with body.
    pub fn ok(body: impl Into<Bytes>) -> Self {
        let mut resp = Self::new(200);
        resp.body = body.into();
        resp
    }

    /// Create a 502 Bad Gateway response.
    pub fn bad_gateway(detail: impl Into<String>) -> Self {
        let mut resp = Self::new(502);
        resp.body = Bytes::from(detail.into());
        resp
    }
}

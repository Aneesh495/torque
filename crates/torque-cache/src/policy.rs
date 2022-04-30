//! RFC 7234 HTTP caching policy rules and conditional validation.

use torque_core::time::fast_now_millis;
use torque_proto::http1::{Method, Request, Response};

/// Cacheability directive flags parsed from Cache-Control headers.
#[derive(Debug, Clone, Default)]
pub struct CacheControlDirectives {
    pub no_cache: bool,
    pub no_store: bool,
    pub max_age_secs: Option<u64>,
    pub s_maxage_secs: Option<u64>,
    pub must_revalidate: bool,
    pub public: bool,
    pub private: bool,
}

impl CacheControlDirectives {
    /// Parse a Cache-Control header string value.
    pub fn parse(header_val: &str) -> Self {
        let mut directives = Self::default();
        for part in header_val.split(',') {
            let part = part.trim().to_ascii_lowercase();
            if part == "no-cache" {
                directives.no_cache = true;
            } else if part == "no-store" {
                directives.no_store = true;
            } else if part == "must-revalidate" {
                directives.must_revalidate = true;
            } else if part == "public" {
                directives.public = true;
            } else if part == "private" {
                directives.private = true;
            } else if part.starts_with("max-age=") {
                if let Ok(secs) = part[8..].parse::<u64>() {
                    directives.max_age_secs = Some(secs);
                }
            } else if part.starts_with("s-maxage=") {
                if let Ok(secs) = part[9..].parse::<u64>() {
                    directives.s_maxage_secs = Some(secs);
                }
            }
        }
        directives
    }
}

/// Check if an incoming request is eligible for caching lookup.
pub fn is_request_cacheable(req: &Request) -> bool {
    // Only GET and HEAD methods are cacheable (RFC 7234 Section 2)
    if req.method != Method::Get && req.method != Method::Head {
        return false;
    }

    if let Some(cc) = req.headers.get("cache-control") {
        let d = CacheControlDirectives::parse(cc);
        if d.no_store {
            return false;
        }
    }

    true
}

/// Check if an upstream response is eligible for cache storage.
pub fn is_response_cacheable(resp: &Response) -> bool {
    // Status codes cacheable by default
    let status_cacheable = matches!(
        resp.status,
        200 | 203 | 204 | 206 | 300 | 301 | 308 | 404 | 410
    );
    if !status_cacheable {
        return false;
    }

    if let Some(cc) = resp.headers.get("cache-control") {
        let d = CacheControlDirectives::parse(cc);
        if d.no_store || d.private {
            return false;
        }
    }

    true
}

/// Calculate expiration timestamp in milliseconds for a response.
pub fn calculate_expiration(resp: &Response, default_ttl_secs: u64) -> u64 {
    let now = fast_now_millis();
    if let Some(cc) = resp.headers.get("cache-control") {
        let d = CacheControlDirectives::parse(cc);
        let max_age = d.s_maxage_secs.or(d.max_age_secs);
        if let Some(secs) = max_age {
            return now + (secs * 1000);
        }
    }
    now + (default_ttl_secs * 1000)
}

/// Validate conditional request against cached response (`ETag` / `If-None-Match`).
/// Returns true if client conditional matches (synthesize 304).
pub fn is_conditional_match(req: &Request, cached_resp: &Response) -> bool {
    if let Some(client_etag) = req.headers.get("if-none-match") {
        if let Some(cached_etag) = cached_resp.headers.get("etag") {
            if client_etag.trim() == cached_etag.trim() || client_etag.trim() == "*" {
                return true;
            }
        }
    }
    false
}

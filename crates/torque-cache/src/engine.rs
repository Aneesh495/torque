//! High-level HTTP cache engine managing ARC storage, expiration, and conditional responses.

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use torque_core::time::fast_now_millis;
use torque_proto::http1::{Request, Response};

use super::arc::AdaptiveReplacementCache;
use super::key::generate_cache_key;
use super::policy::{calculate_expiration, is_conditional_match, is_request_cacheable, is_response_cacheable};

/// Result of a cache lookup attempt.
#[derive(Debug)]
pub enum CacheLookupResult {
    /// Fresh cache hit ready for client response.
    Hit(Response),
    /// Conditional request matched ETag/Last-Modified; 304 synthesized.
    ConditionalHit(Response),
    /// Cached entry found but expired.
    Stale(Response),
    /// Not present in cache.
    Miss,
}

/// High-level HTTP caching engine.
pub struct HttpCacheEngine {
    arc: RwLock<AdaptiveReplacementCache<Response>>,
    hits: AtomicU64,
    misses: AtomicU64,
    stores: AtomicU64,
}

impl HttpCacheEngine {
    /// Construct a cache engine with maximum entry capacity.
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            arc: RwLock::new(AdaptiveReplacementCache::new(capacity)),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            stores: AtomicU64::new(0),
        })
    }

    /// Perform a cache lookup for an incoming request.
    pub fn lookup(&self, req: &Request, vary_headers: &[String]) -> CacheLookupResult {
        if !is_request_cacheable(req) {
            return CacheLookupResult::Miss;
        }

        let key = generate_cache_key(req, vary_headers);
        let entry = {
            let mut guard = self.arc.write();
            guard.get(&key)
        };

        match entry {
            Some(e) => {
                let now = fast_now_millis();
                if is_conditional_match(req, &e.value) {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    let mut resp304 = Response::new(304);
                    resp304.headers.insert("etag", e.value.headers.get("etag").unwrap_or(""));
                    return CacheLookupResult::ConditionalHit(resp304);
                }

                if now < e.expires_at_millis {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    let mut resp = e.value.clone();
                    resp.headers.insert("x-cache", "HIT");
                    CacheLookupResult::Hit(resp)
                } else {
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    CacheLookupResult::Stale(e.value)
                }
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                CacheLookupResult::Miss
            }
        }
    }

    /// Store an upstream response into the cache.
    pub fn store(&self, req: &Request, resp: &Response, default_ttl_secs: u64, tags: Vec<String>) -> bool {
        if !is_response_cacheable(resp) {
            return false;
        }

        let key = generate_cache_key(req, &[]);
        let expires_at = calculate_expiration(resp, default_ttl_secs);

        let mut guard = self.arc.write();
        guard.put(key, resp.clone(), expires_at, tags);
        self.stores.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Invalidate all cache entries matching a surrogate tag.
    pub fn invalidate_tag(&self, tag: &str) -> usize {
        let mut guard = self.arc.write();
        guard.invalidate_tag(tag)
    }

    /// Number of cache hits recorded.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Number of cache misses recorded.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Total entries stored.
    pub fn stores(&self) -> u64 {
        self.stores.load(Ordering::Relaxed)
    }
}

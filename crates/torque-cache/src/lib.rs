//! # Torque Cache
//!
//! RFC 7234 compliant HTTP caching and Adaptive Replacement Cache (ARC) engine.

pub mod arc;
pub mod engine;
pub mod key;
pub mod policy;

pub use arc::{AdaptiveReplacementCache, CacheEntry};
pub use engine::{CacheLookupResult, HttpCacheEngine};
pub use key::generate_cache_key;
pub use policy::{
    calculate_expiration, is_conditional_match, is_request_cacheable, is_response_cacheable,
    CacheControlDirectives,
};

/// Cache prelude.
pub mod prelude {
    pub use crate::engine::{CacheLookupResult, HttpCacheEngine};
    pub use crate::key::generate_cache_key;
}

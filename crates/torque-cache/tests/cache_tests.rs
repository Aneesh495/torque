use bytes::Bytes;
use torque_cache::arc::AdaptiveReplacementCache;
use torque_cache::engine::{CacheLookupResult, HttpCacheEngine};
use torque_cache::policy::{is_request_cacheable, is_response_cacheable, CacheControlDirectives};
use torque_proto::http1::{Method, Request, Response};

#[test]
fn test_cache_control_directives_parsing() {
    let header = "public, max-age=3600, s-maxage=7200, must-revalidate";
    let d = CacheControlDirectives::parse(header);
    assert!(d.public);
    assert!(!d.private);
    assert!(!d.no_store);
    assert_eq!(d.max_age_secs, Some(3600));
    assert_eq!(d.s_maxage_secs, Some(7200));
    assert!(d.must_revalidate);
}

#[test]
fn test_cacheability_checks() {
    let req_get = Request::new(Method::Get, "/api/data");
    assert!(is_request_cacheable(&req_get));

    let req_post = Request::new(Method::Post, "/api/data");
    assert!(!is_request_cacheable(&req_post));

    let mut resp_ok = Response::new(200);
    assert!(is_response_cacheable(&resp_ok));

    resp_ok.headers.insert("cache-control", "no-store");
    assert!(!is_response_cacheable(&resp_ok));

    let resp_500 = Response::new(500);
    assert!(!is_response_cacheable(&resp_500));
}

#[test]
fn test_arc_cache_self_tuning_adaptation() {
    let mut arc = AdaptiveReplacementCache::<String>::new(4);
    assert_eq!(arc.capacity(), 4);
    assert_eq!(arc.target_p(), 0);

    // Insert 4 items into T1
    arc.put("k1".to_string(), "v1".to_string(), 999999999999, vec![]);
    arc.put("k2".to_string(), "v2".to_string(), 999999999999, vec![]);
    arc.put("k3".to_string(), "v3".to_string(), 999999999999, vec![]);
    arc.put("k4".to_string(), "v4".to_string(), 999999999999, vec![]);
    assert_eq!(arc.len(), 4);

    // Access k1 -> moves to T2
    assert!(arc.get("k1").is_some());

    // Insert k5 -> evicts LRU from T1 into B1 ghost list
    arc.put("k5".to_string(), "v5".to_string(), 999999999999, vec![]);
    assert_eq!(arc.len(), 4);

    // Tag invalidation
    arc.put("tagged_key".to_string(), "val".to_string(), 999999999999, vec!["user_42".to_string()]);
    assert_eq!(arc.invalidate_tag("user_42"), 1);
    assert!(arc.get("tagged_key").is_none());
}

#[test]
fn test_http_cache_engine_full_lifecycle() {
    let engine = HttpCacheEngine::new(100);

    let req = Request::new(Method::Get, "/catalog/products");
    let mut resp = Response::ok(Bytes::from_static(b"{\"products\": [1, 2, 3]}"));
    resp.headers.insert("etag", "\"v1.2.3\"");
    resp.headers.insert("cache-control", "public, max-age=60");

    // Miss on first lookup
    match engine.lookup(&req, &[]) {
        CacheLookupResult::Miss => {}
        _ => panic!("Expected cache miss"),
    }

    // Store in cache
    assert!(engine.store(&req, &resp, 60, vec!["catalog".to_string()]));
    assert_eq!(engine.stores(), 1);

    // Fresh hit on second lookup
    match engine.lookup(&req, &[]) {
        CacheLookupResult::Hit(cached) => {
            assert_eq!(cached.status, 200);
            assert_eq!(cached.body, resp.body);
            assert_eq!(cached.headers.get("x-cache"), Some("HIT"));
        }
        _ => panic!("Expected cache hit"),
    }

    // Conditional request with matching If-None-Match -> 304 Not Modified
    let mut cond_req = Request::new(Method::Get, "/catalog/products");
    cond_req.headers.insert("if-none-match", "\"v1.2.3\"");

    match engine.lookup(&cond_req, &[]) {
        CacheLookupResult::ConditionalHit(resp304) => {
            assert_eq!(resp304.status, 304);
            assert_eq!(resp304.headers.get("etag"), Some("\"v1.2.3\""));
        }
        _ => panic!("Expected conditional 304 hit"),
    }

    // Tag invalidation
    assert_eq!(engine.invalidate_tag("catalog"), 1);
    match engine.lookup(&req, &[]) {
        CacheLookupResult::Miss => {}
        _ => panic!("Expected cache miss after tag invalidation"),
    }
}

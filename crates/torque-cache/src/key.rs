//! HTTP cache key generation with Vary header normalization.

use torque_proto::http1::Request;

/// Generate a unique normalized cache key string for an HTTP request.
pub fn generate_cache_key(req: &Request, vary_headers: &[String]) -> String {
    let method = req.method.as_str();
    let host = req.headers.get("host").unwrap_or("localhost");
    let uri = &req.uri;

    let mut key = format!("{}|{}|{}", method, host, uri);

    if !vary_headers.is_empty() {
        key.push('|');
        for vh in vary_headers {
            let val = req.headers.get(vh).unwrap_or("");
            key.push_str(vh);
            key.push('=');
            key.push_str(val);
            key.push(';');
        }
    }

    key
}

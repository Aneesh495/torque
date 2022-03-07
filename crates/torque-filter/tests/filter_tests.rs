use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use torque_filter::auth::{ApiKeyFilter, JwtAuthFilter};
use torque_filter::pipeline::{Filter, FilterAction, FilterChain, FilterContext, FilterStage};
use torque_filter::security::{CorsConfig, CorsFilter, SecurityHeadersFilter};
use torque_filter::tracing::TraceparentFilter;
use torque_proto::http1::{Method, Request, Response};

type HmacSha256 = Hmac<Sha256>;

fn create_test_jwt(secret: &[u8], sub: &str, exp_offset_secs: i64) -> String {
    let header = "{\"alg\":\"HS256\",\"typ\":\"JWT\"}";
    let now = torque_core::time::unix_timestamp_secs() as i64;
    let exp = now + exp_offset_secs;
    let payload = format!("{{\"sub\":\"{}\",\"exp\":{}}}", sub, exp);

    let h_b64 = URL_SAFE_NO_PAD.encode(header.as_bytes());
    let p_b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let signing_input = format!("{}.{}", h_b64, p_b64);

    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(signing_input.as_bytes());
    let sig = mac.finalize().into_bytes();
    let s_b64 = URL_SAFE_NO_PAD.encode(&sig);

    format!("{}.{}", signing_input, s_b64)
}

#[tokio::test]
async fn test_jwt_auth_filter_valid_token() {
    let secret = b"super-secure-torque-jwt-secret-key-256";
    let token = create_test_jwt(secret, "quant_user_101", 3600);

    let filter = JwtAuthFilter::new(secret);
    let mut req = Request::new(Method::Get, "/api/v1/trades");
    req.headers.insert("authorization", format!("Bearer {}", token));

    let mut ctx = FilterContext::new();
    let action = filter.on_request(&mut req, &mut ctx).await.unwrap();

    assert_eq!(action, FilterAction::Continue);
    assert_eq!(req.headers.get("x-user-id"), Some("quant_user_101"));
    assert_eq!(ctx.get("jwt_sub"), Some("quant_user_101"));
}

#[tokio::test]
async fn test_jwt_auth_filter_expired_token() {
    let secret = b"super-secure-torque-jwt-secret-key-256";
    // Expired 60 seconds ago
    let token = create_test_jwt(secret, "expired_user", -60);

    let filter = JwtAuthFilter::new(secret);
    let mut req = Request::new(Method::Get, "/api/v1/trades");
    req.headers.insert("authorization", format!("Bearer {}", token));

    let mut ctx = FilterContext::new();
    let action = filter.on_request(&mut req, &mut ctx).await.unwrap();

    match action {
        FilterAction::Respond(resp) => {
            assert_eq!(resp.status, 401);
        }
        _ => panic!("Expected 401 response for expired token"),
    }
}

#[tokio::test]
async fn test_api_key_filter() {
    let filter = ApiKeyFilter::new(vec!["valid-secret-key-9988"]);

    let mut req_valid = Request::new(Method::Get, "/api/v1/status");
    req_valid.headers.insert("x-api-key", "valid-secret-key-9988");
    let mut ctx = FilterContext::new();
    assert_eq!(
        filter.on_request(&mut req_valid, &mut ctx).await.unwrap(),
        FilterAction::Continue
    );

    let mut req_invalid = Request::new(Method::Get, "/api/v1/status");
    req_invalid.headers.insert("x-api-key", "wrong-key");
    match filter.on_request(&mut req_invalid, &mut ctx).await.unwrap() {
        FilterAction::Respond(resp) => assert_eq!(resp.status, 403),
        _ => panic!("Expected 403 for wrong api key"),
    }
}

#[tokio::test]
async fn test_cors_preflight_interception() {
    let filter = CorsFilter::new(CorsConfig::default());
    let mut req = Request::new(Method::Options, "/api/v1/data");
    req.headers.insert("origin", "https://app.example.com");

    let mut ctx = FilterContext::new();
    let action = filter.on_request(&mut req, &mut ctx).await.unwrap();

    match action {
        FilterAction::Respond(resp) => {
            assert_eq!(resp.status, 204);
            assert_eq!(resp.headers.get("access-control-allow-origin"), Some("*"));
            assert!(resp.headers.contains_key("access-control-allow-methods"));
        }
        _ => panic!("Expected preflight interception"),
    }
}

#[tokio::test]
async fn test_security_headers_filter() {
    let filter = SecurityHeadersFilter;
    let mut resp = Response::new(200);
    let mut ctx = FilterContext::new();

    assert_eq!(
        filter.on_response(&mut resp, &mut ctx).await.unwrap(),
        FilterAction::Continue
    );
    assert!(resp.headers.contains_key("strict-transport-security"));
    assert_eq!(resp.headers.get("x-frame-options"), Some("DENY"));
    assert_eq!(resp.headers.get("x-content-type-options"), Some("nosniff"));
}

#[tokio::test]
async fn test_traceparent_propagation() {
    let filter = TraceparentFilter;
    let mut req = Request::new(Method::Get, "/api/v1/order");
    let mut ctx = FilterContext::new();

    assert_eq!(
        filter.on_request(&mut req, &mut ctx).await.unwrap(),
        FilterAction::Continue
    );
    let generated_tp = req.headers.get("traceparent").expect("traceparent should be set");
    assert!(generated_tp.starts_with("00-"));

    let mut resp = Response::new(200);
    assert_eq!(
        filter.on_response(&mut resp, &mut ctx).await.unwrap(),
        FilterAction::Continue
    );
    assert_eq!(resp.headers.get("traceparent"), Some(generated_tp));
}

#[tokio::test]
async fn test_filter_chain_execution() {
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(TraceparentFilter));
    chain.add_filter(Arc::new(SecurityHeadersFilter));

    let mut req = Request::new(Method::Get, "/healthz");
    let mut ctx = FilterContext::new();

    let action = chain.execute_request(FilterStage::PreRouting, &mut req, &mut ctx).await.unwrap();
    assert_eq!(action, FilterAction::Continue);
    assert!(req.headers.contains_key("traceparent"));

    let mut resp = Response::new(200);
    let action_resp = chain.execute_response(FilterStage::PostUpstream, &mut resp, &mut ctx).await.unwrap();
    assert_eq!(action_resp, FilterAction::Continue);
    assert!(resp.headers.contains_key("strict-transport-security"));
}

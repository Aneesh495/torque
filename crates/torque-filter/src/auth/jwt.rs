//! Zero-copy JWT validation and claims extraction filter (HMAC-SHA256).

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use torque_core::error::Result;
use torque_core::time::unix_timestamp_secs;
use torque_proto::http1::{Request, Response};

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

type HmacSha256 = Hmac<Sha256>;

/// JWT authentication and validation filter.
pub struct JwtAuthFilter {
    secret: Vec<u8>,
    header_name: String,
}

impl JwtAuthFilter {
    /// Construct a JWT filter with HMAC-SHA256 shared secret.
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret: secret.as_ref().to_vec(),
            header_name: "authorization".to_string(),
        }
    }

    /// Set custom header name (default: authorization).
    pub fn with_header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = name.into();
        self
    }

    /// Verify token signature and expiration.
    fn verify_token(&self, token: &str) -> Option<String> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2]).ok()?;

        // Verify HMAC-SHA256
        let mut mac = HmacSha256::new_from_slice(&self.secret).ok()?;
        mac.update(signing_input.as_bytes());
        if mac.verify_slice(&signature_bytes).is_err() {
            return None;
        }

        // Decode Payload
        let payload_bytes = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
        let payload_str = String::from_utf8(payload_bytes).ok()?;

        // Parse claims: find "sub" and "exp"
        // Simple fast string search avoiding heavy serde overhead
        let now = unix_timestamp_secs();
        if let Some(exp_idx) = payload_str.find("\"exp\":") {
            let rest = &payload_str[exp_idx + 6..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(exp) = num_str.parse::<u64>() {
                if exp <= now {
                    return None; // Expired
                }
            }
        }

        let sub = if let Some(sub_idx) = payload_str.find("\"sub\":\"") {
            let rest = &payload_str[sub_idx + 7..];
            let s: String = rest.chars().take_while(|&c| c != '"').collect();
            s
        } else {
            "anonymous".to_string()
        };

        Some(sub)
    }
}

#[async_trait]
impl Filter for JwtAuthFilter {
    fn name(&self) -> &'static str {
        "jwt_auth"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PostRouting
    }

    async fn on_request(&self, req: &mut Request, ctx: &mut FilterContext) -> Result<FilterAction> {
        let auth_hdr = match req.headers.get(&self.header_name) {
            Some(h) => h,
            None => {
                let mut resp = Response::new(401);
                resp.body =
                    bytes::Bytes::from_static(b"{\"error\":\"Missing authorization header\"}");
                resp.headers.insert("content-type", "application/json");
                return Ok(FilterAction::Respond(resp));
            }
        };

        let token = if auth_hdr.to_ascii_lowercase().starts_with("bearer ") {
            &auth_hdr[7..].trim()
        } else {
            auth_hdr.trim()
        };

        match self.verify_token(token) {
            Some(sub) => {
                req.headers.insert("x-user-id", sub.clone());
                ctx.set("jwt_sub", sub);
                Ok(FilterAction::Continue)
            }
            None => {
                let mut resp = Response::new(401);
                resp.body =
                    bytes::Bytes::from_static(b"{\"error\":\"Invalid or expired JWT token\"}");
                resp.headers.insert("content-type", "application/json");
                Ok(FilterAction::Respond(resp))
            }
        }
    }
}

//! Security headers response filter.

use async_trait::async_trait;
use torque_core::error::Result;
use torque_proto::http1::Response;

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

/// Injects standard security headers into downstream responses.
#[derive(Default)]
pub struct SecurityHeadersFilter;

#[async_trait]
impl Filter for SecurityHeadersFilter {
    fn name(&self) -> &'static str {
        "security_headers"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PostUpstream
    }

    async fn on_response(&self, resp: &mut Response, _ctx: &mut FilterContext) -> Result<FilterAction> {
        resp.headers.insert(
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        );
        resp.headers.insert("x-content-type-options", "nosniff");
        resp.headers.insert("x-frame-options", "DENY");
        resp.headers.insert(
            "referrer-policy",
            "strict-origin-when-cross-origin",
        );
        resp.headers.insert("content-security-policy", "default-src 'self'");
        Ok(FilterAction::Continue)
    }
}

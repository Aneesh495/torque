//! API Key authentication filter.

use async_trait::async_trait;
use std::collections::HashSet;
use torque_core::error::Result;
use torque_proto::http1::{Request, Response};

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

/// API key authentication filter.
pub struct ApiKeyFilter {
    header_name: String,
    valid_keys: HashSet<String>,
}

impl ApiKeyFilter {
    /// Construct with a list of valid API keys.
    pub fn new<I, S>(keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            header_name: "x-api-key".to_string(),
            valid_keys: keys.into_iter().map(Into::into).collect(),
        }
    }

    /// Set custom header name.
    pub fn with_header_name(mut self, name: impl Into<String>) -> Self {
        self.header_name = name.into();
        self
    }
}

#[async_trait]
impl Filter for ApiKeyFilter {
    fn name(&self) -> &'static str {
        "api_key_auth"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PreRouting
    }

    async fn on_request(&self, req: &mut Request, ctx: &mut FilterContext) -> Result<FilterAction> {
        let key = match req.headers.get(&self.header_name) {
            Some(k) => k,
            None => {
                let mut resp = Response::new(401);
                resp.body = bytes::Bytes::from_static(b"{\"error\":\"Missing API key\"}");
                resp.headers.insert("content-type", "application/json");
                return Ok(FilterAction::Respond(resp));
            }
        };

        if self.valid_keys.contains(key) {
            ctx.set("api_key", key);
            Ok(FilterAction::Continue)
        } else {
            let mut resp = Response::new(403);
            resp.body = bytes::Bytes::from_static(b"{\"error\":\"Invalid API key\"}");
            resp.headers.insert("content-type", "application/json");
            Ok(FilterAction::Respond(resp))
        }
    }
}

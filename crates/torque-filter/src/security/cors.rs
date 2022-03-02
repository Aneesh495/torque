//! Cross-Origin Resource Sharing (CORS) filter and preflight OPTIONS handler.

use async_trait::async_trait;
use torque_core::error::Result;
use torque_proto::http1::{Method, Request, Response};

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

/// CORS configuration parameters.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub allow_origin: String,
    pub allow_methods: String,
    pub allow_headers: String,
    pub expose_headers: String,
    pub max_age_secs: u32,
    pub allow_credentials: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origin: "*".to_string(),
            allow_methods: "GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD".to_string(),
            allow_headers: "Authorization, Content-Type, Accept, Origin, User-Agent, X-Requested-With, traceparent".to_string(),
            expose_headers: "Content-Length, Content-Type, Date, ETag, traceparent".to_string(),
            max_age_secs: 86400,
            allow_credentials: true,
        }
    }
}

/// CORS filter handling preflight requests and response header injection.
pub struct CorsFilter {
    config: CorsConfig,
}

impl CorsFilter {
    pub fn new(config: CorsConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Filter for CorsFilter {
    fn name(&self) -> &'static str {
        "cors"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PreRouting
    }

    async fn on_request(&self, req: &mut Request, _ctx: &mut FilterContext) -> Result<FilterAction> {
        // Intercept preflight OPTIONS request
        if req.method == Method::Options && req.headers.contains_key("origin") {
            let mut resp = Response::new(204);
            resp.reason = "No Content".to_string();
            resp.headers.insert("access-control-allow-origin", &self.config.allow_origin);
            resp.headers.insert("access-control-allow-methods", &self.config.allow_methods);
            resp.headers.insert("access-control-allow-headers", &self.config.allow_headers);
            resp.headers.insert("access-control-max-age", self.config.max_age_secs.to_string());
            if self.config.allow_credentials {
                resp.headers.insert("access-control-allow-credentials", "true");
            }
            return Ok(FilterAction::Respond(resp));
        }

        Ok(FilterAction::Continue)
    }

    async fn on_response(&self, resp: &mut Response, _ctx: &mut FilterContext) -> Result<FilterAction> {
        resp.headers.insert("access-control-allow-origin", &self.config.allow_origin);
        resp.headers.insert("access-control-expose-headers", &self.config.expose_headers);
        if self.config.allow_credentials {
            resp.headers.insert("access-control-allow-credentials", "true");
        }
        Ok(FilterAction::Continue)
    }
}

//! Asynchronous filter pipeline and lifecycle stages.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use torque_core::error::Result;
use torque_proto::http1::{Request, Response};

/// Lifecycle stage where a filter operates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterStage {
    PreRouting,
    PostRouting,
    PreUpstream,
    PostUpstream,
}

/// Action outcome returned by a filter.
#[derive(Debug, PartialEq, Eq)]
pub enum FilterAction {
    /// Proceed to the next filter in the pipeline.
    Continue,
    /// Terminate pipeline immediately and send this response to the client.
    Respond(Response),
    /// Abruptly drop the connection.
    Drop,
}

/// Shared request context passed down the filter chain.
#[derive(Debug, Default, Clone)]
pub struct FilterContext {
    pub properties: HashMap<String, String>,
}

impl FilterContext {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.properties.insert(key.into(), val.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
}

/// Asynchronous filter interface.
#[async_trait]
pub trait Filter: Send + Sync {
    /// Filter name for logging and metrics.
    fn name(&self) -> &'static str;

    /// Filter stage in the request/response lifecycle.
    fn stage(&self) -> FilterStage;

    /// Intercept and mutate an incoming request.
    async fn on_request(
        &self,
        _req: &mut Request,
        _ctx: &mut FilterContext,
    ) -> Result<FilterAction> {
        Ok(FilterAction::Continue)
    }

    /// Intercept and mutate an outgoing response.
    async fn on_response(
        &self,
        _resp: &mut Response,
        _ctx: &mut FilterContext,
    ) -> Result<FilterAction> {
        Ok(FilterAction::Continue)
    }
}

/// Filter chain executor managing sequential filter execution.
#[derive(Default)]
pub struct FilterChain {
    filters: Vec<Arc<dyn Filter>>,
}

impl FilterChain {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Register a filter into the chain.
    pub fn add_filter(&mut self, filter: Arc<dyn Filter>) {
        self.filters.push(filter);
    }

    /// Execute request filters matching the specified stage.
    pub async fn execute_request(
        &self,
        stage: FilterStage,
        req: &mut Request,
        ctx: &mut FilterContext,
    ) -> Result<FilterAction> {
        for f in &self.filters {
            if f.stage() == stage {
                match f.on_request(req, ctx).await? {
                    FilterAction::Continue => {}
                    other => return Ok(other),
                }
            }
        }
        Ok(FilterAction::Continue)
    }

    /// Execute response filters matching the specified stage.
    pub async fn execute_response(
        &self,
        stage: FilterStage,
        resp: &mut Response,
        ctx: &mut FilterContext,
    ) -> Result<FilterAction> {
        for f in &self.filters {
            if f.stage() == stage {
                match f.on_response(resp, ctx).await? {
                    FilterAction::Continue => {}
                    other => return Ok(other),
                }
            }
        }
        Ok(FilterAction::Continue)
    }
}

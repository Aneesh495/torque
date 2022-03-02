//! W3C Trace Context (traceparent) propagation and generation filter.

use async_trait::async_trait;
use rand::Rng;
use torque_core::error::Result;
use torque_proto::http1::{Request, Response};

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

/// W3C distributed traceparent propagator and generator.
#[derive(Default)]
pub struct TraceparentFilter;

#[async_trait]
impl Filter for TraceparentFilter {
    fn name(&self) -> &'static str {
        "traceparent"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PreRouting
    }

    async fn on_request(&self, req: &mut Request, ctx: &mut FilterContext) -> Result<FilterAction> {
        let traceparent = match req.headers.get("traceparent") {
            Some(tp) => tp.to_string(),
            None => {
                // Generate W3C traceparent: 00-{trace_id_32_hex}-{span_id_16_hex}-01
                let mut rng = rand::thread_rng();
                let trace_id: [u8; 16] = rng.gen();
                let span_id: [u8; 8] = rng.gen();
                let trace_hex = hex_encode(&trace_id);
                let span_hex = hex_encode(&span_id);
                format!("00-{}-{}-01", trace_hex, span_hex)
            }
        };

        req.headers.insert("traceparent", &traceparent);
        ctx.set("traceparent", &traceparent);
        Ok(FilterAction::Continue)
    }

    async fn on_response(&self, resp: &mut Response, ctx: &mut FilterContext) -> Result<FilterAction> {
        if let Some(tp) = ctx.get("traceparent") {
            resp.headers.insert("traceparent", tp);
        }
        Ok(FilterAction::Continue)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

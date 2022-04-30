//! Response body compression filter (Gzip and Deflate).

use async_trait::async_trait;
use bytes::Bytes;
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use std::io::Write;
use torque_core::error::Result;
use torque_proto::http1::Response;

use crate::pipeline::{Filter, FilterAction, FilterContext, FilterStage};

/// Response compression filter.
pub struct CompressionFilter {
    min_length: usize,
}

impl CompressionFilter {
    /// Construct with minimum payload length threshold in bytes (default: 256 bytes).
    pub fn new(min_length: usize) -> Self {
        Self { min_length }
    }
}

impl Default for CompressionFilter {
    fn default() -> Self {
        Self::new(256)
    }
}

#[async_trait]
impl Filter for CompressionFilter {
    fn name(&self) -> &'static str {
        "compression"
    }

    fn stage(&self) -> FilterStage {
        FilterStage::PostUpstream
    }

    async fn on_response(
        &self,
        resp: &mut Response,
        ctx: &mut FilterContext,
    ) -> Result<FilterAction> {
        if resp.body.len() < self.min_length {
            return Ok(FilterAction::Continue);
        }

        // Check if already compressed
        if resp.headers.contains_key("content-encoding") {
            return Ok(FilterAction::Continue);
        }

        let accept_encoding = ctx.get("accept_encoding").unwrap_or("");

        if accept_encoding.contains("gzip") {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            if encoder.write_all(&resp.body).is_ok() {
                if let Ok(compressed) = encoder.finish() {
                    if compressed.len() < resp.body.len() {
                        resp.body = Bytes::from(compressed);
                        resp.headers.insert("content-encoding", "gzip");
                        resp.headers
                            .insert("content-length", resp.body.len().to_string());
                    }
                }
            }
        } else if accept_encoding.contains("deflate") {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            if encoder.write_all(&resp.body).is_ok() {
                if let Ok(compressed) = encoder.finish() {
                    if compressed.len() < resp.body.len() {
                        resp.body = Bytes::from(compressed);
                        resp.headers.insert("content-encoding", "deflate");
                        resp.headers
                            .insert("content-length", resp.body.len().to_string());
                    }
                }
            }
        }

        Ok(FilterAction::Continue)
    }
}

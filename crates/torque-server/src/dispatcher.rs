//! Request processing pipeline, cluster dispatch, circuit breaking, and telemetry.

use bytes::Bytes;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, warn};

use torque_balance::balancer::ClusterBalancer;
use torque_cache::engine::{CacheLookupResult, HttpCacheEngine};
use torque_filter::pipeline::{FilterAction, FilterChain, FilterContext, FilterStage};
use torque_metrics::registry::MetricsRegistry;
use torque_proto::http1::types::{Method, Request, Response};
use torque_resilience::circuit::CircuitBreaker;
use torque_resilience::ratelimit::RateLimiterRegistry;
use torque_resilience::retry::{Backoff, JitterStrategy};
use torque_router::table::AtomicRouteTable;
use torque_upstream::client::UpstreamClient;

/// Core request processing and traffic routing coordinator.
pub struct RequestDispatcher {
    route_table: Arc<AtomicRouteTable>,
    clusters: Arc<DashMap<String, Arc<ClusterBalancer>>>,
    circuit_breakers: Arc<DashMap<String, Arc<CircuitBreaker>>>,
    rate_limiters: Arc<DashMap<String, Arc<RateLimiterRegistry>>>,
    cache_engine: Arc<HttpCacheEngine>,
    upstream_client: Arc<UpstreamClient>,
    filter_chain: Arc<FilterChain>,
    metrics: Arc<MetricsRegistry>,
}

impl RequestDispatcher {
    /// Construct a new request dispatcher.
    pub fn new(
        route_table: Arc<AtomicRouteTable>,
        clusters: Arc<DashMap<String, Arc<ClusterBalancer>>>,
        circuit_breakers: Arc<DashMap<String, Arc<CircuitBreaker>>>,
        rate_limiters: Arc<DashMap<String, Arc<RateLimiterRegistry>>>,
        cache_engine: Arc<HttpCacheEngine>,
        upstream_client: Arc<UpstreamClient>,
        filter_chain: Arc<FilterChain>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            route_table,
            clusters,
            circuit_breakers,
            rate_limiters,
            cache_engine,
            upstream_client,
            filter_chain,
            metrics,
        }
    }

    /// Primary entry point: process an incoming client HTTP request and return a response.
    pub async fn dispatch(&self, mut req: Request, client_addr: SocketAddr) -> Response {
        let start = Instant::now();
        let mut ctx = FilterContext::new();
        ctx.set("client_ip", client_addr.ip().to_string());
        ctx.set("client_port", client_addr.port().to_string());
        ctx.set("request_uri", req.uri.clone());
        ctx.set("request_method", req.method.as_str().to_string());

        // Stage 1: PreRouting Filters
        match self
            .filter_chain
            .execute_request(FilterStage::PreRouting, &mut req, &mut ctx)
            .await
        {
            Ok(FilterAction::Respond(resp)) => return self.finalize_response(resp, start, &req),
            Ok(FilterAction::Drop) => {
                let resp = Response::new(400);
                return self.finalize_response(resp, start, &req);
            }
            Ok(FilterAction::Continue) => {}
            Err(e) => {
                error!(error = %e, "PreRouting filter execution failed");
                let mut resp = Response::new(500);
                resp.body = Bytes::from_static(b"{\"error\":\"Internal filter error\"}\n");
                return self.finalize_response(resp, start, &req);
            }
        }

        // Stage 2: Route Resolution
        let route_table_snapshot = self.route_table.load();
        let route_match = route_table_snapshot.match_request(&req);

        let (route, matched_params) = match route_match {
            Some(m) => (m.route.clone(), m.params.clone()),
            None => {
                debug!(uri = %req.uri, "No route matched incoming request");
                let mut resp = Response::new(404);
                resp.body = Bytes::from_static(b"{\"error\":\"Not Found\",\"code\":\"TORQUE-E4004\"}\n");
                resp.headers.insert("content-type", "application/json");
                return self.finalize_response(resp, start, &req);
            }
        };

        // Populate route parameters into filter context
        for (param_name, param_val) in matched_params.iter() {
            ctx.set(format!("param:{}", param_name), param_val);
        }
        ctx.set("route_id", route.id.clone());
        ctx.set("cluster_name", route.cluster.clone());

        // Stage 3: PostRouting Filters
        match self
            .filter_chain
            .execute_request(FilterStage::PostRouting, &mut req, &mut ctx)
            .await
        {
            Ok(FilterAction::Respond(resp)) => return self.finalize_response(resp, start, &req),
            Ok(FilterAction::Drop) => {
                let resp = Response::new(400);
                return self.finalize_response(resp, start, &req);
            }
            Ok(FilterAction::Continue) => {}
            Err(e) => {
                error!(error = %e, "PostRouting filter execution failed");
                let mut resp = Response::new(500);
                resp.body = Bytes::from_static(b"{\"error\":\"Internal filter error\"}\n");
                return self.finalize_response(resp, start, &req);
            }
        }

        // Stage 4: Rate Limiting
        if let Some(ref limiter_registry) = self.rate_limiters.get(&route.id) {
            let limit_key = route
                .rate_limit_key
                .as_deref()
                .unwrap_or_else(|| ctx.get("client_ip").unwrap_or("unknown"));

            if let Err(e) = limiter_registry.check(limit_key) {
                warn!(key = %limit_key, "Rate limit exceeded for route {}", route.id);
                let mut resp = Response::new(429);
                resp.body = Bytes::from(format!("{{\"error\":\"{}\"}}\n", e));
                resp.headers.insert("content-type", "application/json");
                resp.headers.insert("retry-after", "1");
                return self.finalize_response(resp, start, &req);
            }
        }

        // Stage 5: Cache Lookup (for GET/HEAD requests)
        if req.method == Method::Get || req.method == Method::Head {
            match self.cache_engine.lookup(&req, &[]) {
                CacheLookupResult::Hit(cached_resp) => {
                    debug!(uri = %req.uri, "Cache hit returned to client");
                    return self.finalize_response(cached_resp, start, &req);
                }
                CacheLookupResult::ConditionalHit(cond_resp) => {
                    debug!(uri = %req.uri, "Conditional 304 cache hit returned to client");
                    return self.finalize_response(cond_resp, start, &req);
                }
                CacheLookupResult::Stale(_) | CacheLookupResult::Miss => {}
            }
        }

        // Stage 6: PreUpstream Filters
        match self
            .filter_chain
            .execute_request(FilterStage::PreUpstream, &mut req, &mut ctx)
            .await
        {
            Ok(FilterAction::Respond(resp)) => return self.finalize_response(resp, start, &req),
            Ok(FilterAction::Drop) => {
                let resp = Response::new(400);
                return self.finalize_response(resp, start, &req);
            }
            Ok(FilterAction::Continue) => {}
            Err(e) => {
                error!(error = %e, "PreUpstream filter execution failed");
                let mut resp = Response::new(500);
                resp.body = Bytes::from_static(b"{\"error\":\"Internal filter error\"}\n");
                return self.finalize_response(resp, start, &req);
            }
        }

        // Stage 7: Cluster and Circuit Breaker Lookup
        let cluster_balancer = match self.clusters.get(&route.cluster) {
            Some(b) => b.clone(),
            None => {
                error!(cluster = %route.cluster, "Target cluster not found");
                let mut resp = Response::new(502);
                resp.body = Bytes::from_static(b"{\"error\":\"Upstream cluster not configured\"}\n");
                resp.headers.insert("content-type", "application/json");
                return self.finalize_response(resp, start, &req);
            }
        };

        if let Some(cb) = self.circuit_breakers.get(&route.cluster) {
            if let Err(e) = cb.allow_request() {
                warn!(cluster = %route.cluster, "Circuit breaker tripped open; rejecting request");
                let mut resp = Response::new(503);
                resp.body = Bytes::from(format!("{{\"error\":\"{}\"}}\n", e));
                resp.headers.insert("content-type", "application/json");
                resp.headers.insert("x-torque-circuit", "OPEN");
                return self.finalize_response(resp, start, &req);
            }
        }

        // Stage 8: Endpoint Selection and Upstream Dispatch with Retries
        let timeout_dur = Duration::from_millis(route.timeout_ms.unwrap_or(5000));
        let max_retries = route.retry_count.unwrap_or(1);
        let backoff = Backoff::new(20, 500, JitterStrategy::Full);
        let mut prev_sleep = Duration::from_millis(20);

        let mut last_error_msg = String::new();
        let mut attempts = 0;

        while attempts <= max_retries {
            attempts += 1;

            let endpoint = match cluster_balancer.select(None) {
                Ok(ep) => ep,
                Err(e) => {
                    error!(cluster = %route.cluster, error = %e, "No healthy endpoint available");
                    let mut resp = Response::new(503);
                    resp.body = Bytes::from_static(b"{\"error\":\"No healthy endpoints available\"}\n");
                    resp.headers.insert("content-type", "application/json");
                    return self.finalize_response(resp, start, &req);
                }
            };

            let call_start = Instant::now();
            match self
                .upstream_client
                .forward(&endpoint, &req, timeout_dur)
                .await
            {
                Ok(mut upstream_resp) => {
                    let call_duration_ms = call_start.elapsed().as_millis() as u64;

                    if let Some(cb) = self.circuit_breakers.get(&route.cluster) {
                        if upstream_resp.status >= 500 {
                            cb.on_failure(call_duration_ms);
                        } else {
                            cb.on_success(call_duration_ms);
                        }
                    }

                    // Store response in cache if configured
                    if let Some(ttl_secs) = route.cache_ttl_secs {
                        if (req.method == Method::Get || req.method == Method::Head)
                            && upstream_resp.status == 200
                        {
                            self.cache_engine.store(
                                &req,
                                &upstream_resp,
                                ttl_secs,
                                vec![route.cluster.clone()],
                            );
                        }
                    }

                    // Stage 9: PostUpstream Filters
                    match self
                        .filter_chain
                        .execute_response(FilterStage::PostUpstream, &mut upstream_resp, &mut ctx)
                        .await
                    {
                        Ok(FilterAction::Respond(override_resp)) => {
                            return self.finalize_response(override_resp, start, &req);
                        }
                        Ok(_) => return self.finalize_response(upstream_resp, start, &req),
                        Err(e) => {
                            error!(error = %e, "PostUpstream filter execution failed");
                            let mut resp = Response::new(500);
                            resp.body = Bytes::from_static(b"{\"error\":\"Internal filter error\"}\n");
                            return self.finalize_response(resp, start, &req);
                        }
                    }
                }
                Err(e) => {
                    let call_duration_ms = call_start.elapsed().as_millis() as u64;
                    if let Some(cb) = self.circuit_breakers.get(&route.cluster) {
                        cb.on_failure(call_duration_ms);
                    }

                    last_error_msg = e.to_string();
                    warn!(
                        attempt = attempts,
                        endpoint = %endpoint.addr,
                        error = %e,
                        "Upstream exchange failed"
                    );

                    if attempts <= max_retries {
                        let delay = backoff.calculate(attempts as u32, prev_sleep);
                        prev_sleep = delay;
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // Exhausted retries: synthesize 504 Gateway Timeout or 502 Bad Gateway
        let mut err_resp = Response::new(504);
        err_resp.body = Bytes::from(format!(
            "{{\"error\":\"Upstream request timed out or failed after {} attempts: {}\"}}\n",
            attempts, last_error_msg
        ));
        err_resp.headers.insert("content-type", "application/json");
        self.finalize_response(err_resp, start, &req)
    }

    /// Record telemetry metrics and finalize outgoing HTTP response.
    fn finalize_response(&self, mut resp: Response, start: Instant, req: &Request) -> Response {
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.metrics.record_request(
            resp.status,
            elapsed_ms,
            resp.body.len(),
            req.body.len(),
        );

        if !resp.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("server")) {
            resp.headers.insert("server", "Torque/0.1.0");
        }

        resp
    }

    /// Access the underlying metrics registry.
    pub fn metrics(&self) -> &Arc<MetricsRegistry> {
        &self.metrics
    }
}

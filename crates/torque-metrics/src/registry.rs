//! Zero-allocation lock-free Prometheus metrics registry.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use super::histogram::{HdrHistogram, BUCKET_BOUNDS_MS};

/// Global operational metrics registry.
pub struct MetricsRegistry {
    pub total_requests: AtomicU64,
    pub status_2xx: AtomicU64,
    pub status_3xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicI64,
    pub latency_histogram: HdrHistogram,
}

impl MetricsRegistry {
    /// Construct a new empty metrics registry.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_requests: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            active_connections: AtomicI64::new(0),
            latency_histogram: HdrHistogram::new(),
        })
    }

    /// Record a completed request execution.
    pub fn record_request(&self, status: u16, duration_ms: f64, sent: usize, recv: usize) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(sent as u64, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(recv as u64, Ordering::Relaxed);
        self.latency_histogram.record(duration_ms);

        match status {
            200..=299 => {
                self.status_2xx.fetch_add(1, Ordering::Relaxed);
            }
            300..=399 => {
                self.status_3xx.fetch_add(1, Ordering::Relaxed);
            }
            400..=499 => {
                self.status_4xx.fetch_add(1, Ordering::Relaxed);
            }
            500..=599 => {
                self.status_5xx.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Record new incoming client TCP connection.
    pub fn inc_connection(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record client TCP connection closed.
    pub fn dec_connection(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Export all counters, gauges, and histogram buckets into Prometheus text format.
    pub fn export_prometheus(&self) -> String {
        let mut out = String::with_capacity(2048);

        // torque_requests_total
        out.push_str("# HELP torque_requests_total Total number of HTTP requests processed\n");
        out.push_str("# TYPE torque_requests_total counter\n");
        out.push_str(&format!(
            "torque_requests_total {}\n\n",
            self.total_requests.load(Ordering::Relaxed)
        ));

        // torque_requests_by_status
        out.push_str("# HELP torque_requests_by_status HTTP requests grouped by response class\n");
        out.push_str("# TYPE torque_requests_by_status counter\n");
        out.push_str(&format!(
            "torque_requests_by_status{{class=\"2xx\"}} {}\n",
            self.status_2xx.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "torque_requests_by_status{{class=\"3xx\"}} {}\n",
            self.status_3xx.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "torque_requests_by_status{{class=\"4xx\"}} {}\n",
            self.status_4xx.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "torque_requests_by_status{{class=\"5xx\"}} {}\n\n",
            self.status_5xx.load(Ordering::Relaxed)
        ));

        // torque_active_connections
        out.push_str(
            "# HELP torque_active_connections Current number of active client TCP sockets\n",
        );
        out.push_str("# TYPE torque_active_connections gauge\n");
        out.push_str(&format!(
            "torque_active_connections {}\n\n",
            self.active_connections.load(Ordering::Relaxed)
        ));

        // torque_bytes
        out.push_str("# HELP torque_bytes_transmitted_total Total bytes sent and received\n");
        out.push_str("# TYPE torque_bytes_transmitted_total counter\n");
        out.push_str(&format!(
            "torque_bytes_transmitted_total{{direction=\"sent\"}} {}\n",
            self.bytes_sent.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "torque_bytes_transmitted_total{{direction=\"received\"}} {}\n\n",
            self.bytes_received.load(Ordering::Relaxed)
        ));

        // torque_request_duration_ms
        out.push_str("# HELP torque_request_duration_ms HTTP request execution latency histogram in milliseconds\n");
        out.push_str("# TYPE torque_request_duration_ms histogram\n");
        let counts = self.latency_histogram.bucket_counts();
        let mut cumulative = 0;
        for (i, &bound) in BUCKET_BOUNDS_MS.iter().enumerate() {
            cumulative += counts[i];
            let le_str = if bound.is_infinite() {
                "+Inf".to_string()
            } else {
                bound.to_string()
            };
            out.push_str(&format!(
                "torque_request_duration_ms_bucket{{le=\"{le_str}\"}} {cumulative}\n"
            ));
        }
        out.push_str(&format!(
            "torque_request_duration_ms_count {}\n\n",
            self.latency_histogram.count()
        ));

        out
    }

    /// Export operational metrics into structured JSON string.
    pub fn export_json(&self) -> String {
        format!(
            "{{\"requests\":{{\"total\":{},\"2xx\":{},\"3xx\":{},\"4xx\":{},\"5xx\":{}}},\"connections\":{},\"bytes\":{{\"sent\":{},\"received\":{}}},\"latency\":{{\"p50\":{:.2},\"p90\":{:.2},\"p99\":{:.2},\"mean\":{:.2}}}}}",
            self.total_requests.load(Ordering::Relaxed),
            self.status_2xx.load(Ordering::Relaxed),
            self.status_3xx.load(Ordering::Relaxed),
            self.status_4xx.load(Ordering::Relaxed),
            self.status_5xx.load(Ordering::Relaxed),
            self.active_connections.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
            self.bytes_received.load(Ordering::Relaxed),
            self.latency_histogram.quantile(0.50),
            self.latency_histogram.quantile(0.90),
            self.latency_histogram.quantile(0.99),
            self.latency_histogram.mean(),
        )
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            active_connections: AtomicI64::new(0),
            latency_histogram: HdrHistogram::new(),
        }
    }
}

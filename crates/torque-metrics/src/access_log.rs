//! High-throughput structured access logging formatter.

use torque_core::time::fast_now_millis;

/// Structured record representing an individual processed request.
#[derive(Debug, Clone)]
pub struct AccessLogRecord {
    pub client_ip: String,
    pub method: String,
    pub uri: String,
    pub status: u16,
    pub duration_ms: f64,
    pub bytes_sent: usize,
    pub cluster: String,
    pub trace_id: Option<String>,
}

impl AccessLogRecord {
    /// Serialize record into single-line JSON format.
    pub fn to_json_line(&self) -> String {
        let trace = self.trace_id.as_deref().unwrap_or("-");
        format!(
            "{{\"ts\":{},\"client_ip\":\"{}\",\"method\":\"{}\",\"uri\":\"{}\",\"status\":{},\"duration_ms\":{:.2},\"bytes_sent\":{},\"cluster\":\"{}\",\"trace_id\":\"{}\"}}\n",
            fast_now_millis(),
            self.client_ip,
            self.method,
            self.uri,
            self.status,
            self.duration_ms,
            self.bytes_sent,
            self.cluster,
            trace
        )
    }
}

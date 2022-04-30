//! High-throughput asynchronous HTTP benchmarking engine and latency profiler.

use anyhow::Result;
use bytes::BytesMut;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use torque_metrics::histogram::HdrHistogram;
use torque_proto::http1::parser::{Http1Parser, ParseResult};

/// Benchmarking run parameters and target specification.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub target_addr: SocketAddr,
    pub path: String,
    pub concurrency: usize,
    pub duration: Duration,
    pub pipeline_depth: usize,
}

/// Comprehensive benchmark outcome and latency percentiles.
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub elapsed: Duration,
    pub requests_per_sec: f64,
    pub bytes_received: u64,
    pub throughput_mb_sec: f64,
    pub latency_mean_ms: f64,
    pub latency_p50_ms: f64,
    pub latency_p90_ms: f64,
    pub latency_p99_ms: f64,
    pub latency_p999_ms: f64,
}

impl BenchResult {
    /// Render formatted ASCII summary report.
    pub fn print_report(&self) {
        println!("===============================================================================");
        println!("  TORQUE LOAD BENCHMARK EXECUTION REPORT");
        println!("===============================================================================");
        println!("  Timing & Throughput:");
        println!(
            "    Test Duration:       {:.3} s",
            self.elapsed.as_secs_f64()
        );
        println!("    Total Transactions:  {}", self.total_requests);
        println!("    Completed Success:   {}", self.successful_requests);
        println!("    Failed / Errors:     {}", self.failed_requests);
        println!(
            "    Mean Throughput:     {:.1} req/sec",
            self.requests_per_sec
        );
        println!(
            "    Network Bandwidth:   {:.2} MB/sec",
            self.throughput_mb_sec
        );
        println!();
        println!("  Status Code Distribution:");
        println!("    2xx (Success):       {}", self.status_2xx);
        println!("    3xx (Redirect):      {}", self.status_3xx);
        println!("    4xx (Client Error):  {}", self.status_4xx);
        println!("    5xx (Server Error):  {}", self.status_5xx);
        println!();
        println!("  Latency Percentiles (HDR logarithmic):");
        println!("    Mean:                {:.2} ms", self.latency_mean_ms);
        println!("    p50 (Median):        {:.2} ms", self.latency_p50_ms);
        println!("    p90:                 {:.2} ms", self.latency_p90_ms);
        println!("    p99:                 {:.2} ms", self.latency_p99_ms);
        println!("    p99.9:               {:.2} ms", self.latency_p999_ms);
        println!("===============================================================================");
    }

    /// Export benchmark metrics into CSV format.
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str("metric,value\n");
        csv.push_str(&format!("duration_sec,{:.3}\n", self.elapsed.as_secs_f64()));
        csv.push_str(&format!("total_requests,{}\n", self.total_requests));
        csv.push_str(&format!(
            "successful_requests,{}\n",
            self.successful_requests
        ));
        csv.push_str(&format!("failed_requests,{}\n", self.failed_requests));
        csv.push_str(&format!("requests_per_sec,{:.1}\n", self.requests_per_sec));
        csv.push_str(&format!(
            "throughput_mb_sec,{:.2}\n",
            self.throughput_mb_sec
        ));
        csv.push_str(&format!("latency_mean_ms,{:.2}\n", self.latency_mean_ms));
        csv.push_str(&format!("latency_p50_ms,{:.2}\n", self.latency_p50_ms));
        csv.push_str(&format!("latency_p90_ms,{:.2}\n", self.latency_p90_ms));
        csv.push_str(&format!("latency_p99_ms,{:.2}\n", self.latency_p99_ms));
        csv.push_str(&format!("latency_p999_ms,{:.2}\n", self.latency_p999_ms));
        csv
    }
}

/// Core benchmarking runner.
pub struct BenchEngine {
    config: BenchConfig,
}

impl BenchEngine {
    pub fn new(config: BenchConfig) -> Self {
        Self { config }
    }

    /// Execute the configured benchmark against the target server.
    pub async fn run(&self) -> Result<BenchResult> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let total_requests = Arc::new(AtomicU64::new(0));
        let successful_requests = Arc::new(AtomicU64::new(0));
        let failed_requests = Arc::new(AtomicU64::new(0));
        let status_2xx = Arc::new(AtomicU64::new(0));
        let status_3xx = Arc::new(AtomicU64::new(0));
        let status_4xx = Arc::new(AtomicU64::new(0));
        let status_5xx = Arc::new(AtomicU64::new(0));
        let total_bytes = Arc::new(AtomicU64::new(0));
        let hist = Arc::new(Mutex::new(HdrHistogram::new()));

        let request_string = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nAccept: */*\r\n\r\n",
            self.config.path, self.config.target_addr
        );
        let request_bytes: Arc<Vec<u8>> = Arc::new(request_string.into_bytes());

        let mut handles = Vec::new();
        let start = Instant::now();

        // Spawn timer task to trigger stop_flag after configured duration
        let stop_flag_timer = stop_flag.clone();
        let run_duration = self.config.duration;
        tokio::spawn(async move {
            tokio::time::sleep(run_duration).await;
            stop_flag_timer.store(true, Ordering::Relaxed);
        });

        // Spawn worker tasks
        for _ in 0..self.config.concurrency {
            let stop = stop_flag.clone();
            let total = total_requests.clone();
            let success = successful_requests.clone();
            let failed = failed_requests.clone();
            let s2xx = status_2xx.clone();
            let s3xx = status_3xx.clone();
            let s4xx = status_4xx.clone();
            let s5xx = status_5xx.clone();
            let bytes_rec = total_bytes.clone();
            let h = hist.clone();
            let req_data = request_bytes.clone();
            let target = self.config.target_addr;
            let pipeline_depth = self.config.pipeline_depth.max(1);

            handles.push(tokio::spawn(async move {
                let mut read_buf = BytesMut::with_capacity(32768);

                while !stop.load(Ordering::Relaxed) {
                    let mut stream = match TcpStream::connect(target).await {
                        Ok(s) => {
                            let _ = s.set_nodelay(true);
                            s
                        }
                        Err(_) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                            tokio::time::sleep(Duration::from_millis(5)).await;
                            continue;
                        }
                    };

                    while !stop.load(Ordering::Relaxed) {
                        let call_start = Instant::now();

                        // Write pipelined requests
                        let mut send_buf = Vec::with_capacity(req_data.len() * pipeline_depth);
                        for _ in 0..pipeline_depth {
                            send_buf.extend_from_slice(&req_data);
                        }

                        if stream.write_all(&send_buf).await.is_err() {
                            failed.fetch_add(1, Ordering::Relaxed);
                            break;
                        }

                        let mut responses_received = 0;
                        let mut socket_err = false;

                        while responses_received < pipeline_depth {
                            match stream.read_buf(&mut read_buf).await {
                                Ok(0) => {
                                    socket_err = true;
                                    break;
                                }
                                Ok(n) => {
                                    bytes_rec.fetch_add(n as u64, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    socket_err = true;
                                    break;
                                }
                            }

                            while let Ok(parse_res) = Http1Parser::parse_response(&read_buf) {
                                match parse_res {
                                    ParseResult::Complete(resp, consumed) => {
                                        let _ = read_buf.split_to(consumed);
                                        let elapsed_ms =
                                            call_start.elapsed().as_secs_f64() * 1000.0;
                                        h.lock().record(elapsed_ms);

                                        total.fetch_add(1, Ordering::Relaxed);
                                        success.fetch_add(1, Ordering::Relaxed);

                                        match resp.status / 100 {
                                            2 => s2xx.fetch_add(1, Ordering::Relaxed),
                                            3 => s3xx.fetch_add(1, Ordering::Relaxed),
                                            4 => s4xx.fetch_add(1, Ordering::Relaxed),
                                            5 => s5xx.fetch_add(1, Ordering::Relaxed),
                                            _ => 0,
                                        };

                                        responses_received += 1;
                                        if responses_received >= pipeline_depth {
                                            break;
                                        }
                                    }
                                    ParseResult::Incomplete => break,
                                }
                            }
                        }

                        if socket_err {
                            break;
                        }
                    }
                }
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        let elapsed = start.elapsed();
        let total = total_requests.load(Ordering::Relaxed);
        let succ = successful_requests.load(Ordering::Relaxed);
        let fail = failed_requests.load(Ordering::Relaxed);
        let rps = if elapsed.as_secs_f64() > 0.0 {
            succ as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };
        let b = total_bytes.load(Ordering::Relaxed);
        let mb_sec = (b as f64 / 1_048_576.0) / elapsed.as_secs_f64();

        let h_guard = hist.lock();

        Ok(BenchResult {
            total_requests: total,
            successful_requests: succ,
            failed_requests: fail,
            status_2xx: status_2xx.load(Ordering::Relaxed),
            status_3xx: status_3xx.load(Ordering::Relaxed),
            status_4xx: status_4xx.load(Ordering::Relaxed),
            status_5xx: status_5xx.load(Ordering::Relaxed),
            elapsed,
            requests_per_sec: rps,
            bytes_received: b,
            throughput_mb_sec: mb_sec,
            latency_mean_ms: h_guard.mean(),
            latency_p50_ms: h_guard.quantile(0.50),
            latency_p90_ms: h_guard.quantile(0.90),
            latency_p99_ms: h_guard.quantile(0.99),
            latency_p999_ms: h_guard.quantile(0.999),
        })
    }
}

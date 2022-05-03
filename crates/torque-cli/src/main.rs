//! Torque CLI: control plane utility, daemon launcher, and operational inspection tool.

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::info;
use tracing_subscriber::EnvFilter;

use torque_config::loader::load_from_file;
use torque_metrics::histogram::HdrHistogram;
use torque_proto::http1::parser::{Http1Parser, ParseResult};
use torque_server::TorqueServer;

#[derive(Parser, Debug)]
#[command(
    name = "torque",
    version = "0.1.0",
    about = "Zero-allocation programmable Layer-7 reverse proxy and service mesh data plane",
    long_about = "Torque is an asynchronous, zero-allocation Layer-7 reverse proxy, edge traffic router, and service mesh data plane in Rust."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Torque reverse proxy daemon
    Start(StartArgs),
    /// Validate configuration syntax and semantic integrity
    Check(CheckArgs),
    /// Signal running daemon to hot-reload configuration
    Reload(ReloadArgs),
    /// Live terminal telemetry dashboard
    Top(TopArgs),
    /// Run high-throughput HTTP load benchmark
    Bench(BenchArgs),
}

#[derive(Args, Debug)]
struct StartArgs {
    /// Path to configuration file (YAML, JSON, or TOML)
    #[arg(short, long, default_value = "torque.yaml")]
    config: PathBuf,

    /// Log level filter (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Format logs as JSON lines
    #[arg(long)]
    json_logs: bool,
}

#[derive(Args, Debug)]
struct CheckArgs {
    /// Path to configuration file to validate
    #[arg(short, long, default_value = "torque.yaml")]
    config: PathBuf,
}

#[derive(Args, Debug)]
struct ReloadArgs {
    /// Admin server address (host:port)
    #[arg(long, default_value = "127.0.0.1:9090")]
    admin_addr: String,
}

#[derive(Args, Debug)]
struct TopArgs {
    /// Admin server address (host:port)
    #[arg(long, default_value = "127.0.0.1:9090")]
    admin_addr: String,

    /// Sampling interval in seconds
    #[arg(short, long, default_value_t = 1)]
    interval: u64,

    /// Run single iteration and exit
    #[arg(long)]
    once: bool,
}

#[derive(Args, Debug)]
struct BenchArgs {
    /// Target host:port address (e.g. 127.0.0.1:8080)
    #[arg(short, long)]
    target: String,

    /// Target URI path
    #[arg(short, long, default_value = "/")]
    path: String,

    /// Number of concurrent connection workers
    #[arg(short, long, default_value_t = 16)]
    concurrency: usize,

    /// Total number of requests to execute
    #[arg(short, long, default_value_t = 10_000)]
    requests: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start(args) => run_start(args).await,
        Commands::Check(args) => run_check(args),
        Commands::Reload(args) => run_reload(args).await,
        Commands::Top(args) => run_top(args).await,
        Commands::Bench(args) => run_bench(args).await,
    }
}

async fn run_start(args: StartArgs) -> Result<()> {
    // Initialize logging subscriber
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    info!(config = %args.config.display(), "Initializing Torque proxy daemon");

    if !args.config.exists() {
        bail!("Configuration file not found: {}", args.config.display());
    }

    let server = TorqueServer::from_file(&args.config)
        .await
        .context("Failed initializing Torque server")?;

    info!(
        proxy_addr = %server.server_addr(),
        admin_addr = ?server.admin_addr(),
        "Torque runtime initialized; entering event reactor loop"
    );

    server.run().await.context("Server runtime error")?;
    Ok(())
}

fn run_check(args: CheckArgs) -> Result<()> {
    println!("Checking configuration: {}", args.config.display());

    let config = load_from_file(&args.config).context("Configuration check failed")?;

    println!("\nConfiguration Integrity: VALID");
    println!("--------------------------------------------------");
    println!("Proxy Listener:");
    println!("  Bind Address:    {}", config.server.bind_addr);
    println!("  Worker Threads:  {}", config.server.worker_threads);
    println!("  Max Connections: {}", config.server.max_connections);
    println!("  Buffer Size:     {} KB", config.server.buffer_size_kb);
    println!("  TCP NoDelay:     {}", config.server.tcp_nodelay);

    println!("\nAdmin Plane:");
    println!("  Bind Address:    {}", config.admin.bind_addr);
    println!("  Dashboard:       {}", config.admin.enable_dashboard);
    println!("  Metrics Path:    {}", config.admin.metrics_path);

    println!("\nClusters ({} configured):", config.clusters.len());
    for cluster in &config.clusters {
        println!(
            "  - Cluster '{}': policy = {}, endpoints = {}",
            cluster.name,
            cluster.lb_policy,
            cluster.endpoints.len()
        );
        for ep in &cluster.endpoints {
            println!("      * {} (weight: {})", ep.addr, ep.weight);
        }
    }

    println!("\nRoutes ({} configured):", config.routes.len());
    for route in &config.routes {
        let methods_str = if route.methods.is_empty() {
            "ALL".to_string()
        } else {
            route.methods.join(", ")
        };
        println!(
            "  - Route '{}': {} {} -> cluster '{}'",
            route.name, methods_str, route.path, route.cluster
        );
    }

    println!("--------------------------------------------------");
    println!("0 errors detected. Configuration is ready for production.");
    Ok(())
}

async fn run_reload(args: ReloadArgs) -> Result<()> {
    println!(
        "Sending hot reload signal to admin server at {}...",
        args.admin_addr
    );

    let mut stream = TcpStream::connect(&args.admin_addr)
        .await
        .with_context(|| format!("Failed to connect to admin server at {}", args.admin_addr))?;

    let req = format!(
        "POST /config/reload HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        args.admin_addr
    );
    stream.write_all(req.as_bytes()).await?;
    stream.flush().await?;

    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await?;

    match Http1Parser::parse_response(&resp_buf)? {
        ParseResult::Complete(resp, _) => {
            if resp.status == 200 {
                println!("SUCCESS: Configuration hot-reloaded successfully.");
                println!("{}", String::from_utf8_lossy(&resp.body));
                Ok(())
            } else {
                bail!(
                    "FAILED: Server returned HTTP {} {}: {}",
                    resp.status,
                    resp.reason,
                    String::from_utf8_lossy(&resp.body)
                );
            }
        }
        ParseResult::Incomplete => bail!("Incomplete response received from admin server"),
    }
}

async fn run_top(args: TopArgs) -> Result<()> {
    loop {
        let mut stream = match TcpStream::connect(&args.admin_addr).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Error connecting to admin server at {}: {}",
                    args.admin_addr, e
                );
                if args.once {
                    return Err(e.into());
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(args.interval)).await;
                continue;
            }
        };

        let req = format!(
            "GET /stats HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            args.admin_addr
        );
        let _ = stream.write_all(req.as_bytes()).await;
        let _ = stream.flush().await;

        let mut resp_buf = Vec::new();
        let _ = stream.read_to_end(&mut resp_buf).await;

        if let Ok(ParseResult::Complete(resp, _)) = Http1Parser::parse_response(&resp_buf) {
            if resp.status == 200 {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&resp.body) {
                    print!("\x1B[2J\x1B[1;1H"); // Clear screen
                    println!("===============================================================================");
                    println!("  TORQUE REAL-TIME TELEMETRY MONITOR (/stats)");
                    println!(
                        "  Admin Host: {} | Refresh: {}s",
                        args.admin_addr, args.interval
                    );
                    println!("===============================================================================");
                    let reqs = &v["requests"];
                    println!("  Traffic Summary:");
                    println!("    Total Requests:     {}", reqs["total"]);
                    println!("    Status 2xx (OK):    {}", reqs["2xx"]);
                    println!("    Status 3xx (Redir): {}", reqs["3xx"]);
                    println!("    Status 4xx (Client):{}", reqs["4xx"]);
                    println!("    Status 5xx (Server):{}", reqs["5xx"]);
                    println!();
                    println!("  Connections:");
                    println!("    Active TCP Sockets: {}", v["connections"]);
                    println!();
                    let bytes = &v["bytes"];
                    let sent_kb = bytes["sent"].as_f64().unwrap_or(0.0) / 1024.0;
                    let recv_kb = bytes["received"].as_f64().unwrap_or(0.0) / 1024.0;
                    println!("  Bandwidth:");
                    println!("    Bytes Sent:         {sent_kb:.2} KB");
                    println!("    Bytes Received:     {recv_kb:.2} KB");
                    println!();
                    let lat = &v["latency"];
                    println!("  Latency Distribution (HDR logarithmic):");
                    println!("    Mean:               {:.2} ms", lat["mean"]);
                    println!("    p50 (Median):       {:.2} ms", lat["p50"]);
                    println!("    p90:                {:.2} ms", lat["p90"]);
                    println!("    p99 (Tail):         {:.2} ms", lat["p99"]);
                    println!("===============================================================================");
                }
            }
        }

        if args.once {
            break;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(args.interval)).await;
    }

    Ok(())
}

async fn run_bench(args: BenchArgs) -> Result<()> {
    println!("===============================================================================");
    println!("  TORQUE HTTP LOAD BENCHMARK");
    println!("  Target: http://{}{}", args.target, args.path);
    println!(
        "  Concurrency: {} workers | Requests: {}",
        args.concurrency, args.requests
    );
    println!("===============================================================================");

    let requests_per_worker = args.requests / args.concurrency;
    let completed = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let hist = Arc::new(parking_lot::Mutex::new(HdrHistogram::new()));

    let target_addr: SocketAddr = args.target.parse().context("Invalid target host:port")?;
    let path = args.path.clone();

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..args.concurrency {
        let comp = completed.clone();
        let errs = errors.clone();
        let h = hist.clone();
        let path_clone = path.clone();

        handles.push(tokio::spawn(async move {
            let req_str = format!(
                "GET {path_clone} HTTP/1.1\r\nHost: {target_addr}\r\nConnection: keep-alive\r\n\r\n"
            );
            let req_bytes = req_str.as_bytes();

            for _ in 0..requests_per_worker {
                let call_start = Instant::now();
                match TcpStream::connect(target_addr).await {
                    Ok(mut stream) => {
                        if stream.write_all(req_bytes).await.is_err() {
                            errs.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        let mut buf = [0u8; 1024];
                        if stream.read(&mut buf).await.is_err() {
                            errs.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }

                        let elapsed_ms = call_start.elapsed().as_secs_f64() * 1000.0;
                        h.lock().record(elapsed_ms);
                        comp.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errs.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let elapsed = start.elapsed();
    let total_done = completed.load(Ordering::Relaxed);
    let total_errs = errors.load(Ordering::Relaxed);
    let rps = total_done as f64 / elapsed.as_secs_f64();

    let guard = hist.lock();
    println!("\nResults:");
    println!("  Total Duration:     {:.3} s", elapsed.as_secs_f64());
    println!("  Successful Calls:   {total_done}");
    println!("  Failed Calls:       {total_errs}");
    println!("  Throughput:         {rps:.1} requests/sec");
    println!();
    println!("Latency percentiles:");
    println!("  p50:                {:.2} ms", guard.quantile(0.50));
    println!("  p90:                {:.2} ms", guard.quantile(0.90));
    println!("  p99:                {:.2} ms", guard.quantile(0.99));
    println!("  Mean:               {:.2} ms", guard.mean());
    println!("===============================================================================");

    Ok(())
}

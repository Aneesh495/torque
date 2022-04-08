//! Command-line entrypoint for the dedicated Torque load generator and benchmarking tool.

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use torque_bench::engine::{BenchConfig, BenchEngine};

#[derive(Parser, Debug)]
#[command(
    name = "torque-bench",
    version = "0.1.0",
    about = "Ultra high-throughput load generator and latency benchmarking harness for Torque"
)]
struct BenchCli {
    /// Target server socket address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    target: String,

    /// Request path
    #[arg(short, long, default_value = "/")]
    path: String,

    /// Number of concurrent connection workers
    #[arg(short, long, default_value_t = 32)]
    concurrency: usize,

    /// Test duration in seconds
    #[arg(short, long, default_value_t = 5)]
    duration: u64,

    /// Pipelining depth per TCP connection
    #[arg(long, default_value_t = 1)]
    pipeline: usize,

    /// Optional path to export CSV report
    #[arg(long)]
    csv: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = BenchCli::parse();

    let target_addr: SocketAddr = cli
        .target
        .parse()
        .with_context(|| format!("Invalid socket address: {}", cli.target))?;

    let config = BenchConfig {
        target_addr,
        path: cli.path.clone(),
        concurrency: cli.concurrency,
        duration: Duration::from_secs(cli.duration),
        pipeline_depth: cli.pipeline,
    };

    println!("Starting Torque benchmark:");
    println!("  Target:        {}", config.target_addr);
    println!("  Path:          {}", config.path);
    println!("  Concurrency:   {} connections", config.concurrency);
    println!("  Duration:      {} seconds", cli.duration);
    println!("  Pipelining:    depth {}", config.pipeline_depth);
    println!("\nWarming up worker sockets and executing load test...");

    let engine = BenchEngine::new(config);
    let result = engine.run().await.context("Benchmark execution failed")?;

    result.print_report();

    if let Some(csv_path) = cli.csv {
        fs::write(&csv_path, result.to_csv())
            .with_context(|| format!("Failed writing CSV report to {}", csv_path.display()))?;
        println!("Report exported to: {}", csv_path.display());
    }

    Ok(())
}

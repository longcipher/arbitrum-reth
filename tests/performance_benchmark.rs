//! 性能基准测试工具
//!
//! 对比 Arbitrum-Reth 与 Nitro 的性能指标

use clap::Parser;
use eyre::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "performance-benchmark")]
#[command(about = "Benchmark performance between Arbitrum-Reth and Nitro")]
struct Args {
    /// Test duration in seconds
    #[arg(long, default_value = "3600")]
    duration: u64,

    /// Report interval in seconds
    #[arg(long, default_value = "60")]
    report_interval: u64,

    /// Output file for results
    #[arg(long)]
    output: Option<PathBuf>,

    /// Nitro node RPC endpoint
    #[arg(long, default_value = "http://localhost:8547")]
    nitro_endpoint: String,

    /// Arbitrum-Reth node RPC endpoint
    #[arg(long, default_value = "http://localhost:8548")]
    reth_endpoint: String,

    /// Number of concurrent connections
    #[arg(long, default_value = "10")]
    concurrent: usize,

    /// Warmup duration in seconds
    #[arg(long, default_value = "60")]
    warmup: u64,

    /// Target transactions per second
    #[arg(long, default_value = "100")]
    target_tps: u64,
}

#[derive(Debug, Clone)]
struct PerformanceMetrics {
    timestamp: u64,
    requests_per_second: f64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    success_rate: f64,
    memory_usage_mb: f64,
    cpu_usage_percent: f64,
}

#[derive(Debug)]
struct BenchmarkResult {
    node_type: String,
    endpoint: String,
    start_time: String,
    end_time: String,
    duration_seconds: u64,
    total_requests: u64,
    total_successes: u64,
    total_failures: u64,
    avg_tps: f64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    memory_stats: MemoryStats,
    cpu_stats: CpuStats,
    metrics_timeline: Vec<PerformanceMetrics>,
}

#[derive(Debug, Clone)]
struct MemoryStats {
    avg_usage_mb: f64,
    max_usage_mb: f64,
    min_usage_mb: f64,
}

#[derive(Debug, Clone)]
struct CpuStats {
    avg_usage_percent: f64,
    max_usage_percent: f64,
    min_usage_percent: f64,
}

struct NodeBenchmarker {
    client: reqwest::Client,
    endpoint: String,
    node_type: String,
    metrics: Arc<RwLock<Vec<Duration>>>,
    success_count: Arc<AtomicU64>,
    failure_count: Arc<AtomicU64>,
    request_count: Arc<AtomicU64>,
}

impl NodeBenchmarker {
    fn new(endpoint: String, node_type: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            endpoint,
            node_type,
            metrics: Arc::new(RwLock::new(Vec::new())),
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            request_count: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn benchmark(&self, duration: Duration, target_tps: u64, concurrent: usize) -> Result<BenchmarkResult> {
        info!("Starting {} benchmark for {} seconds", self.node_type, duration.as_secs());
        
        let start_time = Instant::now();
        let start_timestamp = chrono::Utc::now();

        // 启动监控任务
        let monitoring_handle = self.start_monitoring(duration).await;

        // 启动负载生成器
        let load_generators = self.start_load_generators(duration, target_tps, concurrent).await;

        // 等待所有任务完成
        for handle in load_generators {
            handle.await?;
        }
        monitoring_handle.await?;

        let end_time = Instant::now();
        let end_timestamp = chrono::Utc::now();

        // 计算统计信息
        let metrics = self.metrics.read().await;
        let total_requests = self.request_count.load(Ordering::SeqCst);
        let total_successes = self.success_count.load(Ordering::SeqCst);
        let total_failures = self.failure_count.load(Ordering::SeqCst);

        let latencies_ms: Vec<f64> = metrics.iter().map(|d| d.as_millis() as f64).collect();
        
        let avg_latency = if !latencies_ms.is_empty() {
            latencies_ms.iter().sum::<f64>() / latencies_ms.len() as f64
        } else {
            0.0
        };

        let (p95_latency, p99_latency, min_latency, max_latency) = if !latencies_ms.is_empty() {
            let mut sorted = latencies_ms.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let p95_idx = (sorted.len() as f64 * 0.95) as usize;
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            
            (
                sorted.get(p95_idx).copied().unwrap_or(0.0),
                sorted.get(p99_idx).copied().unwrap_or(0.0),
                sorted.first().copied().unwrap_or(0.0),
                sorted.last().copied().unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        let actual_duration = end_time.duration_since(start_time);
        let avg_tps = total_successes as f64 / actual_duration.as_secs_f64();

        Ok(BenchmarkResult {
            node_type: self.node_type.clone(),
            endpoint: self.endpoint.clone(),
            start_time: start_timestamp.to_rfc3339(),
            end_time: end_timestamp.to_rfc3339(),
            duration_seconds: actual_duration.as_secs(),
            total_requests,
            total_successes,
            total_failures,
            avg_tps,
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            memory_stats: MemoryStats {
                avg_usage_mb: 0.0, // TODO: 实现系统监控
                max_usage_mb: 0.0,
                min_usage_mb: 0.0,
            },
            cpu_stats: CpuStats {
                avg_usage_percent: 0.0,
                max_usage_percent: 0.0,
                min_usage_percent: 0.0,
            },
            metrics_timeline: Vec::new(), // TODO: 实现时间线监控
        })
    }

    async fn start_monitoring(&self, duration: Duration) -> tokio::task::JoinHandle<()> {
        let endpoint = self.endpoint.clone();
        
        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(5));
            let start = Instant::now();

            while start.elapsed() < duration {
                monitor_interval.tick().await;
                
                // TODO: 实现系统资源监控
                // - 内存使用量
                // - CPU 使用率
                // - 网络 I/O
                // - 磁盘 I/O
            }
        })
    }

    async fn start_load_generators(
        &self,
        duration: Duration,
        target_tps: u64,
        concurrent: usize,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();
        let requests_per_worker = target_tps / concurrent as u64;

        for worker_id in 0..concurrent {
            let client = self.client.clone();
            let endpoint = self.endpoint.clone();
            let metrics = self.metrics.clone();
            let success_count = self.success_count.clone();
            let failure_count = self.failure_count.clone();
            let request_count = self.request_count.clone();

            let handle = tokio::spawn(async move {
                let mut request_interval = interval(Duration::from_millis(1000 / requests_per_worker));
                let start = Instant::now();

                while start.elapsed() < duration {
                    request_interval.tick().await;

                    let req_start = Instant::now();
                    request_count.fetch_add(1, Ordering::SeqCst);

                    // 发送测试请求
                    let request = json!({
                        "jsonrpc": "2.0",
                        "method": "eth_blockNumber",
                        "params": [],
                        "id": worker_id
                    });

                    match client
                        .post(&endpoint)
                        .header("Content-Type", "application/json")
                        .json(&request)
                        .send()
                        .await
                    {
                        Ok(response) => {
                            let latency = req_start.elapsed();
                            
                            if response.status().is_success() {
                                success_count.fetch_add(1, Ordering::SeqCst);
                                metrics.write().await.push(latency);
                            } else {
                                failure_count.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        Err(_) => {
                            failure_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        handles
    }
}

struct BenchmarkSuite {
    nitro_benchmarker: NodeBenchmarker,
    reth_benchmarker: NodeBenchmarker,
}

impl BenchmarkSuite {
    fn new(nitro_endpoint: String, reth_endpoint: String) -> Self {
        Self {
            nitro_benchmarker: NodeBenchmarker::new(nitro_endpoint, "Nitro".to_string()),
            reth_benchmarker: NodeBenchmarker::new(reth_endpoint, "Arbitrum-Reth".to_string()),
        }
    }

    async fn run_benchmark(
        &self,
        duration: Duration,
        target_tps: u64,
        concurrent: usize,
        warmup: Duration,
    ) -> Result<(BenchmarkResult, BenchmarkResult)> {
        info!("Starting warmup phase for {} seconds", warmup.as_secs());
        
        // 预热阶段
        let warmup_duration = Duration::from_secs(30);
        let (_, _) = tokio::join!(
            self.nitro_benchmarker.benchmark(warmup_duration, target_tps / 10, concurrent / 2),
            self.reth_benchmarker.benchmark(warmup_duration, target_tps / 10, concurrent / 2)
        );

        info!("Warmup completed, starting actual benchmark");
        sleep(Duration::from_secs(5)).await;

        // 实际基准测试
        let (nitro_result, reth_result) = tokio::join!(
            self.nitro_benchmarker.benchmark(duration, target_tps, concurrent),
            self.reth_benchmarker.benchmark(duration, target_tps, concurrent)
        );

        Ok((nitro_result?, reth_result?))
    }

    fn generate_comparison_report(
        &self,
        nitro_result: &BenchmarkResult,
        reth_result: &BenchmarkResult,
    ) -> Value {
        let nitro_tps = nitro_result.avg_tps;
        let reth_tps = reth_result.avg_tps;
        let tps_improvement = if nitro_tps > 0.0 {
            reth_tps / nitro_tps
        } else {
            0.0
        };

        let nitro_latency = nitro_result.avg_latency_ms;
        let reth_latency = reth_result.avg_latency_ms;
        let latency_ratio = if nitro_latency > 0.0 {
            reth_latency / nitro_latency
        } else {
            0.0
        };

        json!({
            "benchmark_summary": {
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "test_duration_seconds": nitro_result.duration_seconds,
                "concurrent_connections": "calculated from TPS and latency"
            },
            "performance_comparison": {
                "throughput": {
                    "nitro_tps": nitro_tps,
                    "reth_tps": reth_tps,
                    "improvement_factor": tps_improvement,
                    "improvement_percentage": (tps_improvement - 1.0) * 100.0
                },
                "latency": {
                    "nitro_avg_ms": nitro_latency,
                    "reth_avg_ms": reth_latency,
                    "nitro_p95_ms": nitro_result.p95_latency_ms,
                    "reth_p95_ms": reth_result.p95_latency_ms,
                    "nitro_p99_ms": nitro_result.p99_latency_ms,
                    "reth_p99_ms": reth_result.p99_latency_ms,
                    "latency_ratio": latency_ratio
                },
                "reliability": {
                    "nitro_success_rate": nitro_result.total_successes as f64 / nitro_result.total_requests as f64,
                    "reth_success_rate": reth_result.total_successes as f64 / reth_result.total_requests as f64
                }
            },
            "detailed_results": {
                "nitro": nitro_result,
                "arbitrum_reth": reth_result
            },
            "verdict": {
                "meets_10x_target": tps_improvement >= 10.0,
                "faster_than_nitro": tps_improvement > 1.0,
                "lower_latency": latency_ratio < 1.0,
                "overall_grade": self.calculate_grade(tps_improvement, latency_ratio, 
                    nitro_result.total_successes as f64 / nitro_result.total_requests as f64,
                    reth_result.total_successes as f64 / reth_result.total_requests as f64)
            }
        })
    }

    fn calculate_grade(&self, tps_improvement: f64, latency_ratio: f64, nitro_success: f64, reth_success: f64) -> String {
        let mut score = 0;

        // TPS 评分
        if tps_improvement >= 10.0 { score += 40; }
        else if tps_improvement >= 5.0 { score += 30; }
        else if tps_improvement >= 2.0 { score += 20; }
        else if tps_improvement >= 1.0 { score += 10; }

        // 延迟评分
        if latency_ratio <= 0.5 { score += 30; }
        else if latency_ratio <= 0.8 { score += 20; }
        else if latency_ratio <= 1.0 { score += 10; }

        // 可靠性评分
        if reth_success >= 0.99 && nitro_success >= 0.99 { score += 30; }
        else if reth_success >= 0.95 && nitro_success >= 0.95 { score += 20; }
        else if reth_success >= 0.90 && nitro_success >= 0.90 { score += 10; }

        match score {
            90..=100 => "A+ (Excellent - Exceeds all targets)".to_string(),
            80..=89 => "A (Very Good - Meets most targets)".to_string(),
            70..=79 => "B (Good - Decent performance)".to_string(),
            60..=69 => "C (Average - Needs improvement)".to_string(),
            _ => "D (Poor - Significant issues)".to_string(),
        }
    }

    fn print_results(&self, nitro_result: &BenchmarkResult, reth_result: &BenchmarkResult) {
        println!("\n⚡ Performance Benchmark Results");
        println!("===============================");
        
        println!("\n📊 Throughput Comparison:");
        println!("  Nitro TPS: {:.1}", nitro_result.avg_tps);
        println!("  Reth TPS:  {:.1}", reth_result.avg_tps);
        
        let tps_improvement = if nitro_result.avg_tps > 0.0 {
            reth_result.avg_tps / nitro_result.avg_tps
        } else {
            0.0
        };
        
        if tps_improvement > 1.0 {
            println!("  🚀 Reth is {:.1}x faster!", tps_improvement);
        } else {
            println!("  ⚠️  Reth is {:.1}x slower", 1.0 / tps_improvement);
        }

        println!("\n⏱️  Latency Comparison:");
        println!("  Nitro avg: {:.1}ms", nitro_result.avg_latency_ms);
        println!("  Reth avg:  {:.1}ms", reth_result.avg_latency_ms);
        println!("  Nitro P95: {:.1}ms", nitro_result.p95_latency_ms);
        println!("  Reth P95:  {:.1}ms", reth_result.p95_latency_ms);

        println!("\n✅ Reliability:");
        let nitro_success_rate = nitro_result.total_successes as f64 / nitro_result.total_requests as f64;
        let reth_success_rate = reth_result.total_successes as f64 / reth_result.total_requests as f64;
        println!("  Nitro success rate: {:.2}%", nitro_success_rate * 100.0);
        println!("  Reth success rate:  {:.2}%", reth_success_rate * 100.0);

        // 目标评估
        println!("\n🎯 Target Assessment:");
        if tps_improvement >= 10.0 {
            println!("  ✅ 10x performance target: ACHIEVED! ({:.1}x)", tps_improvement);
        } else {
            println!("  ❌ 10x performance target: Not met ({:.1}x)", tps_improvement);
        }
    }
}

impl serde::Serialize for BenchmarkResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("BenchmarkResult", 15)?;
        state.serialize_field("node_type", &self.node_type)?;
        state.serialize_field("endpoint", &self.endpoint)?;
        state.serialize_field("start_time", &self.start_time)?;
        state.serialize_field("end_time", &self.end_time)?;
        state.serialize_field("duration_seconds", &self.duration_seconds)?;
        state.serialize_field("total_requests", &self.total_requests)?;
        state.serialize_field("total_successes", &self.total_successes)?;
        state.serialize_field("total_failures", &self.total_failures)?;
        state.serialize_field("avg_tps", &self.avg_tps)?;
        state.serialize_field("avg_latency_ms", &self.avg_latency_ms)?;
        state.serialize_field("p95_latency_ms", &self.p95_latency_ms)?;
        state.serialize_field("p99_latency_ms", &self.p99_latency_ms)?;
        state.serialize_field("min_latency_ms", &self.min_latency_ms)?;
        state.serialize_field("max_latency_ms", &self.max_latency_ms)?;
        state.serialize_field("success_rate", &(self.total_successes as f64 / self.total_requests as f64))?;
        state.end()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting performance benchmark");
    info!("Duration: {} seconds", args.duration);
    info!("Target TPS: {}", args.target_tps);
    info!("Concurrent connections: {}", args.concurrent);

    let suite = BenchmarkSuite::new(args.nitro_endpoint, args.reth_endpoint);

    let (nitro_result, reth_result) = suite
        .run_benchmark(
            Duration::from_secs(args.duration),
            args.target_tps,
            args.concurrent,
            Duration::from_secs(args.warmup),
        )
        .await?;

    // 打印结果
    suite.print_results(&nitro_result, &reth_result);

    // 生成详细报告
    let comparison_report = suite.generate_comparison_report(&nitro_result, &reth_result);

    // 保存报告
    if let Some(output_path) = args.output {
        let json = serde_json::to_string_pretty(&comparison_report)?;
        std::fs::write(&output_path, json)?;
        info!("Detailed report saved to: {}", output_path.display());
    } else {
        println!("\n📄 Detailed Report:");
        println!("{}", serde_json::to_string_pretty(&comparison_report)?);
    }

    Ok(())
}

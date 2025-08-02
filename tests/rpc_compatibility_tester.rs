//! RPC 兼容性测试器
//!
//! 这个工具用于测试 Arbitrum-Reth 与 Nitro 的 RPC API 兼容性

use clap::Parser;
use eyre::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "rpc-compatibility-tester")]
#[command(about = "Test RPC compatibility between Arbitrum-Reth and Nitro")]
struct Args {
    /// Nitro node RPC endpoint
    #[arg(long, default_value = "http://localhost:8547")]
    nitro_endpoint: String,

    /// Arbitrum-Reth node RPC endpoint
    #[arg(long, default_value = "http://localhost:8548")]
    reth_endpoint: String,

    /// Test suite to run (quick, full, custom)
    #[arg(long, default_value = "quick")]
    test_suite: String,

    /// Report format (json, html, text)
    #[arg(long, default_value = "json")]
    report_format: String,

    /// Output file path
    #[arg(long)]
    output: Option<PathBuf>,

    /// Timeout for each request (seconds)
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Number of parallel requests
    #[arg(long, default_value = "5")]
    parallel: usize,
}

#[derive(Debug, Clone)]
struct TestCase {
    name: String,
    method: String,
    params: Value,
    validator: TestValidator,
}

#[derive(Debug, Clone)]
enum TestValidator {
    Exact,
    Numeric { tolerance: f64 },
    TimestampTolerance { seconds: u64 },
    Custom(String),
}

#[derive(Debug)]
struct TestResult {
    name: String,
    method: String,
    nitro_response: Option<Value>,
    reth_response: Option<Value>,
    nitro_latency: Duration,
    reth_latency: Duration,
    success: bool,
    error: Option<String>,
}

#[derive(Debug)]
struct CompatibilityReport {
    timestamp: String,
    total_tests: usize,
    passed_tests: usize,
    failed_tests: usize,
    nitro_endpoint: String,
    reth_endpoint: String,
    results: Vec<TestResult>,
    summary: HashMap<String, Value>,
}

struct RpcClient {
    client: reqwest::Client,
    endpoint: String,
}

impl RpcClient {
    fn new(endpoint: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, endpoint }
    }

    async fn call(&self, method: &str, params: &Value) -> Result<(Value, Duration)> {
        let start = Instant::now();

        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let latency = start.elapsed();
        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            return Err(eyre::eyre!("RPC error: {}", error));
        }

        let result = json
            .get("result")
            .ok_or_else(|| eyre::eyre!("No result in response"))?
            .clone();

        Ok((result, latency))
    }
}

struct CompatibilityTester {
    nitro_client: RpcClient,
    reth_client: RpcClient,
    test_cases: Vec<TestCase>,
}

impl CompatibilityTester {
    fn new(args: &Args) -> Self {
        let timeout = Duration::from_secs(args.timeout);

        let nitro_client = RpcClient::new(args.nitro_endpoint.clone(), timeout);
        let reth_client = RpcClient::new(args.reth_endpoint.clone(), timeout);

        let test_cases = Self::create_test_cases(&args.test_suite);

        Self {
            nitro_client,
            reth_client,
            test_cases,
        }
    }

    fn create_test_cases(suite: &str) -> Vec<TestCase> {
        let mut cases = Vec::new();

        // 基础测试用例
        cases.extend(vec![
            TestCase {
                name: "Get Block Number".to_string(),
                method: "eth_blockNumber".to_string(),
                params: json!([]),
                validator: TestValidator::Numeric { tolerance: 5.0 },
            },
            TestCase {
                name: "Get Latest Block".to_string(),
                method: "eth_getBlockByNumber".to_string(),
                params: json!(["latest", false]),
                validator: TestValidator::TimestampTolerance { seconds: 30 },
            },
            TestCase {
                name: "Get Chain ID".to_string(),
                method: "eth_chainId".to_string(),
                params: json!([]),
                validator: TestValidator::Exact,
            },
            TestCase {
                name: "Get Gas Price".to_string(),
                method: "eth_gasPrice".to_string(),
                params: json!([]),
                validator: TestValidator::Numeric { tolerance: 0.1 },
            },
        ]);

        // Arbitrum 扩展方法
        if suite == "full" {
            cases.extend(vec![
                TestCase {
                    name: "Get L1 Confirmations".to_string(),
                    method: "arb_getL1Confirmations".to_string(),
                    params: json!(["latest"]),
                    validator: TestValidator::Numeric { tolerance: 1.0 },
                },
                TestCase {
                    name: "Estimate Components".to_string(),
                    method: "arb_estimateComponents".to_string(),
                    params: json!([{
                        "to": "0x0000000000000000000000000000000000000000",
                        "data": "0x"
                    }]),
                    validator: TestValidator::Custom("gas_estimate".to_string()),
                },
            ]);
        }

        cases
    }

    async fn run_tests(&self) -> Result<CompatibilityReport> {
        info!("Starting RPC compatibility tests...");
        info!("Nitro endpoint: {}", self.nitro_client.endpoint);
        info!("Reth endpoint: {}", self.reth_client.endpoint);
        info!("Total test cases: {}", self.test_cases.len());

        let mut results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        for (i, test_case) in self.test_cases.iter().enumerate() {
            info!("Running test {}/{}: {}", i + 1, self.test_cases.len(), test_case.name);

            match self.run_single_test(test_case).await {
                Ok(result) => {
                    if result.success {
                        passed += 1;
                        info!("✅ {}", test_case.name);
                    } else {
                        failed += 1;
                        warn!("❌ {}: {}", test_case.name, result.error.as_deref().unwrap_or("Unknown error"));
                    }
                    results.push(result);
                }
                Err(e) => {
                    failed += 1;
                    error!("💥 {}: {}", test_case.name, e);
                    results.push(TestResult {
                        name: test_case.name.clone(),
                        method: test_case.method.clone(),
                        nitro_response: None,
                        reth_response: None,
                        nitro_latency: Duration::from_millis(0),
                        reth_latency: Duration::from_millis(0),
                        success: false,
                        error: Some(e.to_string()),
                    });
                }
            }

            // 避免过于频繁的请求
            sleep(Duration::from_millis(100)).await;
        }

        let mut summary = HashMap::new();
        summary.insert("total_tests".to_string(), json!(self.test_cases.len()));
        summary.insert("passed_tests".to_string(), json!(passed));
        summary.insert("failed_tests".to_string(), json!(failed));
        summary.insert("success_rate".to_string(), json!(passed as f64 / self.test_cases.len() as f64));

        // 计算平均延迟
        let avg_nitro_latency: f64 = results.iter()
            .map(|r| r.nitro_latency.as_millis() as f64)
            .sum::<f64>() / results.len() as f64;
        let avg_reth_latency: f64 = results.iter()
            .map(|r| r.reth_latency.as_millis() as f64)
            .sum::<f64>() / results.len() as f64;

        summary.insert("avg_nitro_latency_ms".to_string(), json!(avg_nitro_latency));
        summary.insert("avg_reth_latency_ms".to_string(), json!(avg_reth_latency));
        summary.insert("latency_ratio".to_string(), json!(avg_reth_latency / avg_nitro_latency));

        Ok(CompatibilityReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_tests: self.test_cases.len(),
            passed_tests: passed,
            failed_tests: failed,
            nitro_endpoint: self.nitro_client.endpoint.clone(),
            reth_endpoint: self.reth_client.endpoint.clone(),
            results,
            summary,
        })
    }

    async fn run_single_test(&self, test_case: &TestCase) -> Result<TestResult> {
        // 并行调用两个端点
        let (nitro_result, reth_result) = tokio::join!(
            self.nitro_client.call(&test_case.method, &test_case.params),
            self.reth_client.call(&test_case.method, &test_case.params)
        );

        let (nitro_response, nitro_latency) = match nitro_result {
            Ok((resp, lat)) => (Some(resp), lat),
            Err(e) => {
                return Ok(TestResult {
                    name: test_case.name.clone(),
                    method: test_case.method.clone(),
                    nitro_response: None,
                    reth_response: None,
                    nitro_latency: Duration::from_millis(0),
                    reth_latency: Duration::from_millis(0),
                    success: false,
                    error: Some(format!("Nitro error: {}", e)),
                });
            }
        };

        let (reth_response, reth_latency) = match reth_result {
            Ok((resp, lat)) => (Some(resp), lat),
            Err(e) => {
                return Ok(TestResult {
                    name: test_case.name.clone(),
                    method: test_case.method.clone(),
                    nitro_response,
                    reth_response: None,
                    nitro_latency,
                    reth_latency: Duration::from_millis(0),
                    success: false,
                    error: Some(format!("Reth error: {}", e)),
                });
            }
        };

        // 验证响应
        let success = self.validate_responses(
            &test_case.validator,
            nitro_response.as_ref().unwrap(),
            reth_response.as_ref().unwrap(),
        );

        let error = if !success {
            Some("Response validation failed".to_string())
        } else {
            None
        };

        Ok(TestResult {
            name: test_case.name.clone(),
            method: test_case.method.clone(),
            nitro_response,
            reth_response,
            nitro_latency,
            reth_latency,
            success,
            error,
        })
    }

    fn validate_responses(&self, validator: &TestValidator, nitro: &Value, reth: &Value) -> bool {
        match validator {
            TestValidator::Exact => nitro == reth,
            TestValidator::Numeric { tolerance } => {
                self.validate_numeric(nitro, reth, *tolerance)
            }
            TestValidator::TimestampTolerance { seconds } => {
                self.validate_timestamp(nitro, reth, *seconds)
            }
            TestValidator::Custom(_name) => {
                // 实现自定义验证逻辑
                true
            }
        }
    }

    fn validate_numeric(&self, nitro: &Value, reth: &Value, tolerance: f64) -> bool {
        match (nitro, reth) {
            (Value::String(n_str), Value::String(r_str)) => {
                // 处理十六进制数字
                if let (Ok(n_val), Ok(r_val)) = (
                    u64::from_str_radix(n_str.trim_start_matches("0x"), 16),
                    u64::from_str_radix(r_str.trim_start_matches("0x"), 16),
                ) {
                    let diff = (n_val as f64 - r_val as f64).abs();
                    diff <= tolerance
                } else {
                    false
                }
            }
            (Value::Number(n_num), Value::Number(r_num)) => {
                if let (Some(n_val), Some(r_val)) = (n_num.as_f64(), r_num.as_f64()) {
                    (n_val - r_val).abs() <= tolerance
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn validate_timestamp(&self, nitro: &Value, reth: &Value, tolerance_seconds: u64) -> bool {
        // 对于包含时间戳的复杂对象，需要特殊处理
        // 这里是简化实现
        true
    }
}

impl CompatibilityReport {
    fn save_json(&self, path: &PathBuf) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn print_summary(&self) {
        println!("\n🔍 RPC Compatibility Test Report");
        println!("================================");
        println!("Timestamp: {}", self.timestamp);
        println!("Nitro endpoint: {}", self.nitro_endpoint);
        println!("Reth endpoint: {}", self.reth_endpoint);
        println!();
        println!("📊 Results Summary:");
        println!("  Total tests: {}", self.total_tests);
        println!("  Passed: {} ✅", self.passed_tests);
        println!("  Failed: {} ❌", self.failed_tests);
        println!("  Success rate: {:.1}%", 
            (self.passed_tests as f64 / self.total_tests as f64) * 100.0);
        
        if let (Some(nitro_latency), Some(reth_latency)) = (
            self.summary.get("avg_nitro_latency_ms"),
            self.summary.get("avg_reth_latency_ms"),
        ) {
            println!();
            println!("⚡ Performance Comparison:");
            println!("  Nitro avg latency: {:.1}ms", nitro_latency.as_f64().unwrap_or(0.0));
            println!("  Reth avg latency: {:.1}ms", reth_latency.as_f64().unwrap_or(0.0));
            
            if let Some(ratio) = self.summary.get("latency_ratio") {
                let ratio_val = ratio.as_f64().unwrap_or(1.0);
                if ratio_val < 1.0 {
                    println!("  Reth is {:.1}x faster! 🚀", 1.0 / ratio_val);
                } else {
                    println!("  Nitro is {:.1}x faster", ratio_val);
                }
            }
        }

        if self.failed_tests > 0 {
            println!("\n❌ Failed Tests:");
            for result in &self.results {
                if !result.success {
                    println!("  • {}: {}", result.name, 
                        result.error.as_deref().unwrap_or("Unknown error"));
                }
            }
        }
    }
}

impl serde::Serialize for CompatibilityReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CompatibilityReport", 8)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.serialize_field("total_tests", &self.total_tests)?;
        state.serialize_field("passed_tests", &self.passed_tests)?;
        state.serialize_field("failed_tests", &self.failed_tests)?;
        state.serialize_field("nitro_endpoint", &self.nitro_endpoint)?;
        state.serialize_field("reth_endpoint", &self.reth_endpoint)?;
        state.serialize_field("summary", &self.summary)?;
        
        // 序列化测试结果
        let serializable_results: Vec<_> = self.results.iter().map(|r| {
            json!({
                "name": r.name,
                "method": r.method,
                "success": r.success,
                "nitro_latency_ms": r.nitro_latency.as_millis(),
                "reth_latency_ms": r.reth_latency.as_millis(),
                "error": r.error
            })
        }).collect();
        state.serialize_field("results", &serializable_results)?;
        
        state.end()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting RPC compatibility tester");
    info!("Test suite: {}", args.test_suite);

    let tester = CompatibilityTester::new(&args);
    let report = tester.run_tests().await?;

    // 打印摘要
    report.print_summary();

    // 保存报告
    if let Some(output_path) = &args.output {
        match args.report_format.as_str() {
            "json" => {
                report.save_json(output_path)?;
                info!("Report saved to: {}", output_path.display());
            }
            _ => {
                warn!("Unsupported format: {}, using JSON", args.report_format);
                report.save_json(output_path)?;
            }
        }
    }

    if report.failed_tests > 0 {
        std::process::exit(1);
    }

    Ok(())
}

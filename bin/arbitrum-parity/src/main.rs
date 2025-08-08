use clap::Parser;
use eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};

/// Compare JSON-RPC outputs across two Ethereum-compatible nodes for a set of methods.
#[derive(Debug, Parser)]
#[command(
    name = "arbitrum-parity",
    version,
    about = "Parity harness to diff JSON-RPC responses between two endpoints"
)]
struct Args {
    /// Left endpoint (e.g. Rust L2 node) HTTP URL
    #[arg(long)]
    left: String,

    /// Right endpoint (e.g. Nitro) HTTP URL
    #[arg(long)]
    right: String,

    /// Comma-separated list of methods to test (default: a small read set)
    #[arg(
        long,
        default_value = "eth_blockNumber,eth_chainId,net_version,eth_gasPrice"
    )]
    methods: String,

    /// Request body params to pass (JSON string or @file)
    #[arg(long, default_value = "[]")]
    params: String,

    /// Number of iterations (for flaky detection)
    #[arg(long, default_value_t = 1)]
    iters: u32,

    /// Optional matrix JSON file: [{"method": "eth_blockNumber", "params": []}, ...]
    #[arg(long)]
    matrix: Option<String>,

    /// Compare mode: full response vs only the result field
    #[arg(long, value_parser = ["full", "result"], default_value = "result")]
    compare: String,

    /// Comma-separated JSON pointer paths to remove from compared value (applies to all methods)
    /// Example: "/timestamp,/result/extraneous"
    #[arg(long, default_value = "")]
    ignore: String,

    /// Optional path to write a JSON report summarizing results
    #[arg(long)]
    report: Option<String>,

    /// Request timeout in seconds
    #[arg(long, default_value_t = 10)]
    timeout: u64,

    /// When comparing result arrays of logs, sort them by blockNumber, transactionIndex, logIndex
    #[arg(long, default_value_t = false)]
    sort_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatrixEntry {
    method: String,
    #[serde(default)]
    params: Value,
    /// Optional override: "result" (default) or "full"
    #[serde(default)]
    compare: Option<String>,
    /// Optional override: comma-separated JSON pointer paths
    #[serde(default)]
    ignore: Option<String>,
    /// Optional override: sort logs for this entry
    #[serde(default)]
    sort_logs: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let args = Args::parse();

    let params: Value = if let Some(rest) = args.params.strip_prefix('@') {
        let text = std::fs::read_to_string(rest)?;
        serde_json::from_str(&text)?
    } else {
        serde_json::from_str(&args.params)?
    };

    let entries: Vec<MatrixEntry> = if let Some(path) = &args.matrix {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str::<Vec<MatrixEntry>>(&text)?
            .into_iter()
            .map(|mut e| {
                if e.params.is_null() {
                    e.params = Value::Array(vec![]);
                }
                e
            })
            .collect()
    } else {
        let methods: Vec<String> = args
            .methods
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        methods
            .into_iter()
            .map(|m| MatrixEntry {
                method: m,
                params: params.clone(),
                compare: None,
                ignore: None,
                sort_logs: None,
            })
            .collect()
    };

    let compare_result_only = args.compare == "result";
    let ignore_paths_global: Vec<String> = args
        .ignore
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout))
        .build()?;
    let mut failures = 0usize;
    let mut report = Report {
        total: 0,
        failures: 0,
        cases: vec![],
    };

    for i in 0..args.iters {
        info!(iter = i, "running parity checks");
        for entry in &entries {
            let left = rpc_call(&client, &args.left, &entry.method, entry.params.clone()).await;
            let right = rpc_call(&client, &args.right, &entry.method, entry.params.clone()).await;
            match (left, right) {
                (Ok(l), Ok(r)) => {
                    let (mut l, mut r) = (l, r);
                    let entry_compare_result_only = match entry.compare.as_deref() {
                        Some("full") => false,
                        Some("result") => true,
                        Some(other) => return Err(eyre!("invalid compare in matrix: {}", other)),
                        None => compare_result_only,
                    };
                    if entry_compare_result_only {
                        l = l
                            .get("result")
                            .cloned()
                            .ok_or_else(|| eyre!("left missing result"))?;
                        r = r
                            .get("result")
                            .cloned()
                            .ok_or_else(|| eyre!("right missing result"))?;
                        let sort_logs_effective = entry.sort_logs.unwrap_or(args.sort_logs);
                        if sort_logs_effective {
                            sort_logs_array(&mut l);
                            sort_logs_array(&mut r);
                        }
                    }
                    l = normalize(&l);
                    r = normalize(&r);
                    let ignore_paths_effective: Vec<String> = entry
                        .ignore
                        .clone()
                        .unwrap_or_else(|| "".to_string())
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .chain(ignore_paths_global.iter().cloned())
                        .collect();
                    for p in &ignore_paths_effective {
                        remove_path(&mut l, p);
                        remove_path(&mut r, p);
                    }
                    if l != r {
                        error!(method = %entry.method, left = %l, right = %r, "Mismatch");
                        failures += 1;
                        report.cases.push(CaseResult::mismatch(entry, l, r));
                    } else {
                        info!(method = %entry.method, "OK");
                        report.cases.push(CaseResult::ok(entry));
                    }
                }
                (l, r) => {
                    error!(method = %entry.method, left = ?l.err(), right = ?r.err(), "Request error");
                    failures += 1;
                    report.cases.push(CaseResult::error(entry));
                }
            }
            report.total += 1;
        }
    }

    if failures > 0 {
        report.failures = failures;
        if let Some(path) = &args.report {
            write_report(path, &report)?;
        }
        eyre::bail!("{} mismatches detected", failures);
    }
    report.failures = 0;
    if let Some(path) = &args.report {
        write_report(path, &report)?;
    }
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_target(false)
        .compact()
        .try_init();
}

async fn rpc_call(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    let resp = client.post(url).json(&req).send().await?;
    let status = resp.status();
    if !status.is_success() {
        eyre::bail!("HTTP {}", status);
    }
    let v: Value = resp.json().await?;
    Ok(v)
}

// Normalize JSON for comparison:
// - Lowercase hex strings
// - Leave arrays in original order (method-specific normalization can be added later)
fn normalize(v: &Value) -> Value {
    match v {
        Value::String(s) => {
            if s.starts_with("0x") {
                Value::String(s.to_lowercase())
            } else {
                Value::String(s.clone())
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(normalize).collect()),
        Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, val) in m.iter() {
                out.insert(k.clone(), normalize(val));
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

// Remove a JSON pointer path from a value if present
fn remove_path(v: &mut Value, pointer: &str) {
    if pointer.is_empty() {
        return;
    }
    // For top-level only removal, handle common cases; for deeper paths use take+replace
    if let Some((parent_ptr, key)) = pointer.rsplit_once('/') {
        if let Some(parent) = v.pointer_mut(parent_ptr)
            && !key.is_empty()
        {
            match parent {
                Value::Object(map) => {
                    map.remove(key);
                }
                Value::Array(arr) => {
                    if let Ok(idx) = key.parse::<usize>()
                        && idx < arr.len()
                    {
                        arr.remove(idx);
                    }
                }
                _ => {}
            }
        }
    } else {
        // e.g., "/field" at root
        if let Some(key) = pointer.strip_prefix('/')
            && let Value::Object(map) = v
            && !key.is_empty()
        {
            map.remove(key);
        }
    }
}

// Sort an array of log objects deterministically by blockNumber, transactionIndex, logIndex
fn sort_logs_array(v: &mut Value) {
    if let Value::Array(arr) = v {
        arr.sort_by_key(key_for_log);
    }
}

fn key_for_log(v: &Value) -> (u128, u128, u128) {
    let n = |obj: &serde_json::Map<String, Value>, key: &str| -> u128 {
        obj.get(key)
            .and_then(|x| x.as_str())
            .and_then(|s| s.strip_prefix("0x").or(Some(s)))
            .and_then(|h| u128::from_str_radix(h, 16).ok())
            .unwrap_or(0)
    };
    match v {
        Value::Object(map) => (
            n(map, "blockNumber"),
            n(map, "transactionIndex"),
            n(map, "logIndex"),
        ),
        _ => (0, 0, 0),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    total: usize,
    failures: usize,
    cases: Vec<CaseResult>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
struct CaseResult {
    method: String,
    params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<Value>,
}

impl CaseResult {
    fn ok(entry: &MatrixEntry) -> Self {
        Self {
            method: entry.method.clone(),
            params: entry.params.clone(),
            left: None,
            right: None,
        }
    }
    fn mismatch(entry: &MatrixEntry, left: Value, right: Value) -> Self {
        Self {
            method: entry.method.clone(),
            params: entry.params.clone(),
            left: Some(left),
            right: Some(right),
        }
    }
    fn error(entry: &MatrixEntry) -> Self {
        Self {
            method: entry.method.clone(),
            params: entry.params.clone(),
            left: None,
            right: None,
        }
    }
}

fn write_report(path: &str, report: &Report) -> Result<()> {
    let contents = serde_json::to_string_pretty(report)?;
    std::fs::write(path, contents)?;
    Ok(())
}

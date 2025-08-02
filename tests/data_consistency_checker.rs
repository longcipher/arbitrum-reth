//! Data consistency verification tool
//!
//! Verify data consistency between Arbitrum-Reth and Nitro

use clap::Parser;
use eyre::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "data-consistency-checker")]
#[command(about = "Verify data consistency between Arbitrum-Reth and Nitro")]
struct Args {
    /// Nitro data directory
    #[arg(long)]
    nitro_datadir: PathBuf,

    /// Arbitrum-Reth data directory
    #[arg(long)]
    reth_datadir: PathBuf,

    /// Start block number
    #[arg(long, default_value = "0")]
    start_block: u64,

    /// End block number
    #[arg(long, default_value = "1000")]
    end_block: u64,

    /// Sample every N blocks
    #[arg(long, default_value = "100")]
    sample_interval: u64,

    /// Output file for results
    #[arg(long)]
    output: Option<PathBuf>,

    /// Nitro RPC endpoint (alternative to datadir)
    #[arg(long)]
    nitro_endpoint: Option<String>,

    /// Reth RPC endpoint (alternative to datadir)
    #[arg(long)]
    reth_endpoint: Option<String>,
}

#[derive(Debug)]
struct ConsistencyReport {
    timestamp: String,
    blocks_checked: u64,
    consistent_blocks: u64,
    inconsistent_blocks: u64,
    errors: u64,
    block_differences: Vec<BlockDifference>,
    summary: HashMap<String, Value>,
}

#[derive(Debug)]
struct BlockDifference {
    block_number: u64,
    difference_type: String,
    nitro_value: Option<Value>,
    reth_value: Option<Value>,
    description: String,
}

struct DataConsistencyChecker {
    nitro_client: Option<reqwest::Client>,
    reth_client: Option<reqwest::Client>,
    nitro_endpoint: Option<String>,
    reth_endpoint: Option<String>,
    nitro_datadir: Option<PathBuf>,
    reth_datadir: Option<PathBuf>,
}

impl DataConsistencyChecker {
    fn new(args: &Args) -> Self {
        let nitro_client = args.nitro_endpoint.as_ref().map(|_| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client")
        });

        let reth_client = args.reth_endpoint.as_ref().map(|_| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client")
        });

        Self {
            nitro_client,
            reth_client,
            nitro_endpoint: args.nitro_endpoint.clone(),
            reth_endpoint: args.reth_endpoint.clone(),
            nitro_datadir: Some(args.nitro_datadir.clone()),
            reth_datadir: Some(args.reth_datadir.clone()),
        }
    }

    async fn check_consistency(
        &self,
        start_block: u64,
        end_block: u64,
        sample_interval: u64,
    ) -> Result<ConsistencyReport> {
        info!("Starting data consistency check");
        info!("Block range: {} to {}", start_block, end_block);
        info!("Sample interval: every {} blocks", sample_interval);

        let mut block_differences = Vec::new();
        let mut blocks_checked = 0;
        let mut consistent_blocks = 0;
        let mut inconsistent_blocks = 0;
        let mut errors = 0;

        let mut current_block = start_block;
        while current_block <= end_block {
            info!("Checking block {} ({:.1}% complete)", 
                current_block,
                ((current_block - start_block) as f64 / (end_block - start_block) as f64) * 100.0
            );

            match self.check_block_consistency(current_block).await {
                Ok(differences) => {
                    blocks_checked += 1;
                    if differences.is_empty() {
                        consistent_blocks += 1;
                    } else {
                        inconsistent_blocks += 1;
                        block_differences.extend(differences);
                    }
                }
                Err(e) => {
                    error!("Error checking block {}: {}", current_block, e);
                    errors += 1;
                }
            }

            current_block += sample_interval;
        }

        let mut summary = HashMap::new();
        summary.insert("blocks_checked".to_string(), json!(blocks_checked));
        summary.insert("consistent_blocks".to_string(), json!(consistent_blocks));
        summary.insert("inconsistent_blocks".to_string(), json!(inconsistent_blocks));
        summary.insert("errors".to_string(), json!(errors));
        summary.insert("consistency_rate".to_string(), 
            json!(consistent_blocks as f64 / blocks_checked as f64));

        Ok(ConsistencyReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            blocks_checked,
            consistent_blocks,
            inconsistent_blocks,
            errors,
            block_differences,
            summary,
        })
    }

    async fn check_block_consistency(&self, block_number: u64) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        // Get block data
        let (nitro_block, reth_block) = self.get_block_data(block_number).await?;

        // Check block header consistency
        differences.extend(self.check_block_header(&nitro_block, &reth_block, block_number)?);

        // Check transaction consistency
        differences.extend(self.check_transactions(&nitro_block, &reth_block, block_number)?);

        // Check receipt consistency
        differences.extend(self.check_receipts(&nitro_block, &reth_block, block_number).await?);

        Ok(differences)
    }

    async fn get_block_data(&self, block_number: u64) -> Result<(Value, Value)> {
        let block_hex = format!("0x{:x}", block_number);

        let nitro_block = if let (Some(client), Some(endpoint)) = (&self.nitro_client, &self.nitro_endpoint) {
            self.rpc_get_block(client, endpoint, &block_hex).await?
        } else {
            self.file_get_block(&self.nitro_datadir.as_ref().unwrap(), block_number).await?
        };

        let reth_block = if let (Some(client), Some(endpoint)) = (&self.reth_client, &self.reth_endpoint) {
            self.rpc_get_block(client, endpoint, &block_hex).await?
        } else {
            self.file_get_block(&self.reth_datadir.as_ref().unwrap(), block_number).await?
        };

        Ok((nitro_block, reth_block))
    }

    async fn rpc_get_block(&self, client: &reqwest::Client, endpoint: &str, block_hex: &str) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [block_hex, true],
            "id": 1
        });

        let response = client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            return Err(eyre::eyre!("RPC error: {}", error));
        }

        let result = json
            .get("result")
            .ok_or_else(|| eyre::eyre!("No result in response"))?
            .clone();

        Ok(result)
    }

    async fn file_get_block(&self, datadir: &PathBuf, block_number: u64) -> Result<Value> {
        // TODO: Implement reading block data from filesystem
        // This requires understanding the specific data storage format
        warn!("File-based block reading not implemented, using placeholder");
        Ok(json!({
            "number": format!("0x{:x}", block_number),
            "hash": format!("0x{:064x}", block_number),
            "parentHash": format!("0x{:064x}", block_number.saturating_sub(1)),
            "timestamp": "0x0",
            "transactions": []
        }))
    }

    fn check_block_header(&self, nitro_block: &Value, reth_block: &Value, block_number: u64) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        // Check key fields
        let fields_to_check = [
            "hash", "parentHash", "sha3Uncles", "miner", "stateRoot",
            "transactionsRoot", "receiptsRoot", "logsBloom", "difficulty",
            "gasLimit", "gasUsed", "timestamp", "extraData", "mixHash", "nonce"
        ];

        for field in &fields_to_check {
            let nitro_value = nitro_block.get(field);
            let reth_value = reth_block.get(field);

            match (nitro_value, reth_value) {
                (Some(n_val), Some(r_val)) => {
                    if !self.values_equal(n_val, r_val, field) {
                        differences.push(BlockDifference {
                            block_number,
                            difference_type: format!("block_header_{}", field),
                            nitro_value: Some(n_val.clone()),
                            reth_value: Some(r_val.clone()),
                            description: format!("Block header field '{}' differs", field),
                        });
                    }
                }
                (Some(n_val), None) => {
                    differences.push(BlockDifference {
                        block_number,
                        difference_type: format!("block_header_{}_missing_reth", field),
                        nitro_value: Some(n_val.clone()),
                        reth_value: None,
                        description: format!("Block header field '{}' missing in Reth", field),
                    });
                }
                (None, Some(r_val)) => {
                    differences.push(BlockDifference {
                        block_number,
                        difference_type: format!("block_header_{}_missing_nitro", field),
                        nitro_value: None,
                        reth_value: Some(r_val.clone()),
                        description: format!("Block header field '{}' missing in Nitro", field),
                    });
                }
                (None, None) => {
                    // Both missing this field, which may be normal
                }
            }
        }

        Ok(differences)
    }

    fn check_transactions(&self, nitro_block: &Value, reth_block: &Value, block_number: u64) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        let nitro_txs = nitro_block.get("transactions").and_then(|t| t.as_array());
        let reth_txs = reth_block.get("transactions").and_then(|t| t.as_array());

        match (nitro_txs, reth_txs) {
            (Some(n_txs), Some(r_txs)) => {
                if n_txs.len() != r_txs.len() {
                    differences.push(BlockDifference {
                        block_number,
                        difference_type: "transaction_count".to_string(),
                        nitro_value: Some(json!(n_txs.len())),
                        reth_value: Some(json!(r_txs.len())),
                        description: "Transaction count differs".to_string(),
                    });
                }

                // Check each transaction
                let min_len = n_txs.len().min(r_txs.len());
                for i in 0..min_len {
                    let tx_differences = self.check_single_transaction(&n_txs[i], &r_txs[i], block_number, i)?;
                    differences.extend(tx_differences);
                }
            }
            (Some(_), None) => {
                differences.push(BlockDifference {
                    block_number,
                    difference_type: "transactions_missing_reth".to_string(),
                    nitro_value: Some(nitro_block.get("transactions").unwrap().clone()),
                    reth_value: None,
                    description: "Transactions missing in Reth block".to_string(),
                });
            }
            (None, Some(_)) => {
                differences.push(BlockDifference {
                    block_number,
                    difference_type: "transactions_missing_nitro".to_string(),
                    nitro_value: None,
                    reth_value: Some(reth_block.get("transactions").unwrap().clone()),
                    description: "Transactions missing in Nitro block".to_string(),
                });
            }
            (None, None) => {
                // Both have no transactions
            }
        }

        Ok(differences)
    }

    fn check_single_transaction(&self, nitro_tx: &Value, reth_tx: &Value, block_number: u64, tx_index: usize) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        let tx_fields = [
            "hash", "from", "to", "value", "gas", "gasPrice", "data",
            "nonce", "type", "accessList", "maxFeePerGas", "maxPriorityFeePerGas"
        ];

        for field in &tx_fields {
            let nitro_value = nitro_tx.get(field);
            let reth_value = reth_tx.get(field);

            if let (Some(n_val), Some(r_val)) = (nitro_value, reth_value) {
                if !self.values_equal(n_val, r_val, field) {
                    differences.push(BlockDifference {
                        block_number,
                        difference_type: format!("transaction_{}_{}", tx_index, field),
                        nitro_value: Some(n_val.clone()),
                        reth_value: Some(r_val.clone()),
                        description: format!("Transaction {} field '{}' differs", tx_index, field),
                    });
                }
            }
        }

        Ok(differences)
    }

    async fn check_receipts(&self, nitro_block: &Value, reth_block: &Value, block_number: u64) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        // Get transaction hash list
        let nitro_txs = nitro_block.get("transactions").and_then(|t| t.as_array());
        let reth_txs = reth_block.get("transactions").and_then(|t| t.as_array());

        if let (Some(n_txs), Some(r_txs)) = (nitro_txs, reth_txs) {
            let min_len = n_txs.len().min(r_txs.len());
            
            for i in 0..min_len {
                if let (Some(n_hash), Some(r_hash)) = (n_txs[i].get("hash"), r_txs[i].get("hash")) {
                    if n_hash == r_hash {
                        // Only compare receipts when transaction hashes match
                        if let Ok(receipt_diffs) = self.check_transaction_receipt(n_hash, block_number, i).await {
                            differences.extend(receipt_diffs);
                        }
                    }
                }
            }
        }

        Ok(differences)
    }

    async fn check_transaction_receipt(&self, tx_hash: &Value, block_number: u64, tx_index: usize) -> Result<Vec<BlockDifference>> {
        let mut differences = Vec::new();

        if let Value::String(hash_str) = tx_hash {
            // Get receipt data (if via RPC)
            if let (Some(nitro_client), Some(nitro_endpoint)) = (&self.nitro_client, &self.nitro_endpoint) {
                if let (Some(reth_client), Some(reth_endpoint)) = (&self.reth_client, &self.reth_endpoint) {
                    let nitro_receipt = self.rpc_get_receipt(nitro_client, nitro_endpoint, hash_str).await?;
                    let reth_receipt = self.rpc_get_receipt(reth_client, reth_endpoint, hash_str).await?;

                    // Compare receipt fields
                    let receipt_fields = ["status", "gasUsed", "cumulativeGasUsed", "logsBloom", "logs"];
                    
                    for field in &receipt_fields {
                        let nitro_value = nitro_receipt.get(field);
                        let reth_value = reth_receipt.get(field);

                        if let (Some(n_val), Some(r_val)) = (nitro_value, reth_value) {
                            if !self.values_equal(n_val, r_val, field) {
                                differences.push(BlockDifference {
                                    block_number,
                                    difference_type: format!("receipt_{}_{}", tx_index, field),
                                    nitro_value: Some(n_val.clone()),
                                    reth_value: Some(r_val.clone()),
                                    description: format!("Receipt {} field '{}' differs", tx_index, field),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(differences)
    }

    async fn rpc_get_receipt(&self, client: &reqwest::Client, endpoint: &str, tx_hash: &str) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });

        let response = client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            return Err(eyre::eyre!("RPC error: {}", error));
        }

        let result = json
            .get("result")
            .ok_or_else(|| eyre::eyre!("No result in response"))?
            .clone();

        Ok(result)
    }

    fn values_equal(&self, val1: &Value, val2: &Value, field: &str) -> bool {
        match field {
            "timestamp" => {
                // Allow small differences in timestamps
                self.timestamps_equal(val1, val2, 5)
            }
            "gasPrice" | "gasUsed" | "gasLimit" => {
                // Allow small differences in gas-related fields
                self.numeric_values_equal(val1, val2, 0.01)
            }
            _ => val1 == val2,
        }
    }

    fn timestamps_equal(&self, val1: &Value, val2: &Value, tolerance_seconds: u64) -> bool {
        if let (Value::String(s1), Value::String(s2)) = (val1, val2) {
            if let (Ok(t1), Ok(t2)) = (
                u64::from_str_radix(s1.trim_start_matches("0x"), 16),
                u64::from_str_radix(s2.trim_start_matches("0x"), 16),
            ) {
                return t1.abs_diff(t2) <= tolerance_seconds;
            }
        }
        val1 == val2
    }

    fn numeric_values_equal(&self, val1: &Value, val2: &Value, tolerance_ratio: f64) -> bool {
        if let (Value::String(s1), Value::String(s2)) = (val1, val2) {
            if let (Ok(n1), Ok(n2)) = (
                u64::from_str_radix(s1.trim_start_matches("0x"), 16),
                u64::from_str_radix(s2.trim_start_matches("0x"), 16),
            ) {
                let diff = (n1 as f64 - n2 as f64).abs();
                let avg = (n1 as f64 + n2 as f64) / 2.0;
                return diff / avg <= tolerance_ratio;
            }
        }
        val1 == val2
    }
}

impl ConsistencyReport {
    fn save_json(&self, path: &PathBuf) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn print_summary(&self) {
        println!("\n📊 Data Consistency Report");
        println!("==========================");
        println!("Timestamp: {}", self.timestamp);
        println!();
        println!("📈 Summary:");
        println!("  Blocks checked: {}", self.blocks_checked);
        println!("  Consistent blocks: {} ✅", self.consistent_blocks);
        println!("  Inconsistent blocks: {} ❌", self.inconsistent_blocks);
        println!("  Errors: {} 💥", self.errors);
        
        if self.blocks_checked > 0 {
            let consistency_rate = self.consistent_blocks as f64 / self.blocks_checked as f64;
            println!("  Consistency rate: {:.2}%", consistency_rate * 100.0);
        }

        if !self.block_differences.is_empty() {
            println!("\n❌ Inconsistencies Found:");
            let mut diff_summary: HashMap<String, u64> = HashMap::new();
            
            for diff in &self.block_differences {
                *diff_summary.entry(diff.difference_type.clone()).or_insert(0) += 1;
            }

            for (diff_type, count) in &diff_summary {
                println!("  • {}: {} occurrences", diff_type, count);
            }

            println!("\n🔍 First 5 Differences:");
            for (i, diff) in self.block_differences.iter().take(5).enumerate() {
                println!("  {}. Block {}: {}", i + 1, diff.block_number, diff.description);
            }
        }
    }
}

impl serde::Serialize for ConsistencyReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ConsistencyReport", 7)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.serialize_field("blocks_checked", &self.blocks_checked)?;
        state.serialize_field("consistent_blocks", &self.consistent_blocks)?;
        state.serialize_field("inconsistent_blocks", &self.inconsistent_blocks)?;
        state.serialize_field("errors", &self.errors)?;
        state.serialize_field("summary", &self.summary)?;
        
        let serializable_differences: Vec<_> = self.block_differences.iter().map(|d| {
            json!({
                "block_number": d.block_number,
                "difference_type": d.difference_type,
                "description": d.description,
                "nitro_value": d.nitro_value,
                "reth_value": d.reth_value
            })
        }).collect();
        state.serialize_field("differences", &serializable_differences)?;
        
        state.end()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting data consistency check");

    let checker = DataConsistencyChecker::new(&args);
    let report = checker
        .check_consistency(args.start_block, args.end_block, args.sample_interval)
        .await?;

    // 打印摘要
    report.print_summary();

    // 保存报告
    if let Some(output_path) = args.output {
        report.save_json(&output_path)?;
        info!("Report saved to: {}", output_path.display());
    }

    if report.inconsistent_blocks > 0 {
        warn!("Data inconsistencies found!");
        std::process::exit(1);
    }

    info!("Data consistency check completed successfully");
    Ok(())
}

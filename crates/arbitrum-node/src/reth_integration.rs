use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use alloy_primitives::{Address, B256, U256};
use arbitrum_config::ArbitrumRethConfig;
use arbitrum_storage::ArbitrumStorage;
use axum::{
    Json, Router, extract::State, response::IntoResponse, routing::get, serve as axum_serve,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{Mutex, oneshot},
    task::JoinHandle,
};
use tracing::{debug, info};

/// Minimal scaffold for integrating with Reth SDK. This will be replaced by real NodeBuilder wiring.
pub struct RethNodeHandle {
    server_shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    server_task: Mutex<Option<JoinHandle<()>>>,
    reth_task: Mutex<Option<JoinHandle<()>>>,
    prune_task: Mutex<Option<JoinHandle<()>>>,
}

impl RethNodeHandle {
    pub async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.server_shutdown_tx.lock().await.take() {
            let _ = tx.send(());
            debug!("Sent shutdown signal to Reth scaffold task");
        }
        // Abort experimental Reth task if present
        if let Some(task) = self.reth_task.lock().await.take() {
            task.abort();
        }
        // Abort pruning task if running
        if let Some(task) = self.prune_task.lock().await.take() {
            task.abort();
        }
        Ok(())
    }

    pub async fn wait(&self) -> Result<()> {
        if let Some(task) = self.server_task.lock().await.take() {
            let _ = task.await;
        }
        if let Some(task) = self.reth_task.lock().await.take() {
            let _ = task.await;
        }
        if let Some(task) = self.prune_task.lock().await.take() {
            let _ = task.await;
        }
        Ok(())
    }
}

/// Launches a background task to simulate a running Reth node until real integration is added.
pub async fn launch_reth_node(
    config: &ArbitrumRethConfig,
    storage: Option<Arc<ArbitrumStorage>>,
) -> Result<RethNodeHandle> {
    // Start HTTP server (health + JSON-RPC mock)
    let (tx, rx) = oneshot::channel::<()>();
    let http_addr: SocketAddr = ([127, 0, 0, 1], config.rpc.port).into();

    let state = ServerState {
        config: config.clone(),
        storage,
        filters: Arc::new(Mutex::new(FiltersManager {
            next_id: 0,
            installed: HashMap::new(),
            ttl_millis: if config.rpc.filter_ttl_ms == 0 {
                FiltersManager::DEFAULT_TTL_MILLIS
            } else {
                config.rpc.filter_ttl_ms
            },
            pruned_total: 0,
        })),
    };
    let app_state = state.clone();
    let app = Router::new()
        .route("/health", get(health))
        .route("/", axum::routing::post(json_rpc))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .expect("bind http");

    let server_task = tokio::spawn(async move {
        info!("HTTP on http://{http_addr}");
        tokio::select! {
            res = axum_serve(listener, app) => {
                info!("HTTP server exited: {:?}", res.err());
            }
            _ = rx => {
                info!("Shutdown received; stopping HTTP server");
            }
        }
    });

    // Spawn background pruning loop for expired filters if storage is available
    let prune_task = if let Some(storage_arc) = &state.storage {
        let filters_arc = Arc::clone(&state.filters);
        let storage_clone = Arc::clone(storage_arc);
        // Determine interval based on TTL; run at max every ttl/4, min 5s, max 5min
        let ttl_ms = {
            let mgr = filters_arc.lock().await;
            if mgr.ttl_millis == 0 {
                FiltersManager::DEFAULT_TTL_MILLIS
            } else {
                mgr.ttl_millis
            }
        };
        let mut interval = std::time::Duration::from_millis(ttl_ms / 4);
        if interval < std::time::Duration::from_secs(5) {
            interval = std::time::Duration::from_secs(5);
        }
        if interval > std::time::Duration::from_secs(300) {
            interval = std::time::Duration::from_secs(300);
        }
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let now = chrono::Utc::now().timestamp_millis() as u64;
                let mut mgr = filters_arc.lock().await;
                let ttl = if mgr.ttl_millis == 0 {
                    FiltersManager::DEFAULT_TTL_MILLIS
                } else {
                    mgr.ttl_millis
                };
                let ids: Vec<u64> = mgr.installed.keys().copied().collect();
                if ids.is_empty() {
                    continue;
                }
                match storage_clone.prune_expired_filters(&ids, now, ttl).await {
                    Ok(pruned) if !pruned.is_empty() => {
                        for idp in &pruned {
                            mgr.installed.remove(idp);
                        }
                        mgr.pruned_total = mgr.pruned_total.saturating_add(pruned.len() as u64);
                        debug!(count = pruned.len(), "Pruned expired filters in background");
                    }
                    Ok(_) => {}
                    Err(err) => {
                        tracing::warn!(?err, "Background filter prune failed");
                    }
                }
            }
        }))
    } else {
        None
    };

    // Optionally start experimental Reth NodeBuilder
    #[cfg(feature = "experimental-reth")]
    let reth_task = Some(spawn_reth_nodebuilder());
    #[cfg(not(feature = "experimental-reth"))]
    let reth_task = None;

    Ok(RethNodeHandle {
        server_shutdown_tx: Mutex::new(Some(tx)),
        server_task: Mutex::new(Some(server_task)),
        reth_task: Mutex::new(reth_task),
        prune_task: Mutex::new(prune_task),
    })
}

async fn health(State(state): State<ServerState>) -> impl IntoResponse {
    let (installed, pruned_total) = if let Ok(mgr) = state.filters.try_lock() {
        (mgr.installed.len(), mgr.pruned_total)
    } else {
        (0, 0)
    };
    Json(serde_json::json!({
        "status":"ok",
        "filters": {
            "installed": installed,
            "pruned_total": pruned_total,
            "ttl_ms": {
                "configured": {
                    "value": if let Ok(mgr) = state.filters.try_lock() { mgr.ttl_millis } else { 0 }
                }
            }
        }
    }))
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: serde_json::Value,
    result: serde_json::Value,
}

#[derive(Clone)]
struct ServerState {
    config: ArbitrumRethConfig,
    storage: Option<Arc<ArbitrumStorage>>,
    filters: Arc<Mutex<FiltersManager>>,
}

fn u64_to_hex(n: u64) -> String {
    format!("0x{n:x}")
}

fn u256_to_hex(n: &U256) -> String {
    format!("0x{n:x}")
}

fn b256_to_hex(h: &B256) -> String {
    format!("0x{}", hex::encode(h.as_slice()))
}

fn address_to_hex(a: &Address) -> String {
    format!("0x{}", hex::encode(a.as_slice()))
}

fn block_object(block: &arbitrum_storage::ArbitrumBlock) -> serde_json::Value {
    serde_json::json!({
        "number": u64_to_hex(block.number),
        "hash": b256_to_hex(&block.hash),
        "parentHash": b256_to_hex(&block.parent_hash),
        "timestamp": u64_to_hex(block.timestamp),
        "gasUsed": u64_to_hex(block.gas_used),
        "gasLimit": u64_to_hex(block.gas_limit),
    "transactions": block.transactions.iter().map(b256_to_hex).collect::<Vec<_>>(),
        // Minimal shape; add fields as needed for parity tests
    })
}

fn tx_object(tx: &arbitrum_storage::ArbitrumTransaction) -> serde_json::Value {
    serde_json::json!({
        "hash": b256_to_hex(&tx.hash),
        "from": address_to_hex(&tx.from),
        "to": tx.to.as_ref().map(address_to_hex),
        "value": u256_to_hex(&tx.value),
        "nonce": u64_to_hex(tx.nonce),
        "gas": u64_to_hex(tx.gas),
        "gasPrice": u256_to_hex(&tx.gas_price),
        // Minimal set
    })
}

#[allow(clippy::collapsible_if)]
async fn json_rpc(
    State(state): State<ServerState>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let id = req.id.unwrap_or(serde_json::json!(1));
    // Minimal methods to satisfy CI and local smoke tests
    let result = match req.method.as_str() {
        "web3_clientVersion" => serde_json::json!("arbitrum-reth/mock-scaffold"),
        "net_version" => serde_json::json!(state.config.l2.chain_id.to_string()),
        "eth_chainId" => serde_json::json!(u64_to_hex(state.config.l2.chain_id)),
        "eth_blockNumber" => {
            if let Some(storage) = &state.storage {
                match storage.get_current_block_number().await {
                    Ok(n) => serde_json::json!(u64_to_hex(n)),
                    Err(_) => serde_json::json!("0x0"),
                }
            } else {
                serde_json::json!("0x0")
            }
        }
        "eth_gasPrice" => serde_json::json!("0x174876e800"),
        "eth_getBlockByNumber" => {
            // params: ["0xN"|"latest", includeTxs]
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                let latest = storage.get_current_block_number().await.unwrap_or(0);
                let number_opt: Option<u64> = params
                    .first()
                    .and_then(|num_val| num_val.as_str())
                    .and_then(|s| {
                        if s == "latest" {
                            Some(latest)
                        } else if let Some(stripped) = s.strip_prefix("0x") {
                            u64::from_str_radix(stripped, 16).ok()
                        } else {
                            s.parse::<u64>().ok()
                        }
                    });
                let include_txs = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
                if let Some(n) = number_opt
                    && let Ok(Some(block)) = storage.get_block_by_number(n).await
                {
                    if include_txs {
                        let mut obj = block_object(&block);
                        if let Some(arr) =
                            obj.get_mut("transactions").and_then(|v| v.as_array_mut())
                        {
                            arr.clear();
                            let txs = futures::future::join_all(
                                block
                                    .transactions
                                    .iter()
                                    .map(|h| storage.get_transaction(h)),
                            )
                            .await;
                            let expanded: Vec<serde_json::Value> = txs
                                .into_iter()
                                .filter_map(|res| res.ok().flatten())
                                .map(|tx| tx_object(&tx))
                                .collect();
                            *arr = expanded;
                        }
                        out = obj;
                    } else {
                        out = block_object(&block);
                    }
                }
            }
            out
        }
        "eth_getBlockByHash" => {
            // params: ["0x<hash>", includeTxs]
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(h) = params
                    .first()
                    .and_then(|hv| hv.as_str())
                    .and_then(parse_b256_hex)
                {
                    let include_txs = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
                    if let Ok(Some(block)) = storage.get_block(&h).await {
                        if include_txs {
                            let mut obj = block_object(&block);
                            if let Some(arr) =
                                obj.get_mut("transactions").and_then(|v| v.as_array_mut())
                            {
                                arr.clear();
                                let txs = futures::future::join_all(
                                    block
                                        .transactions
                                        .iter()
                                        .map(|th| storage.get_transaction(th)),
                                )
                                .await;
                                let expanded: Vec<serde_json::Value> = txs
                                    .into_iter()
                                    .filter_map(|res| res.ok().flatten())
                                    .map(|tx| tx_object(&tx))
                                    .collect();
                                *arr = expanded;
                            }
                            out = obj;
                        } else {
                            out = block_object(&block);
                        }
                    }
                }
            }
            out
        }
        "eth_getBalance" => {
            // params: ["0x<address>", "latest"]
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(addr) = params
                    .first()
                    .and_then(|v| v.as_str())
                    .and_then(parse_address_hex)
                {
                    match storage.get_account(&addr).await {
                        Ok(Some(acct)) => {
                            out = serde_json::json!(u256_to_hex(&acct.balance));
                        }
                        _ => {
                            out = serde_json::json!("0x0");
                        }
                    }
                }
            }
            out
        }
        "eth_getTransactionCount" => {
            // params: ["0x<address>", "latest"]
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(addr) = params
                    .first()
                    .and_then(|v| v.as_str())
                    .and_then(parse_address_hex)
                {
                    match storage.get_account(&addr).await {
                        Ok(Some(acct)) => {
                            out = serde_json::json!(u64_to_hex(acct.nonce));
                        }
                        _ => {
                            out = serde_json::json!("0x0");
                        }
                    }
                }
            }
            out
        }
        "eth_getBlockTransactionCountByNumber" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                let latest = storage.get_current_block_number().await.unwrap_or(0);
                let number_opt: Option<u64> = params
                    .first()
                    .and_then(|num_val| num_val.as_str())
                    .and_then(|s| {
                        if s == "latest" {
                            Some(latest)
                        } else if let Some(stripped) = s.strip_prefix("0x") {
                            u64::from_str_radix(stripped, 16).ok()
                        } else {
                            s.parse::<u64>().ok()
                        }
                    });
                if let Some(n) = number_opt
                    && let Ok(Some(block)) = storage.get_block_by_number(n).await
                {
                    out = serde_json::json!(u64_to_hex(block.transactions.len() as u64));
                }
            }
            out
        }
        "eth_getBlockTransactionCountByHash" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(h) = params
                    .first()
                    .and_then(|hv| hv.as_str())
                    .and_then(parse_b256_hex)
                {
                    if let Ok(Some(block)) = storage.get_block(&h).await {
                        out = serde_json::json!(u64_to_hex(block.transactions.len() as u64));
                    }
                }
            }
            out
        }
        "eth_getTransactionByHash" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(h) = params
                    .first()
                    .and_then(|hv| hv.as_str())
                    .and_then(parse_b256_hex)
                {
                    if let Ok(Some(tx)) = storage.get_transaction(&h).await {
                        out = tx_object(&tx);
                    }
                }
            }
            out
        }
        "eth_getTransactionReceipt" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(h) = params
                    .first()
                    .and_then(|hv| hv.as_str())
                    .and_then(parse_b256_hex)
                {
                    if let Ok(Some(rcpt)) = storage.get_receipt(&h).await {
                        out = serde_json::json!({
                            "transactionHash": b256_to_hex(&rcpt.transaction_hash),
                            "transactionIndex": u64_to_hex(rcpt.transaction_index),
                            "blockHash": b256_to_hex(&rcpt.block_hash),
                            "blockNumber": u64_to_hex(rcpt.block_number),
                            "cumulativeGasUsed": u64_to_hex(rcpt.cumulative_gas_used),
                            "gasUsed": u64_to_hex(rcpt.gas_used),
                            "contractAddress": rcpt.contract_address.as_ref().map(address_to_hex),
                            "logs": rcpt.logs.iter().map(|l| serde_json::json!({
                                "address": address_to_hex(&l.address),
                                "topics": l.topics.iter().map(b256_to_hex).collect::<Vec<_>>(),
                                "data": format!("0x{}", hex::encode(&l.data)),
                                "blockHash": l.block_hash.as_ref().map(b256_to_hex),
                                "blockNumber": l.block_number.map(u64_to_hex),
                                "transactionHash": l.transaction_hash.as_ref().map(b256_to_hex),
                                "transactionIndex": l.transaction_index.map(u64_to_hex),
                                "logIndex": l.log_index.map(u64_to_hex),
                                "removed": l.removed,
                            })).collect::<Vec<_>>(),
                            "status": u64_to_hex(rcpt.status),
                            "effectiveGasPrice": u256_to_hex(&rcpt.effective_gas_price),
                        });
                    }
                }
            }
            out
        }
        "eth_getTransactionByBlockNumberAndIndex" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                let latest = storage.get_current_block_number().await.unwrap_or(0);
                let number_opt: Option<u64> = params
                    .first()
                    .and_then(|num_val| num_val.as_str())
                    .and_then(|s| {
                        if s == "latest" {
                            Some(latest)
                        } else if let Some(stripped) = s.strip_prefix("0x") {
                            u64::from_str_radix(stripped, 16).ok()
                        } else {
                            s.parse::<u64>().ok()
                        }
                    });
                let idx_opt: Option<usize> = params
                    .get(1)
                    .and_then(|idx_val| idx_val.as_str())
                    .and_then(|s| {
                        if let Some(stripped) = s.strip_prefix("0x") {
                            usize::from_str_radix(stripped, 16).ok()
                        } else {
                            s.parse::<usize>().ok()
                        }
                    });
                if let (Some(n), Some(i)) = (number_opt, idx_opt)
                    && let Ok(Some(block)) = storage.get_block_by_number(n).await
                    && let Some(h) = block.transactions.get(i)
                    && let Ok(Some(tx)) = storage.get_transaction(h).await
                {
                    out = tx_object(&tx);
                }
            }
            out
        }
        "eth_getTransactionByBlockHashAndIndex" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                let idx_opt: Option<usize> = params
                    .get(1)
                    .and_then(|idx_val| idx_val.as_str())
                    .and_then(|s| {
                        if let Some(stripped) = s.strip_prefix("0x") {
                            usize::from_str_radix(stripped, 16).ok()
                        } else {
                            s.parse::<usize>().ok()
                        }
                    });
                if let (Some(h), Some(i)) = (
                    params
                        .first()
                        .and_then(|hv| hv.as_str())
                        .and_then(parse_b256_hex),
                    idx_opt,
                ) && let Ok(Some(block)) = storage.get_block(&h).await
                    && let Some(txh) = block.transactions.get(i)
                    && let Ok(Some(tx)) = storage.get_transaction(txh).await
                {
                    out = tx_object(&tx);
                }
            }
            out
        }
        "eth_getLogs" => {
            let mut out = serde_json::Value::Array(vec![]);
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(f) = params.first().and_then(|v| v.as_object()) {
                    let (from_block, to_block, addrs, topics) =
                        parse_filter_fields(f, storage).await;
                    let logs = collect_logs_in_range(
                        storage,
                        from_block,
                        to_block,
                        addrs.as_ref(),
                        topics.as_ref(),
                    )
                    .await;
                    out = serde_json::Value::Array(logs);
                }
            }
            out
        }
        "eth_newFilter" => {
            let mut out = serde_json::Value::Null;
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(f) = params.first().and_then(|v| v.as_object()) {
                    let (from_block, to_block, addrs, topics) =
                        parse_filter_fields(f, storage).await;
                    let mut mgr = state.filters.lock().await;
                    let id = mgr.install_filter(FilterDef {
                        from_block: Some(from_block),
                        to_block: Some(to_block),
                        addresses: addrs,
                        topics,
                    });
                    out = serde_json::json!(u64_to_hex(id));
                }
            }
            out
        }
        "eth_getFilterChanges" => {
            let mut out = serde_json::Value::Array(vec![]);
            if let (Some(params), Some(storage)) = (
                req.params.as_ref().and_then(|v| v.as_array()),
                &state.storage,
            ) {
                if let Some(id) = params
                    .first()
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.strip_prefix("0x"))
                    .and_then(|hex| u64::from_str_radix(hex, 16).ok())
                {
                    let mut mgr = state.filters.lock().await;
                    if let Some((from, to, def)) = mgr.next_poll_range(id, storage).await {
                        let logs = collect_logs_in_range(
                            storage,
                            from,
                            to,
                            def.addresses.as_ref(),
                            def.topics.as_ref(),
                        )
                        .await;
                        out = serde_json::Value::Array(logs);
                    }
                }
            }
            out
        }
        "eth_uninstallFilter" => {
            let mut out = serde_json::Value::Bool(false);
            if let Some(params) = req.params.as_ref().and_then(|v| v.as_array()) {
                if let Some(id) = params
                    .first()
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.strip_prefix("0x"))
                    .and_then(|hex| u64::from_str_radix(hex, 16).ok())
                {
                    let mut mgr = state.filters.lock().await;
                    out = serde_json::Value::Bool(mgr.uninstall_filter(id));
                }
            }
            out
        }
        _ => serde_json::Value::Null,
    };

    Json(JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result,
    })
}

async fn parse_filter_fields(
    f: &serde_json::Map<String, serde_json::Value>,
    storage: &Arc<ArbitrumStorage>,
) -> (
    u64,
    u64,
    Option<Vec<Address>>,
    Option<Vec<Option<Vec<B256>>>>,
) {
    let latest = storage.get_current_block_number().await.unwrap_or(0);
    let from_block = match f.get("fromBlock").and_then(|v| v.as_str()) {
        Some("latest") => latest,
        Some(s) if s.starts_with("0x") => u64::from_str_radix(&s[2..], 16).unwrap_or(0),
        Some(s) => s.parse::<u64>().unwrap_or(0),
        None => 0,
    };
    let to_block = match f.get("toBlock").and_then(|v| v.as_str()) {
        Some("latest") | None => latest,
        Some(s) if s.starts_with("0x") => u64::from_str_radix(&s[2..], 16).unwrap_or(latest),
        Some(s) => s.parse::<u64>().unwrap_or(latest),
    };

    // address can be string or array
    let addrs: Option<Vec<Address>> = if let Some(a) = f.get("address") {
        if let Some(s) = a.as_str() {
            parse_address_hex(s).map(|addr| vec![addr])
        } else if let Some(arr) = a.as_array() {
            let mut out = Vec::new();
            for v in arr {
                if let Some(addr) = v.as_str().and_then(parse_address_hex) {
                    out.push(addr);
                }
            }
            Some(out)
        } else {
            None
        }
    } else {
        None
    };

    // topics: support [pos0, pos1, ...] where each pos can be null, a topic string, or an array of topic strings => OR at that position
    let topics: Option<Vec<Option<Vec<B256>>>> =
        if let Some(t) = f.get("topics").and_then(|v| v.as_array()) {
            let mut out: Vec<Option<Vec<B256>>> = Vec::new();
            for v in t {
                if v.is_null() {
                    out.push(None);
                } else if let Some(s) = v.as_str() {
                    out.push(parse_b256_hex(s).map(|x| vec![x]));
                } else if let Some(arr) = v.as_array() {
                    let mut or_vec: Vec<B256> = Vec::new();
                    for item in arr {
                        if let Some(x) = item.as_str().and_then(parse_b256_hex) {
                            or_vec.push(x);
                        }
                    }
                    out.push(Some(or_vec));
                } else {
                    out.push(None);
                }
            }
            Some(out)
        } else {
            None
        };

    (from_block, to_block, addrs, topics)
}

fn parse_address_hex(s: &str) -> Option<Address> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(hex).ok()?;
    if bytes.len() != 20 {
        return None;
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Some(Address::from(arr))
}

fn parse_b256_hex(s: &str) -> Option<B256> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(B256::from(arr))
}

async fn collect_logs_in_range(
    storage: &Arc<ArbitrumStorage>,
    from_block: u64,
    to_block: u64,
    addrs: Option<&Vec<Address>>,
    topics: Option<&Vec<Option<Vec<B256>>>>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if to_block < from_block {
        return out;
    }
    for n in from_block..=to_block {
        if let Ok(Some(block)) = storage.get_block_by_number(n).await {
            // Try indexed logs first
            if let Ok(indexed) = storage.get_indexed_logs_in_range(n, n).await
                && let Some((_, logs)) = indexed.into_iter().next()
            {
                for (log_idx, log) in logs.iter().enumerate() {
                    if !log_matches(log, addrs, topics) {
                        continue;
                    }
                    out.push(serde_json::json!({
                        "address": address_to_hex(&log.address),
                        "topics": log.topics.iter().map(b256_to_hex).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(&log.data)),
                        "blockHash": b256_to_hex(&block.hash),
                        "blockNumber": u64_to_hex(block.number),
                        "transactionHash": log.transaction_hash.as_ref().map(b256_to_hex),
                        "transactionIndex": log.transaction_index.map(u64_to_hex),
                        "logIndex": u64_to_hex(log_idx as u64),
                        "removed": false,
                    }));
                }
                continue;
            }
            // Fetch all receipts for this block concurrently
            let storage_clone = Arc::clone(storage);
            let futs = block.transactions.iter().enumerate().map(|(tx_idx, txh)| {
                let storage2 = Arc::clone(&storage_clone);
                async move {
                    match storage2.get_receipt(txh).await {
                        Ok(Some(rcpt)) => Some((tx_idx, rcpt)),
                        _ => None,
                    }
                }
            });
            let receipts = futures::future::join_all(futs).await;
            for maybe in receipts.into_iter().flatten() {
                let (tx_idx, rcpt) = maybe;
                for (log_idx, log) in rcpt.logs.iter().enumerate() {
                    if !log_matches(log, addrs, topics) {
                        continue;
                    }
                    out.push(serde_json::json!({
                        "address": address_to_hex(&log.address),
                        "topics": log.topics.iter().map(b256_to_hex).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(&log.data)),
                        "blockHash": b256_to_hex(&rcpt.block_hash),
                        "blockNumber": u64_to_hex(rcpt.block_number),
                        "transactionHash": b256_to_hex(&rcpt.transaction_hash),
                        "transactionIndex": u64_to_hex(tx_idx as u64),
                        "logIndex": u64_to_hex(log_idx as u64),
                        "removed": false,
                    }));
                }
            }
        }
    }
    out
}

fn log_matches(
    log: &arbitrum_storage::Log,
    addrs: Option<&Vec<Address>>,
    topics: Option<&Vec<Option<Vec<B256>>>>,
) -> bool {
    // Address filter (OR over provided addresses)
    if let Some(aset) = addrs
        && !aset.iter().any(|a| a == &log.address)
    {
        return false;
    }

    // Topics filter: AND across positions, OR within a position
    if let Some(ts) = topics {
        for (i, opt_or_list) in ts.iter().enumerate() {
            match opt_or_list {
                None => {
                    // Wildcard at this position
                }
                Some(or_list) => {
                    // Empty OR-list matches nothing
                    if or_list.is_empty() {
                        return false;
                    }
                    // If the log doesn't have a topic at this index, it's not a match
                    let Some(topic) = log.topics.get(i) else {
                        return false;
                    };
                    if !or_list.iter().any(|t| t == topic) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

#[derive(Clone)]
struct FilterDef {
    from_block: Option<u64>,
    to_block: Option<u64>,
    addresses: Option<Vec<Address>>,
    topics: Option<Vec<Option<Vec<B256>>>>,
}

struct FilterInstance {
    def: FilterDef,
    last_block: u64,
}

#[derive(Default)]
struct FiltersManager {
    next_id: u64,
    installed: HashMap<u64, FilterInstance>,
    ttl_millis: u64,
    pruned_total: u64,
}

impl FiltersManager {
    const MAX_BLOCKS_PER_POLL: u64 = 1024;
    const DEFAULT_TTL_MILLIS: u64 = 5 * 60 * 1000; // 5 minutes
    fn install_filter(&mut self, def: FilterDef) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        self.installed
            .insert(id, FilterInstance { def, last_block: 0 });
        id
    }

    async fn next_poll_range(
        &mut self,
        id: u64,
        storage: &Arc<ArbitrumStorage>,
    ) -> Option<(u64, u64, FilterDef)> {
        let latest = storage.get_current_block_number().await.ok()?;
        // Prune expired before serving
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let ttl = if self.ttl_millis == 0 {
            Self::DEFAULT_TTL_MILLIS
        } else {
            self.ttl_millis
        };
        let ids: Vec<u64> = self.installed.keys().copied().collect();
        if let Ok(pruned) = storage.prune_expired_filters(&ids, now, ttl).await {
            for idp in &pruned {
                self.installed.remove(idp);
            }
            if !pruned.is_empty() {
                self.pruned_total = self.pruned_total.saturating_add(pruned.len() as u64);
            }
        }
        if let Some(inst) = self.installed.get_mut(&id) {
            // Merge persisted cursor
            if let Ok(persisted) = storage.get_filter_cursor(id).await
                && persisted > inst.last_block
            {
                inst.last_block = persisted;
            }
            let start_base = inst.last_block.saturating_add(1);
            let start = inst.def.from_block.unwrap_or(0).max(start_base);
            let end_cap = inst.def.to_block.unwrap_or(latest).min(latest);
            if start > end_cap {
                return Some((start, end_cap, inst.def.clone()));
            }
            let end = (start.saturating_add(Self::MAX_BLOCKS_PER_POLL - 1)).min(end_cap);
            // Advance cursor only to processed end to allow chunked polling
            inst.last_block = end;
            let _ = storage.set_filter_cursor(id, inst.last_block).await;
            let _ = storage.touch_filter_last_seen(id, now).await;
            return Some((start, end, inst.def.clone()));
        }
        None
    }

    fn uninstall_filter(&mut self, id: u64) -> bool {
        self.installed.remove(&id).is_some()
    }
}

#[cfg(feature = "experimental-reth")]
fn spawn_reth_nodebuilder() -> JoinHandle<()> {
    use reth_ethereum::EthereumNode;
    use reth_node_builder::{NodeBuilder, NodeConfig as RethNodeConfig}; // type alias for Ethereum primitives

    // Prepare a minimal Reth config; this is a placeholder and will be expanded.
    let reth_config = RethNodeConfig::default();

    tokio::spawn(async move {
        info!("Starting experimental Reth NodeBuilder...");
        // Note: This uses defaults and does not yet wire our custom components.
        let res = NodeBuilder::new(reth_config)
            .with_types::<EthereumNode>()
            .launch()
            .await;
        if let Err(err) = res {
            tracing::error!(?err, "Reth node exited with error");
        }
    })
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{U256, address};
    use tempfile::TempDir;

    use super::*;

    async fn make_storage() -> (Arc<ArbitrumStorage>, TempDir, ArbitrumRethConfig) {
        let temp = TempDir::new().unwrap();
        let mut cfg = ArbitrumRethConfig::default();
        cfg.node.datadir = temp.path().to_path_buf();
        let storage = Arc::new(ArbitrumStorage::new(&cfg).await.unwrap());
        storage.start().await.unwrap();
        (storage, temp, cfg)
    }

    #[tokio::test]
    async fn test_filters_manager_ttl_prune_and_cursor() {
        let (storage, _tmp, _cfg) = make_storage().await;

        // Seed blocks 1..=10 so get_current_block_number returns 10
        for n in 1..=10u64 {
            let blk = arbitrum_storage::ArbitrumBlock {
                number: n,
                hash: B256::from([n as u8; 32]),
                parent_hash: if n == 1 {
                    B256::ZERO
                } else {
                    B256::from([(n - 1) as u8; 32])
                },
                timestamp: 1_700_000_000 + n,
                gas_used: 0,
                gas_limit: 30_000_000,
                transactions: vec![],
                l1_block_number: 0,
            };
            storage.store_block(&blk).await.unwrap();
        }

        let mut mgr = FiltersManager {
            next_id: 0,
            installed: HashMap::new(),
            ttl_millis: 200, // short ttl for test
            pruned_total: 0,
        };

        let id = mgr.install_filter(FilterDef {
            from_block: Some(0),
            to_block: Some(10),
            addresses: None,
            topics: None,
        });

        // First poll should advance cursor up to a chunk end
        let range = mgr.next_poll_range(id, &storage).await;
        assert!(range.is_some());
        let (start, end, _def) = range.unwrap();
        assert_eq!(start, 1);
        // cursor persisted
        let persisted = storage.get_filter_cursor(id).await.unwrap();
        assert_eq!(persisted, end);
        // last seen touched
        let last_seen = storage.get_filter_last_seen(id).await.unwrap();
        assert!(last_seen > 0);

        // Force expiration by backdating last_seen beyond ttl
        let now = chrono::Utc::now().timestamp_millis() as u64;
        storage
            .touch_filter_last_seen(id, now.saturating_sub(mgr.ttl_millis + 50))
            .await
            .unwrap();

        // Next poll should prune and return None
        let r2 = mgr.next_poll_range(id, &storage).await;
        assert!(r2.is_none());
        assert_eq!(mgr.installed.contains_key(&id), false);
        assert!(mgr.pruned_total >= 1);
    }

    #[tokio::test]
    async fn test_collect_logs_index_basic() {
        let (storage, _tmp, _cfg) = make_storage().await;

        // Create block 1 with two txs and one log each
        let txh1 = B256::from([2u8; 32]);
        let txh2 = B256::from([3u8; 32]);
        let blk = arbitrum_storage::ArbitrumBlock {
            number: 1,
            hash: B256::from([1u8; 32]),
            parent_hash: B256::ZERO,
            timestamp: 1,
            gas_used: 0,
            gas_limit: 30_000_000,
            transactions: vec![txh1, txh2],
            l1_block_number: 0,
        };
        storage.store_block(&blk).await.unwrap();

        // store txs
        let tx1 = arbitrum_storage::ArbitrumTransaction {
            hash: blk.transactions[0],
            from: address!("0x1111111111111111111111111111111111111111"),
            to: None,
            value: U256::from(1u64),
            gas: 21_000,
            gas_price: U256::from(1),
            nonce: 0,
            data: vec![],
            l1_sequence_number: None,
        };
        let tx2 = arbitrum_storage::ArbitrumTransaction {
            hash: blk.transactions[1],
            ..tx1.clone()
        };
        storage.store_transaction(&tx1).await.unwrap();
        storage.store_transaction(&tx2).await.unwrap();

        // receipts with logs
        let addr = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let topic = B256::from([9u8; 32]);
        let log1 = arbitrum_storage::Log {
            address: addr,
            topics: vec![topic],
            data: vec![1, 2, 3],
            block_hash: Some(blk.hash),
            block_number: Some(1),
            transaction_hash: Some(blk.transactions[0]),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        };
        let rcpt1 = arbitrum_storage::ArbitrumReceipt {
            transaction_hash: blk.transactions[0],
            transaction_index: 0,
            block_hash: blk.hash,
            block_number: 1,
            cumulative_gas_used: 0,
            gas_used: 0,
            contract_address: None,
            logs: vec![log1.clone()],
            status: 1,
            effective_gas_price: U256::from(1),
        };
        storage.store_receipt(&rcpt1).await.unwrap();

        let log2 = arbitrum_storage::Log {
            transaction_hash: Some(blk.transactions[1]),
            transaction_index: Some(1),
            ..log1.clone()
        };
        let rcpt2 = arbitrum_storage::ArbitrumReceipt {
            transaction_hash: blk.transactions[1],
            transaction_index: 1,
            logs: vec![log2.clone()],
            ..rcpt1
        };
        storage.store_receipt(&rcpt2).await.unwrap();

        // Collect logs using address filter; should return two logs
        let logs = collect_logs_in_range(&storage, 1, 1, Some(&vec![addr]), None).await;
        assert_eq!(logs.len(), 2);
        for l in logs {
            // basic fields present
            assert!(l.get("address").is_some());
            assert!(l.get("blockHash").is_some());
            assert!(l.get("blockNumber").is_some());
            assert!(l.get("transactionHash").is_some());
        }
    }

    #[tokio::test]
    async fn test_rpc_prune_and_health_metrics() {
        let (storage, _tmp, mut cfg) = make_storage().await;

        // Seed a few blocks to ensure latest resolves
        for n in 1..=3u64 {
            let blk = arbitrum_storage::ArbitrumBlock {
                number: n,
                hash: B256::from([n as u8; 32]),
                parent_hash: if n == 1 {
                    B256::ZERO
                } else {
                    B256::from([(n - 1) as u8; 32])
                },
                timestamp: 1_700_000_000 + n,
                gas_used: 0,
                gas_limit: 30_000_000,
                transactions: vec![],
                l1_block_number: 0,
            };
            storage.store_block(&blk).await.unwrap();
        }

        // Build state with short TTL
        cfg.rpc.filter_ttl_ms = 200;
        let state = ServerState {
            config: cfg,
            storage: Some(Arc::clone(&storage)),
            filters: Arc::new(Mutex::new(FiltersManager {
                next_id: 0,
                installed: HashMap::new(),
                ttl_millis: 200,
                pruned_total: 0,
            })),
        };

        // Create a filter via handler
        let req_obj = JsonRpcRequest {
            jsonrpc: Some("2.0".into()),
            id: Some(serde_json::json!(1)),
            method: "eth_newFilter".into(),
            params: Some(serde_json::json!([{ "fromBlock": "0x0", "toBlock": "latest" }])),
        };
        let resp = json_rpc(State(state.clone()), axum::Json(req_obj))
            .await
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id_hex = v["result"].as_str().unwrap().to_string();
        let id = u64::from_str_radix(id_hex.trim_start_matches("0x"), 16).unwrap();

        // Backdate last_seen to force prune
        let now = chrono::Utc::now().timestamp_millis() as u64;
        storage
            .touch_filter_last_seen(id, now.saturating_sub(300))
            .await
            .unwrap();

        // Trigger getFilterChanges, which should prune
        let req2 = JsonRpcRequest {
            jsonrpc: Some("2.0".into()),
            id: Some(serde_json::json!(2)),
            method: "eth_getFilterChanges".into(),
            params: Some(serde_json::json!([format!("0x{:x}", id)])),
        };
        let resp2 = json_rpc(State(state.clone()), axum::Json(req2))
            .await
            .into_response();
        assert_eq!(resp2.status(), axum::http::StatusCode::OK);

        // Health should report pruned_total >= 1
        let h = health(State(state)).await.into_response();
        let hbytes = axum::body::to_bytes(h.into_body(), usize::MAX)
            .await
            .unwrap();
        let hv: serde_json::Value = serde_json::from_slice(&hbytes).unwrap();
        assert!(hv["filters"]["pruned_total"].as_u64().unwrap_or(0) >= 1);
    }
}

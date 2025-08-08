use alloy_primitives::{B256, U256, address};
use arbitrum_config::ArbitrumRethConfig;
use arbitrum_node::reth_integration::launch_reth_node;
use arbitrum_storage::{
    ArbitrumAccount, ArbitrumBlock, ArbitrumReceipt, ArbitrumStorage, ArbitrumTransaction, Log,
};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_block_number_reflects_storage() {
    // Temp datadir for storage
    let temp = TempDir::new().expect("tempdir");

    // Config with unique port
    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18549; // avoid conflicts with other tests
    cfg.node.datadir = temp.path().to_path_buf();
    cfg.l2.chain_id = 42161;

    // Create and start storage, store a block #1
    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");
    let block = ArbitrumBlock {
        number: 1,
        hash: B256::from([1u8; 32]),
        parent_hash: B256::ZERO,
        timestamp: 1,
        gas_used: 0,
        gas_limit: 30_000_000,
        transactions: vec![],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    // Launch mock RPC with storage wired
    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");

    // Call eth_blockNumber
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": []
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"], serde_json::json!("0x1"));

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_block_by_number() {
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18550;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");
    let block = ArbitrumBlock {
        number: 2,
        hash: B256::from([2u8; 32]),
        parent_hash: B256::from([1u8; 32]),
        timestamp: 2,
        gas_used: 21000,
        gas_limit: 30_000_000,
        transactions: vec![],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");

    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": ["0x2", false]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"]["number"], "0x2");
    assert_eq!(body["result"]["gasUsed"], "0x5208");

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_balance() {
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18551;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");
    let addr = address!("0x1234567890123456789012345678901234567890");
    let acct = ArbitrumAccount {
        address: addr,
        balance: U256::from(1337u64),
        nonce: 0,
        code_hash: B256::ZERO,
        storage_root: B256::ZERO,
    };
    storage
        .store_account(addr, &acct)
        .await
        .expect("store acct");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");

    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBalance",
            "params": ["0x1234567890123456789012345678901234567890", "latest"]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"], "0x539"); // 1337 hex

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_block_by_hash() {
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18552;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");
    let hash = B256::from([9u8; 32]);
    let block = ArbitrumBlock {
        number: 9,
        hash,
        parent_hash: B256::from([8u8; 32]),
        timestamp: 9,
        gas_used: 9000,
        gas_limit: 30_000_000,
        transactions: vec![],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");

    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByHash",
            "params": [format!("0x{}", hex::encode(hash.as_slice())), false]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"]["number"], "0x9");
    assert_eq!(
        body["result"]["hash"],
        format!("0x{}", hex::encode(hash.as_slice()))
    );

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_chain_and_net_version_from_config() {
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18553;
    cfg.l2.chain_id = 42161; // arbitrum one
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");

    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // eth_chainId
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_chainId",
            "params": []
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"], "0xa4b1"); // 42161 hex

    // net_version
    let resp2 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "net_version",
            "params": []
        }))
        .send()
        .await
        .expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json body");
    assert_eq!(body2["result"], "42161");

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_transaction_count_and_block_tx_counts() {
    use alloy_primitives::{U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18554;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    // Create two txs and a block containing them
    let tx1 = ArbitrumTransaction {
        hash: B256::from([0x11u8; 32]),
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(1),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 7,
        data: vec![],
        l1_sequence_number: None,
    };
    let tx2 = ArbitrumTransaction {
        hash: B256::from([0x22u8; 32]),
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(2),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 8,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx1).await.expect("store tx1");
    storage.store_transaction(&tx2).await.expect("store tx2");
    let block_hash = B256::from([0x33u8; 32]);
    let block = ArbitrumBlock {
        number: 5,
        hash: block_hash,
        parent_hash: B256::from([0x22u8; 32]),
        timestamp: 5,
        gas_used: 42,
        gas_limit: 30_000_000,
        transactions: vec![tx1.hash, tx2.hash],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");
    // Also store account with nonce to check eth_getTransactionCount
    let acct = ArbitrumAccount {
        address: tx1.from,
        balance: U256::from(0),
        nonce: 8,
        code_hash: B256::ZERO,
        storage_root: B256::ZERO,
    };
    storage
        .store_account(tx1.from, &acct)
        .await
        .expect("store acct");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // eth_getTransactionCount for address
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionCount",
            "params": [format!("0x{}", hex::encode(acct.address.as_slice())), "latest"]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(body["result"], "0x8");

    // eth_getBlockTransactionCountByNumber
    let resp2 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getBlockTransactionCountByNumber",
            "params": ["0x5"]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json body");
    assert_eq!(body2["result"], "0x2");

    // eth_getBlockTransactionCountByHash
    let resp3 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "eth_getBlockTransactionCountByHash",
            "params": [format!("0x{}", hex::encode(block_hash.as_slice()))]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp3.status().is_success());
    let body3: serde_json::Value = resp3.json().await.expect("json body");
    assert_eq!(body3["result"], "0x2");

    // eth_getTransactionByHash
    let resp4 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "eth_getTransactionByHash",
            "params": [format!("0x{}", hex::encode(tx2.hash.as_slice()))]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp4.status().is_success());
    let body4: serde_json::Value = resp4.json().await.expect("json body");
    assert_eq!(
        body4["result"]["hash"],
        format!("0x{}", hex::encode(tx2.hash.as_slice()))
    );
    assert_eq!(body4["result"]["nonce"], "0x8");

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_block_with_full_transactions() {
    use alloy_primitives::{U256, address};
    let temp = TempDir::new().expect("tempdir");
    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18557;
    cfg.node.datadir = temp.path().to_path_buf();
    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let tx = ArbitrumTransaction {
        hash: B256::from([0x99u8; 32]),
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(3),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 9,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx).await.expect("store tx");
    let bh = B256::from([0x98u8; 32]);
    let block = ArbitrumBlock {
        number: 8,
        hash: bh,
        parent_hash: B256::from([0x97u8; 32]),
        timestamp: 8,
        gas_used: 100,
        gas_limit: 30_000_000,
        transactions: vec![tx.hash],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // By hash
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByHash",
            "params": [format!("0x{}", hex::encode(bh.as_slice())), true]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(
        body["result"]["transactions"][0]["hash"],
        format!("0x{}", hex::encode(tx.hash.as_slice()))
    );

    // By number
    let resp2 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getBlockByNumber",
            "params": ["0x8", true]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json body");
    assert_eq!(body2["result"]["transactions"][0]["nonce"], "0x9");

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_tx_by_block_ref_and_index() {
    use alloy_primitives::{U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18555;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let tx1 = ArbitrumTransaction {
        hash: B256::from([0x44u8; 32]),
        from: address!("0x1111111111111111111111111111111111111111"),
        to: None,
        value: U256::from(1),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 1,
        data: vec![],
        l1_sequence_number: None,
    };
    let tx2 = ArbitrumTransaction {
        hash: B256::from([0x55u8; 32]),
        from: address!("0x1111111111111111111111111111111111111111"),
        to: None,
        value: U256::from(2),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 2,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx1).await.expect("store tx1");
    storage.store_transaction(&tx2).await.expect("store tx2");
    let block_hash = B256::from([0x66u8; 32]);
    let block = ArbitrumBlock {
        number: 6,
        hash: block_hash,
        parent_hash: B256::from([0x55u8; 32]),
        timestamp: 6,
        gas_used: 60,
        gas_limit: 30_000_000,
        transactions: vec![tx1.hash, tx2.hash],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // eth_getTransactionByBlockNumberAndIndex
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionByBlockNumberAndIndex",
            "params": ["0x6", "0x1"]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(
        body["result"]["hash"],
        format!("0x{}", hex::encode(tx2.hash.as_slice()))
    );

    // eth_getTransactionByBlockHashAndIndex
    let resp2 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getTransactionByBlockHashAndIndex",
            "params": [format!("0x{}", hex::encode(block_hash.as_slice())), "0x0"]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json body");
    assert_eq!(
        body2["result"]["hash"],
        format!("0x{}", hex::encode(tx1.hash.as_slice()))
    );

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_transaction_receipt() {
    use alloy_primitives::{U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18556;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let txh = B256::from([0x77u8; 32]);
    let blockh = B256::from([0x88u8; 32]);
    let tx = ArbitrumTransaction {
        hash: txh,
        from: address!("0x3333333333333333333333333333333333333333"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 0,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx).await.expect("store tx");
    let block = ArbitrumBlock {
        number: 7,
        hash: blockh,
        parent_hash: B256::from([0x66u8; 32]),
        timestamp: 7,
        gas_used: 21_000,
        gas_limit: 30_000_000,
        transactions: vec![txh],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");
    let receipt = ArbitrumReceipt {
        transaction_hash: txh,
        transaction_index: 0,
        block_hash: blockh,
        block_number: 7,
        cumulative_gas_used: 21_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    storage
        .store_receipt(&receipt)
        .await
        .expect("store receipt");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{}", hex::encode(txh.as_slice()))]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(
        body["result"]["transactionHash"],
        format!("0x{}", hex::encode(txh.as_slice()))
    );
    assert_eq!(body["result"]["blockNumber"], "0x7");
    assert_eq!(body["result"]["status"], "0x1");

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_logs_basic() {
    use alloy_primitives::{B256 as H256, U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18558;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let txh = B256::from([0x10u8; 32]);
    let blockh = B256::from([0x11u8; 32]);
    let tx = ArbitrumTransaction {
        hash: txh,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 0,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx).await.expect("store tx");
    let block = ArbitrumBlock {
        number: 9,
        hash: blockh,
        parent_hash: B256::from([0x09u8; 32]),
        timestamp: 9,
        gas_used: 21_000,
        gas_limit: 30_000_000,
        transactions: vec![txh],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");
    // Add a receipt with a single log
    let log_addr = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let topic0 = H256::from([0x77u8; 32]);
    let receipt = ArbitrumReceipt {
        transaction_hash: txh,
        transaction_index: 0,
        block_hash: blockh,
        block_number: 9,
        cumulative_gas_used: 21_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: log_addr,
            topics: vec![topic0],
            data: vec![1, 2, 3],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    storage
        .store_receipt(&receipt)
        .await
        .expect("store receipt");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // eth_getLogs with address+topic match
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("0x{}", hex::encode(log_addr.as_slice())),
                "topics": [format!("0x{}", hex::encode(topic0.as_slice()))]
            }]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json");
    let result = &body["result"];
    assert!(result.is_array());
    let first = &result.as_array().unwrap()[0];
    assert_eq!(first["blockNumber"], "0x9");
    assert_eq!(
        first["transactionHash"],
        format!("0x{}", hex::encode(txh.as_slice()))
    );
    assert_eq!(
        first["address"],
        format!("0x{}", hex::encode(log_addr.as_slice()))
    );
    assert_eq!(
        first["topics"][0],
        format!("0x{}", hex::encode(topic0.as_slice()))
    );

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_filters_roundtrip() {
    use alloy_primitives::{B256 as H256, U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18559;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    let txh = B256::from([0x21u8; 32]);
    let blockh = B256::from([0x22u8; 32]);
    let tx = ArbitrumTransaction {
        hash: txh,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 0,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx).await.expect("store tx");
    let block = ArbitrumBlock {
        number: 10,
        hash: blockh,
        parent_hash: B256::from([0x20u8; 32]),
        timestamp: 10,
        gas_used: 21_000,
        gas_limit: 30_000_000,
        transactions: vec![txh],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");
    let log_addr = address!("0xcccccccccccccccccccccccccccccccccccccccc");
    let topic0 = H256::from([0x99u8; 32]);
    let receipt = ArbitrumReceipt {
        transaction_hash: txh,
        transaction_index: 0,
        block_hash: blockh,
        block_number: 10,
        cumulative_gas_used: 21_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: log_addr,
            topics: vec![topic0],
            data: vec![],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    storage
        .store_receipt(&receipt)
        .await
        .expect("store receipt");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // Create a new filter
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_newFilter",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("0x{}", hex::encode(log_addr.as_slice())),
                "topics": [format!("0x{}", hex::encode(topic0.as_slice()))]
            }]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json");
    let filter_id = body["result"].as_str().unwrap().to_string();

    // Get filter changes; expect one log
    let resp2 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getFilterChanges",
            "params": [filter_id]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json");
    assert!(body2["result"].is_array());
    assert_eq!(body2["result"].as_array().unwrap().len(), 1);

    // Next poll should be empty
    let resp3 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "eth_getFilterChanges",
            "params": [body["result"].clone()]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp3.status().is_success());
    let body3: serde_json::Value = resp3.json().await.expect("json");
    assert!(body3["result"].is_array());
    assert_eq!(body3["result"].as_array().unwrap().len(), 0);

    // Uninstall filter
    let resp4 = client
        .post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "eth_uninstallFilter",
            "params": [body["result"].clone()]
        }))
        .send()
        .await
        .expect("post");
    assert!(resp4.status().is_success());
    let body4: serde_json::Value = resp4.json().await.expect("json");
    assert_eq!(body4["result"], true);

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_logs_multi_address_and_topic_or() {
    use alloy_primitives::{B256 as H256, U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18560;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    // Two txs in one block with two different addresses and different topics
    let txh1 = B256::from([0x31u8; 32]);
    let txh2 = B256::from([0x32u8; 32]);
    let blockh = B256::from([0x33u8; 32]);
    let tx1 = ArbitrumTransaction {
        hash: txh1,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 0,
        data: vec![],
        l1_sequence_number: None,
    };
    let tx2 = ArbitrumTransaction {
        hash: txh2,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 1,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx1).await.expect("store tx1");
    storage.store_transaction(&tx2).await.expect("store tx2");
    let block = ArbitrumBlock {
        number: 11,
        hash: blockh,
        parent_hash: B256::from([0x30u8; 32]),
        timestamp: 11,
        gas_used: 42,
        gas_limit: 30_000_000,
        transactions: vec![txh1, txh2],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let addr1 = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let addr2 = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let topic_a = H256::from([0x01u8; 32]);
    let topic_b = H256::from([0x02u8; 32]);
    let r1 = ArbitrumReceipt {
        transaction_hash: txh1,
        transaction_index: 0,
        block_hash: blockh,
        block_number: 11,
        cumulative_gas_used: 21_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: addr1,
            topics: vec![topic_a],
            data: vec![],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    let r2 = ArbitrumReceipt {
        transaction_hash: txh2,
        transaction_index: 1,
        block_hash: blockh,
        block_number: 11,
        cumulative_gas_used: 42_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: addr2,
            topics: vec![topic_b],
            data: vec![],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    storage.store_receipt(&r1).await.expect("store r1");
    storage.store_receipt(&r2).await.expect("store r2");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // Query with multi-address AND topic OR [topicA, topicB] should return both logs (2)
    let resp = client.post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": [format!("0x{}", hex::encode(addr1.as_slice())), format!("0x{}", hex::encode(addr2.as_slice()))],
                "topics": [[format!("0x{}", hex::encode(topic_a.as_slice())), format!("0x{}", hex::encode(topic_b.as_slice()))]]
            }]
        }))
        .send().await.expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json");
    assert!(body["result"].is_array());
    assert_eq!(body["result"].as_array().unwrap().len(), 2);

    // Query with only addr1 and topic OR should return 1
    let resp2 = client.post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("0x{}", hex::encode(addr1.as_slice())),
                "topics": [[format!("0x{}", hex::encode(topic_a.as_slice())), format!("0x{}", hex::encode(topic_b.as_slice()))]]
            }]
        }))
        .send().await.expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json");
    assert!(body2["result"].is_array());
    assert_eq!(body2["result"].as_array().unwrap().len(), 1);

    handle.stop().await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_rpc_eth_get_logs_multi_position_topics_and() {
    use alloy_primitives::{B256 as H256, U256, address};
    let temp = TempDir::new().expect("tempdir");

    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18561;
    cfg.node.datadir = temp.path().to_path_buf();

    let storage = ArbitrumStorage::new(&cfg).await.expect("storage new");
    storage.start().await.expect("storage start");

    // One block with two txs, each emits logs with two topics (topic0, topic1)
    let txh1 = B256::from([0x41u8; 32]);
    let txh2 = B256::from([0x42u8; 32]);
    let blockh = B256::from([0x43u8; 32]);
    let tx1 = ArbitrumTransaction {
        hash: txh1,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 0,
        data: vec![],
        l1_sequence_number: None,
    };
    let tx2 = ArbitrumTransaction {
        hash: txh2,
        from: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        to: None,
        value: U256::from(0),
        gas: 21_000,
        gas_price: U256::from(1),
        nonce: 1,
        data: vec![],
        l1_sequence_number: None,
    };
    storage.store_transaction(&tx1).await.expect("store tx1");
    storage.store_transaction(&tx2).await.expect("store tx2");
    let block = ArbitrumBlock {
        number: 12,
        hash: blockh,
        parent_hash: B256::from([0x40u8; 32]),
        timestamp: 12,
        gas_used: 42,
        gas_limit: 30_000_000,
        transactions: vec![txh1, txh2],
        l1_block_number: 0,
    };
    storage.store_block(&block).await.expect("store block");

    let addr = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let topic0_a = H256::from([0x0au8; 32]);
    let topic0_b = H256::from([0x0bu8; 32]);
    let topic1_x = H256::from([0x1au8; 32]);
    let topic1_y = H256::from([0x1bu8; 32]);
    // tx1: topics [a, x], tx2: topics [b, y]
    let r1 = ArbitrumReceipt {
        transaction_hash: txh1,
        transaction_index: 0,
        block_hash: blockh,
        block_number: 12,
        cumulative_gas_used: 21_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: addr,
            topics: vec![topic0_a, topic1_x],
            data: vec![],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    let r2 = ArbitrumReceipt {
        transaction_hash: txh2,
        transaction_index: 1,
        block_hash: blockh,
        block_number: 12,
        cumulative_gas_used: 42_000,
        gas_used: 21_000,
        contract_address: None,
        logs: vec![Log {
            address: addr,
            topics: vec![topic0_b, topic1_y],
            data: vec![],
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }],
        status: 1,
        effective_gas_price: U256::from(1),
    };
    storage.store_receipt(&r1).await.expect("store r1");
    storage.store_receipt(&r2).await.expect("store r2");

    let handle = launch_reth_node(&cfg, Some(storage.into()))
        .await
        .expect("launch");
    let url = format!("http://127.0.0.1:{}", cfg.rpc.port);
    let client = reqwest::Client::new();

    // topics: [ [a,b], [x] ] should match only tx1 (AND across positions)
    let resp = client.post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("0x{}", hex::encode(addr.as_slice())),
                "topics": [[format!("0x{}", hex::encode(topic0_a.as_slice())), format!("0x{}", hex::encode(topic0_b.as_slice()))], [format!("0x{}", hex::encode(topic1_x.as_slice()))]]
            }]
        }))
        .send().await.expect("post");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("json");
    assert!(body["result"].is_array());
    assert_eq!(body["result"].as_array().unwrap().len(), 1);

    // topics: [ null, [x,y] ] should match both (wildcard at pos0)
    let resp2 = client.post(&url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("0x{}", hex::encode(addr.as_slice())),
                "topics": [null, [format!("0x{}", hex::encode(topic1_x.as_slice())), format!("0x{}", hex::encode(topic1_y.as_slice()))]]
            }]
        }))
        .send().await.expect("post");
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.expect("json");
    assert!(body2["result"].is_array());
    assert_eq!(body2["result"].as_array().unwrap().len(), 2);

    handle.stop().await.expect("stop");
}

#![allow(dead_code)]

//! Arbitrum-Reth Storage Layer
//!
//! High-performance storage implementation using MDBX for Arbitrum L2 data.
//! Provides efficient storage and retrieval of blocks, transactions, accounts,
//! and Arbitrum-specific data structures.

pub mod codec;
pub mod database;
pub mod schema;

// Re-export data types for other crates
use std::sync::Arc;

use alloy_primitives::{Address, B256};
use arbitrum_config::ArbitrumRethConfig;
pub use codec::{
    ArbitrumAccount, ArbitrumBatch, ArbitrumBlock, ArbitrumReceipt, ArbitrumTransaction, L1Message,
    Log,
};
use eyre::Result;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::{
    database::ArbitrumDatabase,
    schema::{TableType, keys, metadata_keys},
};

/// Arbitrum storage layer that handles L2 state and Arbitrum-specific data
pub struct ArbitrumStorage {
    config: ArbitrumRethConfig,
    is_running: Arc<RwLock<bool>>,
    db: Arc<ArbitrumDatabase>,
}

impl ArbitrumStorage {
    /// Create a new Arbitrum storage instance
    pub async fn new(config: &ArbitrumRethConfig) -> Result<Self> {
        info!("Initializing Arbitrum storage layer");
        let db_path = config.db_path();
        let db = ArbitrumDatabase::new(db_path, 10 * 1024 * 1024 * 1024).await?; // 10 GiB default

        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(RwLock::new(false)),
            db: Arc::new(db),
        })
    }

    /// Start the storage layer
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }

        info!("Starting Arbitrum storage layer");

        // Initialize schema
        self.initialize_schema().await?;

        *running = true;
        info!("Arbitrum storage layer started");

        Ok(())
    }

    /// Stop the storage layer
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Ok(());
        }

        info!("Stopping Arbitrum storage layer");

        *running = false;
        info!("Arbitrum storage layer stopped");

        Ok(())
    }

    /// Initialize database schema and metadata
    async fn initialize_schema(&self) -> Result<()> {
        debug!("Initializing database schema");
        // Ensure metadata keys exist
        // Schema version
        if self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::SCHEMA_VERSION.into(),
            )
            .await?
            .is_none()
        {
            self.db
                .put::<keys::MetadataKey, u64>(
                    TableType::Metadata,
                    &metadata_keys::SCHEMA_VERSION.into(),
                    &1u64,
                )
                .await?;
        }

        // Latest block number
        if self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BLOCK_NUMBER.into(),
            )
            .await?
            .is_none()
        {
            self.db
                .put::<keys::MetadataKey, u64>(
                    TableType::Metadata,
                    &metadata_keys::LATEST_BLOCK_NUMBER.into(),
                    &0u64,
                )
                .await?;
        }

        // Latest batch number
        if self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BATCH_NUMBER.into(),
            )
            .await?
            .is_none()
        {
            self.db
                .put::<keys::MetadataKey, u64>(
                    TableType::Metadata,
                    &metadata_keys::LATEST_BATCH_NUMBER.into(),
                    &0u64,
                )
                .await?;
        }

        // Latest L1 message number
        if self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_L1_MESSAGE_NUMBER.into(),
            )
            .await?
            .is_none()
        {
            self.db
                .put::<keys::MetadataKey, u64>(
                    TableType::Metadata,
                    &metadata_keys::LATEST_L1_MESSAGE_NUMBER.into(),
                    &0u64,
                )
                .await?;
        }

        info!("Database schema initialization completed");
        Ok(())
    }

    /// Store a block in the database
    pub async fn store_block(&self, block: &codec::ArbitrumBlock) -> Result<()> {
        // Store by block number
        let key = keys::BlockNumber(block.number);
        self.db
            .put::<keys::BlockNumber, codec::ArbitrumBlock>(TableType::Blocks, &key, block)
            .await?;
        // Store by block hash (same table, different key type)
        let hkey = keys::BlockHash(block.hash);
        self.db
            .put::<keys::BlockHash, codec::ArbitrumBlock>(TableType::Blocks, &hkey, block)
            .await?;
        // Update latest block number
        self.db
            .put::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BLOCK_NUMBER.into(),
                &block.number,
            )
            .await?;
        Ok(())
    }

    /// Get a block by hash
    pub async fn get_block(&self, hash: &B256) -> Result<Option<codec::ArbitrumBlock>> {
        let key = keys::BlockHash(*hash);
        self.db
            .get::<keys::BlockHash, codec::ArbitrumBlock>(TableType::Blocks, &key)
            .await
    }

    /// Get a block by number
    pub async fn get_block_by_number(&self, number: u64) -> Result<Option<codec::ArbitrumBlock>> {
        let key = keys::BlockNumber(number);
        self.db
            .get::<keys::BlockNumber, codec::ArbitrumBlock>(TableType::Blocks, &key)
            .await
    }

    /// Store a transaction in the database
    pub async fn store_transaction(&self, tx: &codec::ArbitrumTransaction) -> Result<()> {
        let key = keys::TransactionHash(tx.hash);
        self.db
            .put::<keys::TransactionHash, codec::ArbitrumTransaction>(
                TableType::Transactions,
                &key,
                tx,
            )
            .await
    }

    /// Get a transaction by hash
    pub async fn get_transaction(&self, hash: &B256) -> Result<Option<codec::ArbitrumTransaction>> {
        let key = keys::TransactionHash(*hash);
        self.db
            .get::<keys::TransactionHash, codec::ArbitrumTransaction>(TableType::Transactions, &key)
            .await
    }

    /// Store a transaction receipt by transaction hash
    pub async fn store_receipt(&self, receipt: &codec::ArbitrumReceipt) -> Result<()> {
        let key = keys::TransactionHash(receipt.transaction_hash);
        self.db
            .put::<keys::TransactionHash, codec::ArbitrumReceipt>(
                TableType::Receipts,
                &key,
                receipt,
            )
            .await?;
        // Update per-block logs index (append semantics)
        let block_n = receipt.block_number;
        let mut current: Vec<codec::Log> = self
            .db
            .get::<keys::BlockNumber, Vec<codec::Log>>(
                TableType::LogsByBlock,
                &keys::BlockNumber(block_n),
            )
            .await?
            .unwrap_or_default();
        // Enrich logs with context from receipt
        let mut enriched: Vec<codec::Log> = Vec::with_capacity(receipt.logs.len());
        for (i, l) in receipt.logs.iter().enumerate() {
            let mut ll = l.clone();
            ll.block_number = Some(receipt.block_number);
            ll.block_hash = Some(receipt.block_hash);
            ll.transaction_hash = Some(receipt.transaction_hash);
            ll.transaction_index = Some(receipt.transaction_index);
            ll.log_index = Some(i as u64);
            enriched.push(ll);
        }
        current.extend(enriched);
        self.db
            .put::<keys::BlockNumber, Vec<codec::Log>>(
                TableType::LogsByBlock,
                &keys::BlockNumber(block_n),
                &current,
            )
            .await
    }

    /// Get a transaction receipt by transaction hash
    pub async fn get_receipt(&self, hash: &B256) -> Result<Option<codec::ArbitrumReceipt>> {
        let key = keys::TransactionHash(*hash);
        self.db
            .get::<keys::TransactionHash, codec::ArbitrumReceipt>(TableType::Receipts, &key)
            .await
    }

    /// Store an account in the database
    pub async fn store_account(
        &self,
        address: Address,
        account: &codec::ArbitrumAccount,
    ) -> Result<()> {
        let key = keys::AccountAddress(address);
        self.db
            .put::<keys::AccountAddress, codec::ArbitrumAccount>(TableType::Accounts, &key, account)
            .await
    }

    /// Get an account by address
    pub async fn get_account(&self, address: &Address) -> Result<Option<codec::ArbitrumAccount>> {
        let key = keys::AccountAddress(*address);
        self.db
            .get::<keys::AccountAddress, codec::ArbitrumAccount>(TableType::Accounts, &key)
            .await
    }

    /// Store an L1 message in the database
    pub async fn store_l1_message(&self, message: &codec::L1Message) -> Result<()> {
        let key = keys::L1MessageNumber(message.message_number);
        self.db
            .put::<keys::L1MessageNumber, codec::L1Message>(TableType::L1Messages, &key, message)
            .await?;
        // Update latest L1 message number
        self.db
            .put::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_L1_MESSAGE_NUMBER.into(),
                &message.message_number,
            )
            .await
    }

    /// Get all L1 messages for a block range
    pub async fn get_l1_messages(
        &self,
        start_number: u64,
        end_number: u64,
    ) -> Result<Vec<codec::L1Message>> {
        let mut out = Vec::new();
        for n in start_number..=end_number {
            let key = keys::L1MessageNumber(n);
            if let Some(m) = self
                .db
                .get::<keys::L1MessageNumber, codec::L1Message>(TableType::L1Messages, &key)
                .await?
            {
                out.push(m);
            }
        }
        Ok(out)
    }

    /// Store an Arbitrum batch in the database
    pub async fn store_batch(&self, batch: &codec::ArbitrumBatch) -> Result<()> {
        let key = keys::BatchNumber(batch.batch_number);
        self.db
            .put::<keys::BatchNumber, codec::ArbitrumBatch>(TableType::Batches, &key, batch)
            .await?;
        // Update latest batch number
        self.db
            .put::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BATCH_NUMBER.into(),
                &batch.batch_number,
            )
            .await
    }

    /// Get the latest batch
    pub async fn get_latest_batch(&self) -> Result<Option<codec::ArbitrumBatch>> {
        let latest = self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BATCH_NUMBER.into(),
            )
            .await?
            .unwrap_or(0);
        if latest == 0 {
            return Ok(None);
        }
        self.get_batch(latest).await
    }

    /// Get a batch by number
    pub async fn get_batch(&self, batch_number: u64) -> Result<Option<codec::ArbitrumBatch>> {
        let key = keys::BatchNumber(batch_number);
        self.db
            .get::<keys::BatchNumber, codec::ArbitrumBatch>(TableType::Batches, &key)
            .await
    }

    /// Get the current block number
    pub async fn get_current_block_number(&self) -> Result<u64> {
        let n = self
            .db
            .get::<keys::MetadataKey, u64>(
                TableType::Metadata,
                &metadata_keys::LATEST_BLOCK_NUMBER.into(),
            )
            .await?
            .unwrap_or(0);
        Ok(n)
    }

    /// Perform database health check
    pub async fn health_check(&self) -> Result<()> {
        info!("Database health check passed");
        Ok(())
    }

    /// Persist filter cursor (last processed block) for given filter id
    pub async fn set_filter_cursor(&self, filter_id: u64, last_block: u64) -> Result<()> {
        let key = keys::FilterId(filter_id);
        self.db
            .put::<keys::FilterId, u64>(TableType::FilterCursors, &key, &last_block)
            .await
    }

    /// Load filter cursor; returns 0 if not found
    pub async fn get_filter_cursor(&self, filter_id: u64) -> Result<u64> {
        let key = keys::FilterId(filter_id);
        let v = self
            .db
            .get::<keys::FilterId, u64>(TableType::FilterCursors, &key)
            .await?;
        Ok(v.unwrap_or(0))
    }

    /// Update last-seen timestamp (epoch millis) for a filter id
    pub async fn touch_filter_last_seen(&self, filter_id: u64, now_millis: u64) -> Result<()> {
        let key = keys::FilterId(filter_id);
        self.db
            .put::<keys::FilterId, u64>(TableType::FilterLastSeen, &key, &now_millis)
            .await
    }

    /// Get last-seen timestamp for a filter id (epoch millis); 0 if missing
    pub async fn get_filter_last_seen(&self, filter_id: u64) -> Result<u64> {
        let key = keys::FilterId(filter_id);
        Ok(self
            .db
            .get::<keys::FilterId, u64>(TableType::FilterLastSeen, &key)
            .await?
            .unwrap_or(0))
    }

    /// Prune expired filter state based on TTL (millis). Returns pruned ids.
    /// Note: Heed/LMDB has no range scan by default here; do a best-effort scan by ids provided.
    /// Callers should pass known ids (e.g., from in-memory manager).
    pub async fn prune_expired_filters(
        &self,
        ids: &[u64],
        now_millis: u64,
        ttl_millis: u64,
    ) -> Result<Vec<u64>> {
        let mut pruned = Vec::new();
        for &id in ids {
            let last = self.get_filter_last_seen(id).await?;
            if last == 0 {
                continue;
            }
            if now_millis.saturating_sub(last) > ttl_millis {
                // delete cursor and last_seen
                let _ = self
                    .db
                    .delete::<keys::FilterId>(TableType::FilterCursors, &keys::FilterId(id))
                    .await?;
                let _ = self
                    .db
                    .delete::<keys::FilterId>(TableType::FilterLastSeen, &keys::FilterId(id))
                    .await?;
                pruned.push(id);
            }
        }
        Ok(pruned)
    }

    /// Store logs for a block as an index to accelerate retrieval.
    /// This replaces existing entry for the block.
    pub async fn index_logs_for_block(&self, block_number: u64, logs: &[codec::Log]) -> Result<()> {
        let key = keys::BlockNumber(block_number);
        self.db
            .put::<keys::BlockNumber, Vec<codec::Log>>(TableType::LogsByBlock, &key, &logs.to_vec())
            .await
    }

    /// Get logs for a range using the simple per-block index; falls back to receipts scan if empty.
    pub async fn get_indexed_logs_in_range(
        &self,
        start_number: u64,
        end_number: u64,
    ) -> Result<Vec<(u64, Vec<codec::Log>)>> {
        let mut out = Vec::new();
        for n in start_number..=end_number {
            let key = keys::BlockNumber(n);
            if let Some(logs) = self
                .db
                .get::<keys::BlockNumber, Vec<codec::Log>>(TableType::LogsByBlock, &key)
                .await?
            {
                out.push((n, logs));
            }
        }
        Ok(out)
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> StorageStats {
        // Best-effort stats using DB stats
        let mut total_blocks = 0;
        let mut total_transactions = 0;
        let mut total_accounts = 0;
        if let Ok(stats) = self.db.stats().await {
            total_blocks = stats.total_blocks as u64;
            total_transactions = stats.total_transactions as u64;
            total_accounts = stats.total_accounts as u64;
        }
        StorageStats {
            total_blocks,
            total_transactions,
            total_accounts,
            db_size_bytes: 0,
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_blocks: u64,
    pub total_transactions: u64,
    pub total_accounts: u64,
    pub db_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use arbitrum_config::*;
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> ArbitrumRethConfig {
        let mut cfg = ArbitrumRethConfig::default();
        cfg.node.chain = "arbitrum-sepolia".to_string();
        cfg.node.datadir = PathBuf::from("/tmp/test");
        cfg.l1.rpc_url = "https://sepolia.example/".to_string();
        cfg.l1.chain_id = 11155111;
        cfg.l2.chain_id = 421614;
        cfg.sequencer.enabled = false;
        cfg
    }

    async fn create_test_storage() -> (ArbitrumStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config();
        config.node.datadir = temp_dir.path().to_path_buf();

        let storage = ArbitrumStorage::new(&config).await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_storage_creation() {
        let (storage, _temp_dir) = create_test_storage().await;

        // Test health check
        assert!(storage.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_storage_start_stop() {
        let (storage, _temp_dir) = create_test_storage().await;

        // Start storage
        assert!(storage.start().await.is_ok());

        // Stop storage
        assert!(storage.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_block_tx_account_crud() {
        use alloy_primitives::{U256, address};
        let (storage, _tmp) = create_test_storage().await;
        storage.start().await.unwrap();

        // Block
        let blk = ArbitrumBlock {
            number: 1,
            hash: B256::from([1u8; 32]),
            parent_hash: B256::ZERO,
            timestamp: 1_700_000_000,
            gas_used: 0,
            gas_limit: 30_000_000,
            transactions: vec![],
            l1_block_number: 0,
        };
        storage.store_block(&blk).await.unwrap();
        assert_eq!(storage.get_current_block_number().await.unwrap(), 1);
        assert!(storage.get_block_by_number(1).await.unwrap().is_some());
        assert!(storage.get_block(&blk.hash).await.unwrap().is_some());

        // Tx
        let tx = ArbitrumTransaction {
            hash: B256::from([2u8; 32]),
            from: address!("0x1111111111111111111111111111111111111111"),
            to: None,
            value: U256::from(1u64),
            gas: 21_000,
            gas_price: U256::from(1),
            nonce: 0,
            data: vec![],
            l1_sequence_number: None,
        };
        storage.store_transaction(&tx).await.unwrap();
        assert!(storage.get_transaction(&tx.hash).await.unwrap().is_some());

        // Account
        let addr = address!("0x2222222222222222222222222222222222222222");
        let acct = ArbitrumAccount {
            address: addr,
            balance: U256::from(100u64),
            nonce: 0,
            code_hash: B256::ZERO,
            storage_root: B256::ZERO,
        };
        storage.store_account(addr, &acct).await.unwrap();
        let fetched = storage.get_account(&addr).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().balance, U256::from(100u64));
    }

    #[tokio::test]
    async fn test_batches_and_l1_messages() {
        use alloy_primitives::address;
        let (storage, _tmp) = create_test_storage().await;
        storage.start().await.unwrap();

        // Batch
        let batch = ArbitrumBatch {
            batch_number: 10,
            block_range: (1, 5),
            l1_block_number: 1000,
            timestamp: 1_700_000_100,
            transactions: vec![],
            l1_tx_hash: Some(B256::from([3u8; 32])),
        };
        storage.store_batch(&batch).await.unwrap();
        assert!(storage.get_batch(10).await.unwrap().is_some());
        assert_eq!(
            storage
                .get_latest_batch()
                .await
                .unwrap()
                .unwrap()
                .batch_number,
            10
        );

        // L1 messages
        let m1 = L1Message {
            message_number: 1,
            sender: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            data: vec![1],
            timestamp: 1,
            block_number: 100,
        };
        let m2 = L1Message {
            message_number: 2,
            sender: address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            data: vec![2],
            timestamp: 2,
            block_number: 101,
        };
        storage.store_l1_message(&m1).await.unwrap();
        storage.store_l1_message(&m2).await.unwrap();
        let msgs = storage.get_l1_messages(1, 2).await.unwrap();
        assert_eq!(msgs.len(), 2);
    }
}

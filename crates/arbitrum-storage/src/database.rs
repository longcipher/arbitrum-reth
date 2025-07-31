//! Database implementation using Heed (LMDB) for high-performance storage
//!
//! This module provides the core database functionality for Arbitrum-Reth,
//! implementing efficient storage for blocks, transactions, accounts, and
//! Arbitrum-specific data structures.

use std::{path::Path, sync::Arc};

use eyre::{Context, Result};
use heed::{Database, Env, EnvOpenOptions, types::Bytes};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::{
    codec::{DatabaseKey, DatabaseValue},
    schema::TableType,
};

/// High-performance LMDB database for Arbitrum-Reth storage
///
/// This implementation provides:
/// - ACID transactions with excellent performance
/// - Memory-mapped access for zero-copy reads
/// - Multiple database tables in a single environment
/// - Async-compatible operations
#[derive(Debug)]
pub struct ArbitrumDatabase {
    /// LMDB environment containing all databases
    env: Arc<Env>,
    /// Individual database tables
    tables: Arc<RwLock<DatabaseTables>>,
}

/// Container for all database tables
#[derive(Debug)]
pub struct DatabaseTables {
    /// Block data indexed by number and hash
    blocks: Database<Bytes, Bytes>,
    /// Transaction data indexed by hash
    transactions: Database<Bytes, Bytes>,
    /// Account state indexed by address
    accounts: Database<Bytes, Bytes>,
    /// Contract storage indexed by (address, key)
    storage: Database<Bytes, Bytes>,
    /// Transaction receipts indexed by hash
    receipts: Database<Bytes, Bytes>,
    /// State trie nodes indexed by hash
    state_trie: Database<Bytes, Bytes>,
    /// Arbitrum batches indexed by number
    batches: Database<Bytes, Bytes>,
    /// L1 messages indexed by number
    l1_messages: Database<Bytes, Bytes>,
    /// Metadata and statistics
    metadata: Database<Bytes, Bytes>,
}

impl ArbitrumDatabase {
    /// Create a new database instance
    ///
    /// # Arguments
    /// * `data_dir` - Directory to store database files
    /// * `max_size` - Maximum database size in bytes
    ///
    /// # Example
    /// ```rust
    /// use arbitrum_storage::database::ArbitrumDatabase;
    ///
    /// let db = ArbitrumDatabase::new("./data", 10 * 1024 * 1024 * 1024).await?; // 10GB
    /// ```
    pub async fn new<P: AsRef<Path>>(data_dir: P, max_size: usize) -> Result<Self> {
        let db_path = data_dir.as_ref().join("lmdb");

        info!("Initializing LMDB database at: {}", db_path.display());

        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&db_path)
            .await
            .context("Failed to create database directory")?;

        // Create LMDB environment
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(max_size)
                .max_dbs(16) // Allow up to 16 databases
                .max_readers(1024) // Support many concurrent readers
                .open(db_path)
                .context("Failed to open LMDB environment")?
        };

        let env = Arc::new(env);

        // Initialize all database tables
        let tables = Self::initialize_tables(&env).await?;

        info!("LMDB database initialized successfully");

        Ok(Self {
            env,
            tables: Arc::new(RwLock::new(tables)),
        })
    }

    /// Initialize all database tables
    async fn initialize_tables(env: &Env) -> Result<DatabaseTables> {
        debug!("Initializing database tables");

        let mut wtxn = env
            .write_txn()
            .context("Failed to begin write transaction")?;

        let tables = DatabaseTables {
            blocks: env
                .create_database(&mut wtxn, Some("blocks"))
                .context("Failed to create blocks table")?,
            transactions: env
                .create_database(&mut wtxn, Some("transactions"))
                .context("Failed to create transactions table")?,
            accounts: env
                .create_database(&mut wtxn, Some("accounts"))
                .context("Failed to create accounts table")?,
            storage: env
                .create_database(&mut wtxn, Some("storage"))
                .context("Failed to create storage table")?,
            receipts: env
                .create_database(&mut wtxn, Some("receipts"))
                .context("Failed to create receipts table")?,
            state_trie: env
                .create_database(&mut wtxn, Some("state_trie"))
                .context("Failed to create state_trie table")?,
            batches: env
                .create_database(&mut wtxn, Some("batches"))
                .context("Failed to create batches table")?,
            l1_messages: env
                .create_database(&mut wtxn, Some("l1_messages"))
                .context("Failed to create l1_messages table")?,
            metadata: env
                .create_database(&mut wtxn, Some("metadata"))
                .context("Failed to create metadata table")?,
        };

        wtxn.commit().context("Failed to commit table creation")?;

        debug!("Database tables initialized successfully");
        Ok(tables)
    }

    /// Execute a read-only operation
    ///
    /// # Arguments
    /// * `operation` - Closure that performs the read operation
    ///
    /// # Example
    /// ```rust
    /// let value = db.read(|txn, tables| tables.blocks.get(txn, &key)).await?;
    /// ```
    pub async fn read<F, R>(&self, operation: F) -> Result<R>
    where
        F: FnOnce(&heed::RoTxn, &DatabaseTables) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let env = Arc::clone(&self.env);
        let tables = {
            let tables_guard = self.tables.read().await;
            DatabaseTables {
                blocks: tables_guard.blocks,
                transactions: tables_guard.transactions,
                accounts: tables_guard.accounts,
                storage: tables_guard.storage,
                receipts: tables_guard.receipts,
                state_trie: tables_guard.state_trie,
                batches: tables_guard.batches,
                l1_messages: tables_guard.l1_messages,
                metadata: tables_guard.metadata,
            }
        };

        // Execute in blocking task to avoid blocking async runtime
        tokio::task::spawn_blocking(move || {
            let rtxn = env.read_txn().context("Failed to begin read transaction")?;
            operation(&rtxn, &tables)
        })
        .await
        .context("Read operation was cancelled")?
    }

    /// Execute a read-write operation
    ///
    /// # Arguments
    /// * `operation` - Closure that performs the write operation
    ///
    /// # Example
    /// ```rust
    /// db.write(|txn, tables| tables.blocks.put(txn, &key, &value))
    ///     .await?;
    /// ```
    pub async fn write<F, R>(&self, operation: F) -> Result<R>
    where
        F: FnOnce(&mut heed::RwTxn, &DatabaseTables) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let env = Arc::clone(&self.env);
        let tables = {
            let tables_guard = self.tables.read().await;
            DatabaseTables {
                blocks: tables_guard.blocks,
                transactions: tables_guard.transactions,
                accounts: tables_guard.accounts,
                storage: tables_guard.storage,
                receipts: tables_guard.receipts,
                state_trie: tables_guard.state_trie,
                batches: tables_guard.batches,
                l1_messages: tables_guard.l1_messages,
                metadata: tables_guard.metadata,
            }
        };

        // Execute in blocking task to avoid blocking async runtime
        tokio::task::spawn_blocking(move || {
            let mut wtxn = env
                .write_txn()
                .context("Failed to begin write transaction")?;
            let result = operation(&mut wtxn, &tables)?;
            wtxn.commit().context("Failed to commit transaction")?;
            Ok(result)
        })
        .await
        .context("Write operation was cancelled")?
    }

    /// Get a value from a specific table
    pub async fn get<K, V>(&self, table: TableType, key: &K) -> Result<Option<V>>
    where
        K: DatabaseKey + Send + Sync,
        V: DatabaseValue + Send + Sync + 'static,
    {
        let key_bytes = key.encode()?;
        self.read(move |txn, tables| {
            let db = Self::get_table(tables, table);

            match db.get(txn, &key_bytes) {
                Ok(Some(value_bytes)) => {
                    let value = V::decode(value_bytes)?;
                    Ok(Some(value))
                }
                Ok(None) => Ok(None),
                Err(err) => Err(eyre::eyre!("Database get error: {}", err)),
            }
        })
        .await
    }

    /// Put a value into a specific table
    pub async fn put<K, V>(&self, table: TableType, key: &K, value: &V) -> Result<()>
    where
        K: DatabaseKey + Send + Sync,
        V: DatabaseValue + Send + Sync + 'static,
    {
        let key_bytes = key.encode()?;
        let value_bytes = value.encode()?;
        self.write(move |txn, tables| {
            let db = Self::get_table(tables, table);

            db.put(txn, &key_bytes, &value_bytes)
                .context("Failed to put value")?;

            Ok(())
        })
        .await
    }

    /// Delete a value from a specific table
    pub async fn delete<K>(&self, table: TableType, key: &K) -> Result<bool>
    where
        K: DatabaseKey + Send + Sync,
    {
        let key_bytes = key.encode()?;
        self.write(move |txn, tables| {
            let db = Self::get_table(tables, table);

            match db.delete(txn, &key_bytes) {
                Ok(true) => Ok(true),
                Ok(false) => Ok(false),
                Err(err) => Err(eyre::eyre!("Database delete error: {}", err)),
            }
        })
        .await
    }

    /// Get database statistics
    pub async fn stats(&self) -> Result<DatabaseStats> {
        self.read(|txn, tables| {
            let blocks_stat = tables.blocks.stat(txn)?;
            let transactions_stat = tables.transactions.stat(txn)?;
            let accounts_stat = tables.accounts.stat(txn)?;

            Ok(DatabaseStats {
                total_blocks: blocks_stat.entries,
                total_transactions: transactions_stat.entries,
                total_accounts: accounts_stat.entries,
                database_size: 0, // TODO: Get actual database size
            })
        })
        .await
    }

    /// Helper to get the correct database for a table type
    fn get_table(tables: &DatabaseTables, table: TableType) -> &Database<Bytes, Bytes> {
        match table {
            TableType::Blocks => &tables.blocks,
            TableType::Transactions => &tables.transactions,
            TableType::Accounts => &tables.accounts,
            TableType::Storage => &tables.storage,
            TableType::Receipts => &tables.receipts,
            TableType::StateTrie => &tables.state_trie,
            TableType::Batches => &tables.batches,
            TableType::L1Messages => &tables.l1_messages,
            TableType::Metadata => &tables.metadata,
        }
    }

    /// Sync database to disk
    pub async fn sync(&self) -> Result<()> {
        let env = Arc::clone(&self.env);
        tokio::task::spawn_blocking(move || env.force_sync().context("Failed to sync database"))
            .await
            .context("Sync operation was cancelled")?
    }

    /// Close the database
    pub async fn close(self) -> Result<()> {
        info!("Closing LMDB database");

        // Sync before closing
        self.sync().await?;

        // Drop the environment to close it
        drop(self.env);

        info!("LMDB database closed successfully");
        Ok(())
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_blocks: usize,
    pub total_transactions: usize,
    pub total_accounts: usize,
    pub database_size: usize,
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db = ArbitrumDatabase::new(temp_dir.path(), 1024 * 1024).await;
        assert!(db.is_ok());
    }

    #[tokio::test]
    async fn test_database_stats() {
        let temp_dir = TempDir::new().unwrap();
        let db = ArbitrumDatabase::new(temp_dir.path(), 1024 * 1024)
            .await
            .unwrap();

        let stats = db.stats().await.unwrap();
        assert_eq!(stats.total_blocks, 0);
        assert_eq!(stats.total_transactions, 0);
        assert_eq!(stats.total_accounts, 0);
    }
}

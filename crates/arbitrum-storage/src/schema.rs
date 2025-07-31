//! Database schema and table type definitions
//!
//! This module defines the database schema, table types, and key-value
//! layouts for efficient storage and retrieval of Arbitrum-Reth data.

/// Database table types for organized data storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableType {
    /// Block data indexed by number and hash
    Blocks,
    /// Transaction data indexed by hash
    Transactions,
    /// Account state indexed by address
    Accounts,
    /// Contract storage indexed by (address, key)
    Storage,
    /// Transaction receipts indexed by hash
    Receipts,
    /// State trie nodes indexed by hash
    StateTrie,
    /// Arbitrum batches indexed by number
    Batches,
    /// L1 messages indexed by number
    L1Messages,
    /// Metadata and chain information
    Metadata,
}

impl TableType {
    /// Get all table types
    pub fn all() -> &'static [TableType] {
        &[
            TableType::Blocks,
            TableType::Transactions,
            TableType::Accounts,
            TableType::Storage,
            TableType::Receipts,
            TableType::StateTrie,
            TableType::Batches,
            TableType::L1Messages,
            TableType::Metadata,
        ]
    }

    /// Get table name as string
    pub fn name(self) -> &'static str {
        match self {
            TableType::Blocks => "blocks",
            TableType::Transactions => "transactions",
            TableType::Accounts => "accounts",
            TableType::Storage => "storage",
            TableType::Receipts => "receipts",
            TableType::StateTrie => "state_trie",
            TableType::Batches => "batches",
            TableType::L1Messages => "l1_messages",
            TableType::Metadata => "metadata",
        }
    }
}

/// Database key types for different storage patterns
pub mod keys {
    use alloy_primitives::{Address, B256};
    use serde::{Deserialize, Serialize};

    /// Block number key (8 bytes, big-endian)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct BlockNumber(pub u64);

    /// Block hash key (32 bytes)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct BlockHash(pub B256);

    /// Transaction hash key (32 bytes)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TransactionHash(pub B256);

    /// Account address key (20 bytes)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct AccountAddress(pub Address);

    /// Storage key combining address and storage slot
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct StorageKey {
        pub address: Address,
        pub slot: B256,
    }

    /// State trie node hash key (32 bytes)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TrieNodeHash(pub B256);

    /// Batch number key (8 bytes, big-endian)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct BatchNumber(pub u64);

    /// L1 message number key (8 bytes, big-endian)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct L1MessageNumber(pub u64);

    /// Metadata key (string)
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct MetadataKey(pub String);

    // Implement From traits for easier usage
    impl From<u64> for BlockNumber {
        fn from(n: u64) -> Self {
            Self(n)
        }
    }

    impl From<B256> for BlockHash {
        fn from(hash: B256) -> Self {
            Self(hash)
        }
    }

    impl From<B256> for TransactionHash {
        fn from(hash: B256) -> Self {
            Self(hash)
        }
    }

    impl From<Address> for AccountAddress {
        fn from(address: Address) -> Self {
            Self(address)
        }
    }

    impl From<(Address, B256)> for StorageKey {
        fn from((address, slot): (Address, B256)) -> Self {
            Self { address, slot }
        }
    }

    impl From<B256> for TrieNodeHash {
        fn from(hash: B256) -> Self {
            Self(hash)
        }
    }

    impl From<u64> for BatchNumber {
        fn from(n: u64) -> Self {
            Self(n)
        }
    }

    impl From<u64> for L1MessageNumber {
        fn from(n: u64) -> Self {
            Self(n)
        }
    }

    impl From<String> for MetadataKey {
        fn from(key: String) -> Self {
            Self(key)
        }
    }

    impl From<&str> for MetadataKey {
        fn from(key: &str) -> Self {
            Self(key.to_string())
        }
    }
}

/// Common metadata keys used in the database
pub mod metadata_keys {
    /// Latest block number
    pub const LATEST_BLOCK_NUMBER: &str = "latest_block_number";
    /// Latest batch number
    pub const LATEST_BATCH_NUMBER: &str = "latest_batch_number";
    /// Latest L1 message number
    pub const LATEST_L1_MESSAGE_NUMBER: &str = "latest_l1_message_number";
    /// Chain genesis block hash
    pub const GENESIS_BLOCK_HASH: &str = "genesis_block_hash";
    /// Database schema version
    pub const SCHEMA_VERSION: &str = "schema_version";
    /// Node sync status
    pub const SYNC_STATUS: &str = "sync_status";
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, b256};

    use super::*;

    #[test]
    fn test_table_types() {
        let all_tables = TableType::all();
        assert_eq!(all_tables.len(), 9);

        assert_eq!(TableType::Blocks.name(), "blocks");
        assert_eq!(TableType::Transactions.name(), "transactions");
        assert_eq!(TableType::Accounts.name(), "accounts");
    }

    #[test]
    fn test_key_conversions() {
        let block_num = keys::BlockNumber::from(42u64);
        assert_eq!(block_num.0, 42);

        let addr = address!("0x1234567890123456789012345678901234567890");
        let account_key = keys::AccountAddress::from(addr);
        assert_eq!(account_key.0, addr);

        let hash = b256!("0x1234567890123456789012345678901234567890123456789012345678901234");
        let storage_key = keys::StorageKey::from((addr, hash));
        assert_eq!(storage_key.address, addr);
        assert_eq!(storage_key.slot, hash);
    }

    #[test]
    fn test_metadata_keys() {
        assert_eq!(metadata_keys::LATEST_BLOCK_NUMBER, "latest_block_number");
        assert_eq!(metadata_keys::GENESIS_BLOCK_HASH, "genesis_block_hash");
        assert_eq!(metadata_keys::SCHEMA_VERSION, "schema_version");
    }
}

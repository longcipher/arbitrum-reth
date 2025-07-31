//! Serialization and deserialization codecs for database storage
//!
//! This module provides efficient encoding and decoding of Rust data structures
//! for storage in the LMDB database. It supports multiple encoding formats
//! optimized for different types of data.

use alloy_primitives::{Address, B256, U256};
use bincode;
use eyre::{Context, Result};
use rlp::{Decodable, Encodable};
use serde::{Deserialize, Serialize};

use crate::schema::keys;

/// Trait for types that can be used as database keys
pub trait DatabaseKey: Send + Sync {
    /// Encode the key into bytes for database storage
    fn encode(&self) -> Result<Vec<u8>>;
}

/// Trait for types that can be used as database values
pub trait DatabaseValue: Sized + Send + Sync {
    /// Encode the value into bytes for database storage
    fn encode(&self) -> Result<Vec<u8>>;

    /// Decode the value from bytes retrieved from database
    fn decode(bytes: &[u8]) -> Result<Self>;
}

/// Encoding format for database values
#[derive(Debug, Clone, Copy)]
pub enum EncodingFormat {
    /// Bincode encoding (fast, compact)
    Bincode,
    /// RLP encoding (Ethereum compatible)
    Rlp,
    /// Raw bytes (no encoding)
    Raw,
}

// Implement DatabaseKey for all key types

impl DatabaseKey for keys::BlockNumber {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_be_bytes().to_vec())
    }
}

impl DatabaseKey for keys::BlockHash {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.as_slice().to_vec())
    }
}

impl DatabaseKey for keys::TransactionHash {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.as_slice().to_vec())
    }
}

impl DatabaseKey for keys::AccountAddress {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.as_slice().to_vec())
    }
}

impl DatabaseKey for keys::StorageKey {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(52); // 20 bytes address + 32 bytes slot
        bytes.extend_from_slice(self.address.as_slice());
        bytes.extend_from_slice(self.slot.as_slice());
        Ok(bytes)
    }
}

impl DatabaseKey for keys::BatchNumber {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_be_bytes().to_vec())
    }
}

impl DatabaseKey for keys::L1MessageNumber {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_be_bytes().to_vec())
    }
}

impl DatabaseKey for keys::MetadataKey {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.0.as_bytes().to_vec())
    }
}

// Implement DatabaseValue for primitive types

impl DatabaseValue for u64 {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 8 {
            return Err(eyre::eyre!(
                "Invalid u64 length: expected 8, got {}",
                bytes.len()
            ));
        }
        let array: [u8; 8] = bytes
            .try_into()
            .map_err(|_| eyre::eyre!("Failed to convert bytes to u64 array"))?;
        Ok(u64::from_be_bytes(array))
    }
}

impl DatabaseValue for u32 {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 4 {
            return Err(eyre::eyre!(
                "Invalid u32 length: expected 4, got {}",
                bytes.len()
            ));
        }
        let array: [u8; 4] = bytes
            .try_into()
            .map_err(|_| eyre::eyre!("Failed to convert bytes to u32 array"))?;
        Ok(u32::from_be_bytes(array))
    }
}

impl DatabaseValue for Vec<u8> {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.clone())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(bytes.to_vec())
    }
}

impl DatabaseValue for String {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.as_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        String::from_utf8(bytes.to_vec()).context("Failed to decode UTF-8 string")
    }
}

// Implement DatabaseValue for alloy types

impl DatabaseValue for Address {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.as_slice().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 20 {
            return Err(eyre::eyre!(
                "Invalid address length: expected 20, got {}",
                bytes.len()
            ));
        }
        let array: [u8; 20] = bytes
            .try_into()
            .map_err(|_| eyre::eyre!("Failed to convert bytes to address array"))?;
        Ok(Address::from(array))
    }
}

impl DatabaseValue for B256 {
    fn encode(&self) -> Result<Vec<u8>> {
        Ok(self.as_slice().to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(eyre::eyre!(
                "Invalid B256 length: expected 32, got {}",
                bytes.len()
            ));
        }
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| eyre::eyre!("Failed to convert bytes to B256 array"))?;
        Ok(B256::from(array))
    }
}

impl DatabaseValue for U256 {
    fn encode(&self) -> Result<Vec<u8>> {
        let bytes = self.to_be_bytes::<32>();
        Ok(bytes.to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(eyre::eyre!(
                "Invalid U256 length: expected 32, got {}",
                bytes.len()
            ));
        }
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| eyre::eyre!("Failed to convert bytes to U256 array"))?;
        Ok(U256::from_be_bytes(array))
    }
}

// Generic encoding traits for extensibility

/// Trait for custom encoding/decoding strategies
pub trait Encoder<T> {
    /// Encode value to bytes
    fn encode(value: &T) -> Result<Vec<u8>>;
    /// Decode value from bytes
    fn decode(bytes: &[u8]) -> Result<T>;
}

/// Bincode encoder for serde-compatible types
pub struct BincodeEncoder;

impl<T> Encoder<T> for BincodeEncoder
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    fn encode(value: &T) -> Result<Vec<u8>> {
        bincode::serialize(value).context("Bincode encoding failed")
    }

    fn decode(bytes: &[u8]) -> Result<T> {
        bincode::deserialize(bytes).context("Bincode decoding failed")
    }
}

/// RLP encoder for Ethereum-compatible types
pub struct RlpEncoder;

impl<T> Encoder<T> for RlpEncoder
where
    T: Encodable + Decodable,
{
    fn encode(value: &T) -> Result<Vec<u8>> {
        Ok(rlp::encode(value).to_vec())
    }

    fn decode(bytes: &[u8]) -> Result<T> {
        T::decode(&rlp::Rlp::new(bytes)).map_err(|e| eyre::eyre!("RLP decoding failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_number_key() {
        let block_num = keys::BlockNumber(12345);
        let encoded = DatabaseKey::encode(&block_num).unwrap();
        assert_eq!(encoded, 12345u64.to_be_bytes().to_vec());
    }

    #[test]
    fn test_block_hash_key() {
        let block_hash = keys::BlockHash(B256::ZERO);
        let encoded = DatabaseKey::encode(&block_hash).unwrap();
        assert_eq!(encoded, vec![0u8; 32]);
    }

    #[test]
    fn test_storage_key() {
        let storage_key = keys::StorageKey {
            address: Address::ZERO,
            slot: B256::ZERO,
        };
        let encoded = DatabaseKey::encode(&storage_key).unwrap();
        assert_eq!(encoded.len(), 52); // 20 + 32 bytes
    }

    #[test]
    fn test_u64_value() {
        let value = 42u64;
        let encoded = DatabaseValue::encode(&value).unwrap();
        let decoded: u64 = DatabaseValue::decode(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_string_value() {
        let value = "test".to_string();
        let encoded = DatabaseValue::encode(&value).unwrap();
        let decoded: String = DatabaseValue::decode(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_address_value() {
        let address = Address::ZERO;
        let encoded = DatabaseValue::encode(&address).unwrap();
        let decoded: Address = DatabaseValue::decode(&encoded).unwrap();
        assert_eq!(address, decoded);
    }

    #[test]
    fn test_u256_value() {
        let value = U256::from(42);
        let encoded = DatabaseValue::encode(&value).unwrap();
        let decoded: U256 = DatabaseValue::decode(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_metadata_key() {
        let key = keys::MetadataKey("latest_block".to_string());
        let encoded = DatabaseKey::encode(&key).unwrap();
        assert_eq!(encoded, "latest_block".as_bytes().to_vec());
    }
}

/// Arbitrum-specific data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrumBlock {
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub transactions: Vec<B256>,
    pub l1_block_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrumTransaction {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas: u64,
    pub gas_price: U256,
    pub nonce: u64,
    pub data: Vec<u8>,
    pub l1_sequence_number: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrumAccount {
    pub address: Address,
    pub balance: U256,
    pub nonce: u64,
    pub code_hash: B256,
    pub storage_root: B256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrumBatch {
    pub batch_number: u64,
    pub block_range: (u64, u64),
    pub l1_block_number: u64,
    pub timestamp: u64,
    pub transactions: Vec<B256>,
    pub l1_tx_hash: Option<B256>, // Hash of the L1 transaction that submitted this batch
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Message {
    pub message_number: u64,
    pub sender: Address,
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub block_number: u64,
}

// DatabaseValue implementations for Arbitrum types
impl DatabaseValue for ArbitrumBlock {
    fn encode(&self) -> Result<Vec<u8>> {
        let encoded = bincode::serialize(self).context("Failed to serialize ArbitrumBlock")?;
        Ok(encoded)
    }

    fn decode(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).context("Failed to deserialize ArbitrumBlock")
    }
}

impl DatabaseValue for ArbitrumTransaction {
    fn encode(&self) -> Result<Vec<u8>> {
        let encoded =
            bincode::serialize(self).context("Failed to serialize ArbitrumTransaction")?;
        Ok(encoded)
    }

    fn decode(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).context("Failed to deserialize ArbitrumTransaction")
    }
}

impl DatabaseValue for ArbitrumAccount {
    fn encode(&self) -> Result<Vec<u8>> {
        let encoded = bincode::serialize(self).context("Failed to serialize ArbitrumAccount")?;
        Ok(encoded)
    }

    fn decode(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).context("Failed to deserialize ArbitrumAccount")
    }
}

impl DatabaseValue for ArbitrumBatch {
    fn encode(&self) -> Result<Vec<u8>> {
        let encoded = bincode::serialize(self).context("Failed to serialize ArbitrumBatch")?;
        Ok(encoded)
    }

    fn decode(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).context("Failed to deserialize ArbitrumBatch")
    }
}

impl DatabaseValue for L1Message {
    fn encode(&self) -> Result<Vec<u8>> {
        let encoded = bincode::serialize(self).context("Failed to serialize L1Message")?;
        Ok(encoded)
    }

    fn decode(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).context("Failed to deserialize L1Message")
    }
}

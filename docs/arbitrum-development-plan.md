# Arbitrum-Reth Development Plan: Technical Architecture

> Note: This document is kept for historical context and may be out of date.
> For the current, canonical plan and gap analysis, see: `design-implementation-parity-plan.md`.

## Executive Summary

This document outlines a comprehensive plan to develop an Arbitrum-compatible Layer 2 node implementation using Reth as the foundation. The goal is to achieve 100% protocol compatibility with Arbitrum Nitro while leveraging Reth's high-performance execution engine to deliver 10x performance improvements.

## Key Arbitrum Differentiators from Ethereum

Based on the official Arbitrum documentation, the main differences we need to implement:

### 1. **Two-Dimensional Gas Model**
- **L2 Gas**: Standard Ethereum-like gas for computation
- **L1 Data Fee**: Additional fee for posting transaction data to L1
- **Dynamic Pricing**: Both components adjust dynamically based on demand

### 2. **Cross-Chain Messaging**
- **L1→L2 Messages**: Parent chain to child chain messaging
- **L2→L1 Messages**: Child chain to parent chain messaging via outbox
- **Retryable Tickets**: Reliable L1→L2 transaction execution

### 3. **Arbitrum-Specific Precompiles**
- **ArbOS System Contracts**: Native L2 functionality
- **ArbGasInfo**: Gas pricing information
- **ArbSys**: System information and L2→L1 messaging
- **ArbRetryableTx**: Retryable transaction management

### 4. **NodeInterface Contract**
- **Special RPC-only contract** at address `0xc8`
- **Not deployed on-chain** but accessible via RPC
- **Gas estimation and debugging** utilities

### 5. **Block Timing and Sequencing**
- **Deterministic block production** rather than PoW/PoS mining
- **Sequencer-based ordering** with L1 finality
- **Custom block time handling**

## Technical Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Arbitrum-Reth Node                          │
├─────────────────────────────────────────────────────────────────┤
│                     RPC Layer                                   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
│  │   Standard ETH  │ │  Arbitrum ARB   │ │  NodeInterface  │   │
│  │   RPC Methods   │ │   Extensions    │ │   Contract      │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                   Transaction Pool                              │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
│  │   L2 Gas Pool   │ │  L1 Data Pricer │ │ Retryable Pool  │   │
│  │   Management    │ │   & Estimator   │ │   Management    │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                    Execution Engine                             │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              Reth EVM + Arbitrum Extensions                 │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │ │
│  │  │    Base     │ │   Arbitrum  │ │    Precompile       │   │ │
│  │  │    EVM      │ │  Precompiles│ │     Extensions      │   │ │
│  │  └─────────────┘ └─────────────┘ └─────────────────────┘   │ │
│  └─────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                   Consensus Layer                               │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
│  │   Sequencer     │ │   L1 Finality   │ │   State Root    │   │
│  │   Logic         │ │   Tracking      │ │   Management    │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                   Storage Layer                                 │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                 Reth MDBX Storage                           │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │ │
│  │  │   L2 State  │ │  L1 Message │ │    Batch Data       │   │ │
│  │  │   Storage   │ │   Tracking  │ │     Storage         │   │ │
│  │  └─────────────┘ └─────────────┘ └─────────────────────┘   │ │
│  └─────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    Network Layer                                │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐   │
│  │      L2 P2P     │ │  L1 Connection  │ │   Batch Poster  │   │
│  │   Networking    │ │    Manager      │ │     Client      │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Core Arbitrum Execution Engine (Months 1-2)

#### 1.1 Arbitrum Precompiles Implementation
```rust
// File: crates/arbitrum-precompiles/src/lib.rs

pub mod arbos;
pub mod arbsys;
pub mod arbgas;
pub mod arbretryable;
pub mod nodeinterface;

// Arbitrum precompile addresses
pub const ARBSYS_ADDRESS: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100]); // 0x64
pub const ARBGAS_ADDRESS: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108]); // 0x6c
pub const NODEINTERFACE_ADDRESS: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200]); // 0xc8
```

#### 1.2 Two-Dimensional Gas System
```rust
// File: crates/arbitrum-gas/src/gas_model.rs

#[derive(Debug, Clone)]
pub struct ArbitrumGasCalculation {
    pub l2_gas_used: u64,
    pub l1_data_fee: u64,
    pub l1_data_gas: u64,
    pub total_gas: u64,
}

pub struct ArbitrumGasPricer {
    l1_pricer: L1DataPricer,
    l2_pricer: L2GasPricer,
}

impl ArbitrumGasPricer {
    pub fn calculate_gas(&self, tx: &Transaction) -> Result<ArbitrumGasCalculation> {
        let l2_gas = self.l2_pricer.estimate_l2_gas(tx)?;
        let (l1_data_fee, l1_data_gas) = self.l1_pricer.calculate_l1_component(tx)?;
        
        Ok(ArbitrumGasCalculation {
            l2_gas_used: l2_gas,
            l1_data_fee,
            l1_data_gas,
            total_gas: l2_gas + l1_data_gas,
        })
    }
}
```

#### 1.3 EVM Extensions for Arbitrum
```rust
// File: crates/arbitrum-evm/src/lib.rs

use reth_revm::{Database, Evm, EvmBuilder};

pub struct ArbitrumEvm<DB> {
    inner: Evm<'static, (), DB>,
    arbitrum_context: ArbitrumExecutionContext,
}

pub struct ArbitrumExecutionContext {
    pub l1_block_number: u64,
    pub l1_timestamp: u64,
    pub sequencer_inbox: H160,
    pub batch_poster: H160,
}

impl<DB: Database> ArbitrumEvm<DB> {
    pub fn new(database: DB, context: ArbitrumExecutionContext) -> Self {
        let evm = EvmBuilder::default()
            .with_db(database)
            .with_precompiles(arbitrum_precompiles())
            .build();
            
        Self {
            inner: evm,
            arbitrum_context: context,
        }
    }
}

fn arbitrum_precompiles() -> BTreeMap<Address, Precompile> {
    let mut precompiles = BTreeMap::new();
    
    // Add standard Ethereum precompiles
    precompiles.extend(reth_revm::primitives::precompiles::Precompiles::default().precompiles());
    
    // Add Arbitrum-specific precompiles
    precompiles.insert(ARBSYS_ADDRESS, Precompile::Standard(arbsys_precompile));
    precompiles.insert(ARBGAS_ADDRESS, Precompile::Standard(arbgas_precompile));
    precompiles.insert(NODEINTERFACE_ADDRESS, Precompile::Standard(nodeinterface_precompile));
    
    precompiles
}
```

### Phase 2: Transaction Pool and Gas Management (Months 2-3)

#### 2.1 Arbitrum Transaction Pool
```rust
// File: crates/arbitrum-pool/src/arbitrum_pool.rs

use reth_transaction_pool::{PoolTransaction, TransactionPool};

pub struct ArbitrumTransactionPool {
    inner: Box<dyn TransactionPool<Transaction = ArbitrumPooledTransaction>>,
    gas_pricer: Arc<ArbitrumGasPricer>,
    l1_data_pricer: Arc<L1DataPricer>,
}

#[derive(Debug, Clone)]
pub struct ArbitrumPooledTransaction {
    pub inner: PooledTransaction,
    pub l1_data_fee: u64,
    pub l2_gas_price: u64,
    pub total_fee: u256,
}

impl ArbitrumTransactionPool {
    pub async fn add_transaction(&self, tx: Transaction) -> Result<TxHash> {
        // Calculate both L1 and L2 components
        let gas_calc = self.gas_pricer.calculate_gas(&tx)?;
        
        // Validate transaction can pay for both components
        self.validate_arbitrum_transaction(&tx, &gas_calc)?;
        
        // Create pooled transaction with Arbitrum-specific data
        let pooled_tx = ArbitrumPooledTransaction {
            inner: PooledTransaction::new(tx.clone(), TxHash::default()),
            l1_data_fee: gas_calc.l1_data_fee,
            l2_gas_price: gas_calc.l2_gas_used,
            total_fee: U256::from(gas_calc.total_gas),
        };
        
        self.inner.add_transaction(ValidationOutcome::Valid(pooled_tx)).await
    }
}
```

#### 2.2 L1 Data Pricing Component
```rust
// File: crates/arbitrum-gas/src/l1_pricing.rs

pub struct L1DataPricer {
    l1_gas_price: AtomicU64,
    compression_ratio: f64,
    funds_pool: Arc<RwLock<L1PricerFundsPool>>,
}

impl L1DataPricer {
    pub fn calculate_l1_component(&self, tx: &Transaction) -> Result<(u64, u64)> {
        // Estimate compressed size using brotli-zero algorithm
        let compressed_size = self.estimate_compressed_size(&tx.data)?;
        
        // Calculate L1 gas cost (16 gas per byte on L1)
        let l1_gas_cost = compressed_size * 16;
        
        // Get current L1 gas price
        let l1_gas_price = self.l1_gas_price.load(Ordering::Relaxed);
        
        // Calculate L1 data fee in wei
        let l1_data_fee = l1_gas_cost * l1_gas_price;
        
        // Convert to L2 gas units
        let l2_gas_price = self.get_l2_gas_price();
        let l1_data_gas = l1_data_fee / l2_gas_price;
        
        Ok((l1_data_fee, l1_data_gas))
    }
    
    fn estimate_compressed_size(&self, data: &[u8]) -> Result<u64> {
        // Implement brotli-zero compression estimation
        // This is a simplified version - real implementation would use brotli
        let mut compressed = Vec::new();
        brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22)
            .write_all(data)?;
        Ok(compressed.len() as u64)
    }
}
```

### Phase 3: Cross-Chain Messaging System (Months 3-4)

#### 3.1 L1→L2 Message Processing
```rust
// File: crates/arbitrum-messaging/src/l1_to_l2.rs

pub struct L1ToL2MessageProcessor {
    delayed_inbox: Arc<DelayedInbox>,
    sequencer_inbox: Arc<SequencerInbox>,
    retryable_manager: Arc<RetryableTicketManager>,
}

#[derive(Debug, Clone)]
pub struct RetryableTicket {
    pub from: H160,
    pub to: H160,
    pub l2_call_value: U256,
    pub value_to_deposit: U256,
    pub max_submission_fee: U256,
    pub excess_fee_refund_address: H160,
    pub call_value_refund_address: H160,
    pub gas_limit: u64,
    pub max_fee_per_gas: U256,
    pub data: Vec<u8>,
}

impl L1ToL2MessageProcessor {
    pub async fn process_l1_message(&self, message: L1Message) -> Result<()> {
        match message.message_type {
            L1MessageType::SubmitRetryable => {
                let ticket = self.decode_retryable_ticket(&message.data)?;
                self.retryable_manager.create_retryable(ticket).await?;
            }
            L1MessageType::EthDeposit => {
                self.process_eth_deposit(&message).await?;
            }
            L1MessageType::DirectCall => {
                self.process_direct_call(&message).await?;
            }
        }
        Ok(())
    }
}
```

#### 3.2 L2→L1 Message System
```rust
// File: crates/arbitrum-messaging/src/l2_to_l1.rs

pub struct L2ToL1MessageProcessor {
    outbox: Arc<Outbox>,
    merkle_accumulator: Arc<MerkleAccumulator>,
}

#[derive(Debug, Clone)]
pub struct L2ToL1Message {
    pub sender: H160,
    pub to: H160,
    pub l2_block: u64,
    pub l1_block: u64,
    pub l2_timestamp: u64,
    pub value: U256,
    pub data: Vec<u8>,
}

impl L2ToL1MessageProcessor {
    pub async fn send_l2_to_l1_message(&self, message: L2ToL1Message) -> Result<H256> {
        // Add to merkle accumulator
        let message_hash = self.hash_message(&message);
        let merkle_path = self.merkle_accumulator.add_leaf(message_hash).await?;
        
        // Store in outbox for later execution on L1
        self.outbox.store_message(message, merkle_path).await?;
        
        Ok(message_hash)
    }
}
```

### Phase 4: Sequencer and Consensus Integration (Months 4-5)

#### 4.1 Arbitrum Sequencer Logic
```rust
// File: crates/arbitrum-consensus/src/sequencer.rs

pub struct ArbitrumSequencer {
    transaction_pool: Arc<ArbitrumTransactionPool>,
    block_builder: Arc<ArbitrumBlockBuilder>,
    l1_tracker: Arc<L1FinalityTracker>,
    batch_poster: Arc<BatchPoster>,
}

impl ArbitrumSequencer {
    pub async fn produce_block(&self) -> Result<ArbitrumBlock> {
        // Get transactions from pool (FIFO order, no tips)
        let pending_txs = self.transaction_pool.get_pending_transactions().await?;
        
        // Build block with L1 finality constraints
        let l1_info = self.l1_tracker.get_current_l1_info().await?;
        let block = self.block_builder.build_block(pending_txs, l1_info).await?;
        
        // Submit to batch poster for eventual L1 posting
        self.batch_poster.add_block_to_batch(block.clone()).await?;
        
        Ok(block)
    }
}

pub struct ArbitrumBlockBuilder {
    gas_limit: u64,
    l1_gas_oracle: Arc<L1GasOracle>,
}

impl ArbitrumBlockBuilder {
    pub async fn build_block(
        &self,
        transactions: Vec<ArbitrumPooledTransaction>,
        l1_info: L1Info,
    ) -> Result<ArbitrumBlock> {
        let mut block = ArbitrumBlock::new(l1_info);
        let mut gas_used = 0u64;
        
        for tx in transactions {
            if gas_used + tx.total_fee.as_u64() > self.gas_limit {
                break;
            }
            
            // Execute transaction with Arbitrum-specific logic
            let receipt = self.execute_arbitrum_transaction(&tx, &mut block).await?;
            block.add_transaction(tx.inner.transaction, receipt);
            gas_used += receipt.gas_used;
        }
        
        Ok(block)
    }
}
```

#### 4.2 Batch Posting and L1 Integration
```rust
// File: crates/arbitrum-batch-submitter/src/batch_poster.rs

pub struct BatchPoster {
    l1_client: Arc<EthereumClient>,
    sequencer_inbox_contract: H160,
    compression_algorithm: CompressionAlgorithm,
    posting_strategy: PostingStrategy,
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub blocks: Vec<ArbitrumBlock>,
    pub compressed_data: Vec<u8>,
    pub l1_block_number: u64,
    pub timestamp: u64,
}

impl BatchPoster {
    pub async fn post_batch(&self, batch: Batch) -> Result<H256> {
        // Compress batch data
        let compressed_data = self.compress_batch_data(&batch)?;
        
        // Estimate L1 gas costs
        let gas_estimate = self.l1_client
            .estimate_gas_for_batch_submission(&compressed_data)
            .await?;
        
        // Submit to L1 sequencer inbox
        let tx_hash = self.l1_client
            .submit_batch(
                self.sequencer_inbox_contract,
                compressed_data,
                gas_estimate,
            )
            .await?;
        
        info!("Batch posted to L1: {:?}", tx_hash);
        Ok(tx_hash)
    }
    
    fn compress_batch_data(&self, batch: &Batch) -> Result<Vec<u8>> {
        match self.compression_algorithm {
            CompressionAlgorithm::Brotli => {
                let serialized = bincode::serialize(&batch.blocks)?;
                let mut compressed = Vec::new();
                brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22)
                    .write_all(&serialized)?;
                Ok(compressed)
            }
        }
    }
}
```

### Phase 5: RPC API Extensions (Months 5-6)

#### 5.1 Arbitrum-Specific RPC Methods
```rust
// File: crates/arbitrum-rpc/src/arbitrum_api.rs

#[derive(Debug)]
pub struct ArbitrumRpcApi {
    node: Arc<ArbitrumNode>,
    gas_estimator: Arc<ArbitrumGasEstimator>,
    l1_tracker: Arc<L1FinalityTracker>,
}

impl ArbitrumRpcApi {
    // arb_getL1GasPrice
    pub async fn get_l1_gas_price(&self) -> RpcResult<U256> {
        let price = self.gas_estimator.get_current_l1_gas_price().await
            .map_err(|e| RpcError::internal_error(e.to_string()))?;
        Ok(U256::from(price))
    }
    
    // arb_estimateComponents
    pub async fn estimate_components(&self, tx: CallRequest) -> RpcResult<GasEstimateComponents> {
        let transaction = self.build_transaction_from_request(tx)?;
        let components = self.gas_estimator.estimate_components(&transaction).await
            .map_err(|e| RpcError::internal_error(e.to_string()))?;
        
        Ok(GasEstimateComponents {
            l1_gas_used: U256::from(components.l1_data_gas),
            l1_gas_price: U256::from(components.l1_gas_price),
            l2_gas_used: U256::from(components.l2_gas_used),
            l2_gas_price: U256::from(components.l2_gas_price),
        })
    }
    
    // arb_getL1BlockNumber
    pub async fn get_l1_block_number(&self) -> RpcResult<U64> {
        let block_number = self.l1_tracker.get_latest_l1_block().await
            .map_err(|e| RpcError::internal_error(e.to_string()))?;
        Ok(U64::from(block_number))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GasEstimateComponents {
    pub l1_gas_used: U256,
    pub l1_gas_price: U256,
    pub l2_gas_used: U256,
    pub l2_gas_price: U256,
}
```

#### 5.2 NodeInterface Implementation
```rust
// File: crates/arbitrum-rpc/src/node_interface.rs

pub struct NodeInterface {
    node: Arc<ArbitrumNode>,
    estimator: Arc<ArbitrumGasEstimator>,
}

impl NodeInterface {
    // NodeInterface.gasEstimateComponents()
    pub async fn gas_estimate_components(
        &self,
        to: H160,
        is_contract_creation: bool,
        data: Vec<u8>,
    ) -> Result<NodeInterfaceGasEstimate> {
        let tx = Transaction {
            to: Some(to),
            data,
            ..Default::default()
        };
        
        let estimate = self.estimator.estimate_components(&tx).await?;
        
        Ok(NodeInterfaceGasEstimate {
            gas_estimate: estimate.l2_gas_used + estimate.l1_data_gas,
            gas_estimate_for_l1: estimate.l1_data_gas,
            base_fee: estimate.l2_gas_price,
            l1_base_fee_estimate: estimate.l1_gas_price,
        })
    }
    
    // NodeInterface.gasEstimateL1Component()
    pub async fn gas_estimate_l1_component(
        &self,
        to: H160,
        is_contract_creation: bool,
        data: Vec<u8>,
    ) -> Result<u64> {
        let tx = Transaction {
            to: Some(to),
            data,
            ..Default::default()
        };
        
        let estimate = self.estimator.estimate_l1_component(&tx).await?;
        Ok(estimate.l1_data_gas)
    }
}
```

## Development Roadmap and Milestones

### Month 1-2: Foundation
- [ ] Set up Reth SDK integration framework
- [ ] Implement basic Arbitrum precompiles (ArbSys, ArbGasInfo)
- [ ] Create two-dimensional gas calculation system
- [ ] Build EVM extensions for Arbitrum-specific functionality

### Month 3-4: Core Features
- [ ] Implement Arbitrum transaction pool with L1/L2 gas handling
- [ ] Build L1 data pricing and compression system
- [ ] Create cross-chain messaging infrastructure
- [ ] Implement retryable ticket system

### Month 5-6: Advanced Features
- [ ] Build sequencer logic and block production
- [ ] Implement batch posting to L1
- [ ] Create Arbitrum-specific RPC API extensions
- [ ] Implement NodeInterface contract

### Month 7-8: Integration & Testing
- [ ] Complete L1 finality tracking system
- [ ] Implement comprehensive testing suite
- [ ] Performance optimization and benchmarking
- [ ] Protocol compatibility validation

### Month 9-10: Production Readiness
- [ ] Security audits and testing
- [ ] Mainnet compatibility testing
- [ ] Documentation and deployment guides
- [ ] Performance tuning for 10x targets

## Success Metrics

### Performance Targets
- **Transaction Throughput**: >2000 TPS (vs Nitro ~400 TPS)
- **Block Production**: <250ms average block time
- **RPC Response Time**: <50ms P95 for standard queries
- **Memory Usage**: <4GB for typical workloads
- **Sync Speed**: >3x faster than Nitro for historical sync

### Compatibility Targets
- **RPC Compatibility**: 100% compatibility with Arbitrum Nitro RPC API
- **Protocol Compatibility**: Full compatibility with Arbitrum protocol
- **Contract Compatibility**: 100% smart contract compatibility
- **Tool Compatibility**: Full compatibility with existing Arbitrum tooling

## Risk Assessment and Mitigation

### Technical Risks
1. **Reth SDK Breaking Changes**: Mitigation through close upstream collaboration
2. **Arbitrum Protocol Changes**: Mitigation through continuous protocol monitoring
3. **Performance Bottlenecks**: Mitigation through extensive profiling and optimization

### Compatibility Risks
1. **Subtle Protocol Differences**: Mitigation through comprehensive test suites
2. **Gas Model Complexity**: Mitigation through reference implementation comparison
3. **Cross-chain Message Edge Cases**: Mitigation through formal verification

## Next Steps

1. **Immediate (Week 1-2)**:
   - Set up development environment with latest Reth SDK
   - Create project structure following Reth patterns
   - Implement basic Arbitrum precompile framework

2. **Short-term (Month 1)**:
   - Complete gas model implementation
   - Build initial EVM extensions
   - Create comprehensive test framework

3. **Medium-term (Months 2-3)**:
   - Implement transaction pool with Arbitrum features
   - Build cross-chain messaging foundation
   - Create sequencer and consensus logic

This plan provides a comprehensive roadmap for building a high-performance, fully compatible Arbitrum implementation on top of Reth, leveraging Reth's performance advantages while maintaining 100% protocol compatibility with Arbitrum Nitro.

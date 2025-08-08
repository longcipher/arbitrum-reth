# Arbitrum-Reth Reth SDK Integration Guide

Note: This guide focuses on patterns and wiring. For hands-on snippets and Cargo setup, see also: reth-sdk-usage.md.

This document describes how to integrate Arbitrum-Reth with the official Reth SDK to implement a high-performance L2 node.

## Overview

Arbitrum-Reth now uses the Reth SDK NodeBuilder pattern to build custom Arbitrum nodes. This architecture provides:

- **Modular Design**: Customizable network, transaction pool, consensus, and EVM components
- **High Performance**: Based on Reth's highly optimized implementation
- **Developer Friendly**: Easy to extend and maintain

## Reth SDK Architecture

### Core Concepts

According to the [official Reth SDK documentation](https://reth.rs/sdk/overview), Reth SDK provides the following core functionality:

1. **NodeBuilder Pattern**: Use builder pattern to configure and launch nodes
2. **Component Customization**: Customize various components of the node
3. **Type System**: Use Ethereum-specific node types

### Basic Usage

```rust
use reth_ethereum::EthereumNode;
use reth_node_builder::NodeBuilder;

// Build custom node
let node = NodeBuilder::new(config)
    // Install Ethereum-specific node primitives
    .with_types::<EthereumNode>()
    .with_components(|components| {
        // Customize components here
        components
    })
    .build()
    .await?;
```

## Arbitrum-Reth Implementation

### 1. Dependency Configuration

Add Reth SDK dependencies in `Cargo.toml`:

```toml
[dependencies]
# Reth SDK
reth-ethereum = { git = "https://github.com/paradigmxyz/reth" }
reth-node-builder = { git = "https://github.com/paradigmxyz/reth" }
reth-tasks = { git = "https://github.com/paradigmxyz/reth" }
reth-primitives = { git = "https://github.com/paradigmxyz/reth" }
reth-provider = { git = "https://github.com/paradigmxyz/reth" }
reth-chainspec = { git = "https://github.com/paradigmxyz/reth" }
```

### 2. Node Structure

```rust
pub struct ArbitrumRethNode {
    config: ArbitrumRethConfig,
    // Arbitrum-specific components
    consensus: Arc<ArbitrumConsensus>,
    tx_pool: Arc<ArbitrumTransactionPool>,
    storage: Arc<ArbitrumStorage>,
    batch_submitter: Option<Arc<BatchSubmitter>>,
    inbox_tracker: Option<Arc<InboxTracker>>,
    validator: Option<Arc<Validator>>,
    // Reth SDK node handle
    node_handle: Option<NodeHandle>,
}
```

### 3. Node Startup

```rust
pub async fn start(&mut self) -> Result<()> {
    // Create Arbitrum node configuration
    let node_config = self.create_arbitrum_node_config().await?;

    // Build node using Reth SDK NodeBuilder pattern
    let node_handle = NodeBuilder::new(node_config)
        .with_types::<EthereumNode>()
        .with_components(EthereumNode::components())
        .launch()
        .await?;

    // Start Arbitrum-specific components
    self.start_arbitrum_components().await?;

    // Store node handle
    self.node_handle = Some(node_handle);
    
    Ok(())
}
```

## Component Customization

### Network Layer Customization

```rust
.network(|network_builder| {
    info!("Configuring Arbitrum network layer...");
    // Future: Add Arbitrum-specific network functionality:
    // - L1 connection management
    // - Arbitrum node discovery
    // - Custom message types for L2 protocol
    network_builder.build()
})
```

### Transaction Pool Customization

```rust
.pool(|pool_builder| {
    info!("Configuring Arbitrum transaction pool...");
    // Future: Add Arbitrum-specific pool logic:
    // - L2 gas pricing
    // - Priority fee handling
    // - Batch transaction ordering
    pool_builder.build()
})
```

### Consensus Mechanism Customization

```rust
.consensus(|consensus| {
    info!("Configuring Arbitrum consensus...");
    // Future: Add Arbitrum consensus:
    // - L1 PoS finality
    // - Rollup state transitions
    // - Challenge validation
    consensus
})
```

### EVM Customization

```rust
.evm(|evm_builder| {
    info!("Configuring Arbitrum EVM...");
    // Future: Add Arbitrum EVM functionality:
    // - L1/L2 message precompiles
    // - Arbitrum-specific opcodes
    // - Gas metering adjustments
    evm_builder.build()
})
```

## Starting the Node

### Basic Startup

```bash
# Start basic node
cargo run -- --datadir ./data

# Start sequencer mode
cargo run -- --sequencer --datadir ./data

# Start validator mode
cargo run -- --validator --datadir ./data

# Enable metrics server
cargo run -- --metrics --metrics-addr 127.0.0.1:9090
```

### Advanced Configuration

```bash
# Start with configuration file
cargo run -- --config config.toml --datadir ./data

# Enable Reth SDK debug mode
cargo run -- --reth-debug --log-level debug
```

## Architecture Advantages

### 1. Modular Design

- **Scalability**: Each component can be developed and tested independently
- **Maintainability**: Clear code structure, easy to understand and modify
- **Reusability**: Components can be reused in different configurations

### 2. High Performance

- **Reth Optimization**: Based on Reth's highly optimized implementation
- **Async Architecture**: Fully async design improves concurrency performance
- **Zero Copy**: Minimizes memory allocation and copying

### 3. Developer Experience

- **Type Safety**: Rust's type system ensures compile-time safety
- **Well Documented**: Detailed code comments and documentation
- **Test Coverage**: Complete unit tests and integration tests

## Future Plans

### Short-term Goals

1. **Complete Reth SDK Integration**: Implement all Reth SDK functionality
2. **Arbitrum Chain Specification**: Create Arbitrum-specific chain specification
3. **L1 Connection**: Implement connection to L1 Ethereum

### Medium-term Goals

1. **Custom Precompiles**: Implement Arbitrum-specific precompiled contracts
2. **Optimized Gas Metering**: Implement L2-specific gas calculation
3. **State Sync**: Implement efficient state synchronization mechanism

### Long-term Goals

1. **Full Arbitrum Compatibility**: 100% compatible with Arbitrum Nitro
2. **Performance Optimization**: Match or exceed official implementation performance
3. **Ecosystem Integration**: Complete integration with Arbitrum ecosystem tools

## Reference Resources

- [Official Reth SDK Documentation](https://reth.rs/sdk/overview)
- [Reth SDK Components Guide](https://reth.rs/sdk/node-components)
- [Arbitrum Nitro Documentation](https://docs.arbitrum.io/)
- [Project GitHub Repository](https://github.com/longcipher/arbitrum-reth)

## Contribution Guide

Code contributions are welcome! Please read [CONTRIBUTING.md](../CONTRIBUTING.md) to learn how to participate in project development.

### Development Environment Setup

1. Clone the repository
2. Install Rust toolchain
3. Run tests: `cargo test`
4. Start node: `cargo run`

### Code Standards

- Follow official Rust code style guidelines
- Add appropriate comments and documentation
- Write unit tests
- Use `cargo fmt` to format code
- Use `cargo clippy` to check code quality

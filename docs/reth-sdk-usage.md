# Reth SDK Usage Guide

Note: This guide shows practical examples. For architecture and component wiring with NodeBuilder, see also: reth-sdk-integration.md.

This document explains how to properly use the latest Reth SDK in the Arbitrum-Reth project.

## Latest Reth SDK Architecture (2024)

Reth SDK adopts a modular builder pattern that allows developers to customize various components of the node:

### Core Components

1. **NodeBuilder** - Main node builder
2. **ComponentBuilder** - Component customization builder  
3. **EthereumNode** - Ethereum node type definition
4. **Configuration System** - Declarative configuration

### Major Changes

Compared to older versions, the new Reth SDK has the following important changes:

1. **Use `NodeBuilder::new(config)`** instead of direct instantiation
2. **Specify node type through `.with_types::<EthereumNode>()`**
3. **Use `.with_components()` callback to customize components**
4. **Use `.launch().await?` to start the node**

## Correct Usage Patterns

### Basic Node Building

```rust
use reth_ethereum::node::{EthereumNode, NodeBuilder};

let node_handle = NodeBuilder::new(config)
    .with_types::<EthereumNode>()
    .with_components(|components| {
        // Customize components
        components
    })
    .launch()
    .await?;
```

### Advanced Component Customization

```rust
let node_handle = NodeBuilder::new(config)
    .with_types::<EthereumNode>()
    .with_components(|ctx| {
        ctx.components_builder()
            // Customize network component
            .network(|network_builder| {
                network_builder
                    .peer_manager(custom_peer_manager)
                    .build()
            })
            // Customize transaction pool
            .pool(|pool_builder| {
                pool_builder
                    .validator(custom_validator)
                    .ordering(custom_ordering)
                    .build()
            })
            // Customize consensus
            .consensus(custom_consensus)
            // Customize EVM
            .evm(|evm_builder| {
                evm_builder
                    .with_precompiles(custom_precompiles)
                    .build()
            })
            .build()
    })
    .launch()
    .await?;
```

## Project Update Steps

### 1. Update Dependencies

Add in `Cargo.toml`:

```toml
[workspace.dependencies]
reth = { git = "https://github.com/paradigmxyz/reth", features = ["dev"] }
reth-ethereum = { git = "https://github.com/paradigmxyz/reth" }
reth-node-builder = { git = "https://github.com/paradigmxyz/reth" }
reth-config = { git = "https://github.com/paradigmxyz/reth" }
# ... other reth components
```

### 2. Refactor Node Implementation

Refactor existing custom node implementation to be based on Reth SDK:

```rust
// Old way - direct implementation
impl ArbitrumRethNode {
    pub async fn start(&self) -> Result<()> {
        // Manually start various components
    }
}

// New way - using Reth SDK
impl ArbitrumRethNode {
    async fn build_reth_node(&self) -> Result<NodeHandle> {
        NodeBuilder::new(self.create_reth_config()?)
            .with_types::<EthereumNode>()
            .with_components(|ctx| {
                // Integrate Arbitrum components
                self.customize_components(ctx)
            })
            .launch()
            .await
    }
}
```

### 3. Implement Arbitrum-specific Components

Need to implement the following traits to integrate with Reth SDK:

- `reth_consensus::Consensus` - Arbitrum consensus
- `reth_transaction_pool::TransactionValidator` - Transaction validation
- `reth_network::PeerManager` - P2P management
- `reth_evm::Precompiles` - Precompiled contracts

### 4. Configuration Integration

Map Arbitrum configuration to Reth configuration:

```rust
fn create_reth_config(&self) -> Result<Config> {
    let mut reth_config = Config::default();
    
    // Map data directory
    reth_config.datadir = self.config.node.datadir.clone().into();
    
    // Map network configuration
    reth_config.network.port = self.config.p2p.port;
    
    // Map RPC configuration
    if self.config.rpc.enable {
        reth_config.rpc.http = Some(Default::default());
    }
    
    Ok(reth_config)
}
```

## Main Advantages

Using the latest Reth SDK has the following advantages:

1. **Standardization** - Follow standard patterns of the Reth ecosystem
2. **Maintainability** - Automatically get Reth updates and optimizations
3. **Interoperability** - Compatible with other Reth tools and components
4. **Performance** - Leverage Reth's high-performance implementation
5. **Feature Complete** - Get complete Ethereum node functionality

## Examples and References

- Check `examples/reth_sdk_usage.rs` for complete examples
- Refer to [official Reth documentation](https://docs.paradigm.xyz/reth)
- Check [Reth GitHub repository](https://github.com/paradigmxyz/reth) for latest information

## Migration Checklist

- [ ] Update Cargo.toml dependencies
- [ ] Refactor ArbitrumRethNode to use NodeBuilder
- [ ] Implement necessary Reth traits
- [ ] Test node startup and basic functionality
- [ ] Verify Arbitrum-specific functionality
- [ ] Update documentation and examples

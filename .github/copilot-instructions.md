# GitHub Copilot Instructions for Arbitrum-Reth

## Project Overview

Arbitrum-Reth is a high-performance Layer 2 node implementation built with the Reth SDK. This project provides a modular, Rust-based alternative to Arbitrum Nitro with 10x performance improvements while maintaining 100% protocol compatibility.

## Architecture Principles

### 1. Modular Workspace Design
- **Crate Structure**: 9 independent crates under `crates/` directory
- **Component Isolation**: Each crate has single responsibility (consensus, pool, storage, etc.)
- **Async-First**: All components use `async/await` patterns for concurrency
- **Type Safety**: Leverage Rust's type system for compile-time guarantees

### 2. Reth SDK Integration
- **NodeBuilder Pattern**: Use `NodeBuilder::new(config).with_types::<EthereumNode>()` for node construction
- **Component Customization**: Customize network, pool, consensus, and EVM components via builder pattern
- **Ethereum Compatibility**: Build on `EthereumNode` primitives while adding Arbitrum-specific functionality

## Coding Conventions

### 1. Error Handling
```rust
// Always use eyre::Result for public APIs
pub async fn start(&mut self) -> Result<()> {
    // Use ? operator for error propagation
    self.storage.start().await?;
    Ok(())
}

// Use tracing for logging, not println!
use tracing::{info, warn, error, debug};
info!("Starting Arbitrum-Reth node with Reth SDK");
```

### 2. Configuration Management
```rust
// Use the centralized ArbitrumRethConfig
pub async fn new(config: ArbitrumRethConfig) -> Result<Self> {
    // Validate configuration early
    config.validate()?;
    
    // Extract component-specific configs
    let storage_config = &config.storage;
}
```

### 3. Component Lifecycle
```rust
// All components should implement consistent lifecycle methods
impl Component {
    pub async fn new(config: &Config) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;
    pub async fn get_stats(&self) -> ComponentStats;
}
```

### 4. Reth SDK Component Integration
```rust
// When integrating with Reth SDK, follow this pattern:
NodeBuilder::new(config)
    .with_types::<EthereumNode>()
    .with_components(|ctx| {
        ctx.components_builder()
            .network(|builder| {
                // Configure Arbitrum-specific networking
                ArbitrumNetworkBuilder::new()
                    .with_l1_connection(config.l1.rpc_url.clone())
                    .build(builder)
            })
            .pool(|builder| {
                // Configure L2 transaction pool
                ArbitrumPoolBuilder::new()
                    .with_l2_gas_pricing()
                    .build(builder)
            })
            .build()
    })
    .launch()
    .await?
```

## Component-Specific Guidelines

### 1. arbitrum-node (Main Orchestrator)
- **Entry Point**: Contains `ArbitrumRethNode` struct that coordinates all components
- **Reth Integration**: Use `reth_integration.rs` for NodeBuilder patterns
- **Lifecycle**: Manage component startup/shutdown order carefully
- **Health Monitoring**: Implement comprehensive health checks and stats

### 2. arbitrum-config
- **Single Source**: All configuration should flow through `ArbitrumRethConfig`
- **Validation**: Implement `validate()` method for all config structs
- **TOML Support**: Use serde with TOML for configuration files
- **Environment Variables**: Support env var overrides with `#[serde(default)]`

### 3. arbitrum-consensus
- **L1 Finality**: Integrate with L1 Ethereum for finality guarantees
- **State Transitions**: Implement deterministic state transition validation
- **No Mining**: Arbitrum uses deterministic consensus, not PoW/PoS

### 4. arbitrum-pool
- **L2 Gas Pricing**: Implement L2-specific gas pricing mechanisms
- **Sequencer Ordering**: Support deterministic transaction ordering for sequencers
- **Cross-chain Messages**: Handle L1-to-L2 message inclusion

### 5. arbitrum-storage
- **MDBX Database**: Use MDBX for high-performance storage
- **State Management**: Implement efficient state trie operations
- **Parallel Access**: Support concurrent read operations

### 6. arbitrum-batch-submitter
- **L1 Integration**: Submit compressed transaction batches to L1
- **Compression**: Implement efficient batch compression algorithms
- **Fraud Proofs**: Generate and submit fraud proofs when needed

### 7. arbitrum-inbox-tracker
- **L1 Event Monitoring**: Watch L1 for incoming messages and events
- **Message Processing**: Process L1-to-L2 messages in order
- **State Sync**: Coordinate state updates from L1 events

### 8. arbitrum-validator
- **Challenge System**: Implement interactive fraud proof system
- **Dispute Resolution**: Handle challenge validation and resolution
- **Proof Generation**: Generate cryptographic proofs for challenges

## Development Patterns

### 1. Testing Strategy
```rust
// Use tokio::test for async tests
#[tokio::test]
async fn test_component_lifecycle() {
    let config = ArbitrumRethConfig::default();
    let component = Component::new(&config).await.unwrap();
    
    component.start().await.unwrap();
    assert!(component.is_running().await);
    
    component.stop().await.unwrap();
    assert!(!component.is_running().await);
}
```

### 2. Documentation Standards
```rust
/// Component for handling Arbitrum-specific functionality
/// 
/// This component integrates with Reth SDK to provide L2-specific features
/// while maintaining compatibility with Ethereum protocols.
/// 
/// # Example
/// 
/// ```rust
/// let config = ArbitrumRethConfig::default();
/// let component = Component::new(&config).await?;
/// component.start().await?;
/// ```
pub struct Component {
    // Always document public fields
    /// Configuration for this component
    pub config: ComponentConfig,
}
```

### 3. Performance Considerations
- **Async I/O**: Use `tokio::fs` for file operations, never `std::fs`
- **Connection Pooling**: Reuse database and network connections
- **Memory Management**: Use `Arc<T>` for shared state, `RwLock<T>` for concurrent access
- **Zero-Copy**: Prefer `&[u8]` over `Vec<u8>` when possible

### 4. Reth SDK Best Practices
- **Component Builders**: Always use builder patterns for component configuration
- **Type Constraints**: Use Reth's type system (e.g., `EthereumNode`) for compatibility
- **Extension Points**: Leverage Reth's extension system for custom functionality
- **Database Integration**: Use Reth's database abstractions, don't create custom ones

## CLI and Configuration

### 1. Command Line Interface
- **Clap Derive**: Use `#[derive(Parser)]` for CLI argument parsing
- **Subcommands**: Support sequencer, validator, and general node modes
- **Configuration**: Support both CLI args and config file with CLI taking precedence

### 2. Configuration File Structure
```toml
[node]
datadir = "./data"
sequencer_mode = false
validator_mode = false

[l1]
rpc_url = "https://eth-mainnet.alchemyapi.io/v2/API_KEY"

[metrics]
enable = false
addr = "127.0.0.1:9090"
```

## Integration Guidelines

### 1. Reth SDK Dependencies
- **Git Dependencies**: Use git dependencies for latest Reth SDK features
- **Version Compatibility**: Ensure compatibility with Reth's breaking changes
- **Feature Flags**: Use appropriate Reth feature flags to minimize binary size

### 2. External Dependencies
- **Minimal Dependencies**: Prefer standard library when possible
- **Async Runtime**: Use `tokio` exclusively, avoid other async runtimes
- **Serialization**: Use `serde` with appropriate formats (JSON for RPC, TOML for config)

### 3. Network Protocol
- **Ethereum Compatibility**: Maintain compatibility with Ethereum RPC APIs
- **Arbitrum Extensions**: Add `arb_*` namespace methods for L2-specific functionality
- **WebSocket Support**: Support both HTTP and WebSocket RPC endpoints

## Security Considerations

### 1. Input Validation
- **RPC Inputs**: Validate all RPC method parameters
- **Configuration**: Validate configuration values on startup
- **Network Messages**: Validate all incoming network messages

### 2. Resource Management
- **Memory Limits**: Implement memory usage limits and monitoring
- **File Descriptors**: Properly close file handles and network connections
- **Database Transactions**: Use proper transaction management

### 3. Cryptographic Security
- **Secure Randomness**: Use `getrandom` crate for cryptographic randomness
- **Signature Validation**: Properly validate transaction signatures
- **Hash Functions**: Use Keccak256 for Ethereum compatibility

## Common Pitfalls to Avoid

1. **Don't Block Async Context**: Never use blocking operations in async functions
2. **Don't Ignore Errors**: Always handle `Result` types, never `.unwrap()` in production
3. **Don't Create Custom Database**: Use Reth's database abstractions
4. **Don't Bypass Configuration**: Always use `ArbitrumRethConfig` for settings
5. **Don't Mix Sync/Async**: Keep sync and async code properly separated
6. **Don't Hard-code Values**: Use configuration for all environment-specific values

## Development Workflow

### 1. Building and Testing
```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --workspace -- -D warnings
```

### 2. Running the Node
```bash
# Basic node
cargo run -- --datadir ./data

# Sequencer mode
cargo run -- --sequencer --config config.toml

# With metrics
cargo run -- --metrics --metrics-addr 127.0.0.1:9090
```

### 3. Development Scripts
- Use `Justfile` for common development tasks
- Scripts are located in `scripts/` directory
- Examples are in `examples/` directory

## Future Development

When extending Arbitrum-Reth:

1. **Maintain Modularity**: New features should be self-contained crates
2. **Follow Reth Patterns**: Use Reth SDK patterns and conventions
3. **Performance First**: Consider performance implications of all changes
4. **Test Coverage**: Add comprehensive tests for new functionality
5. **Documentation**: Update architecture documentation for significant changes

This codebase represents a modern, high-performance implementation of Arbitrum using cutting-edge Rust and Reth SDK technologies. Follow these guidelines to maintain code quality and architectural consistency.

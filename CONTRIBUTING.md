# Contributing to Arbitrum-Reth

Thank you for your interest in contributing to Arbitrum-Reth! This document provides guidelines and information for contributors.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Contribution Guidelines](#contribution-guidelines)
5. [Code Standards](#code-standards)
6. [Testing](#testing)
7. [Documentation](#documentation)
8. [Pull Request Process](#pull-request-process)
9. [Issue Reporting](#issue-reporting)
10. [Community](#community)

## Code of Conduct

We are committed to providing a welcoming and inclusive experience for everyone. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

### Our Standards

- **Be respectful** and inclusive in all interactions
- **Be constructive** when giving feedback
- **Be patient** with newcomers and different skill levels
- **Be collaborative** and open to different perspectives
- **Focus on what's best** for the community and project

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- Rust 1.88.0 or later installed
- Git configured with your GitHub account
- Familiarity with Rust programming
- Basic understanding of blockchain and L2 technologies
- Understanding of the Arbitrum protocol (helpful but not required)

### Repository Structure

```
arbitrum-reth/
├── bin/                    # Executable binaries
│   └── arbitrum-reth/      # Main node binary
├── crates/                 # Library crates
│   ├── arbitrum-config/    # Configuration management
│   ├── arbitrum-consensus/ # Consensus implementation
│   ├── arbitrum-storage/   # Storage layer
│   ├── arbitrum-pool/      # Transaction pool
│   ├── arbitrum-batch-submitter/ # L1 batch submission
│   ├── arbitrum-inbox-tracker/   # L1 event monitoring
│   ├── arbitrum-validator/       # Validation logic
│   └── arbitrum-node/      # Node orchestration
├── docs/                   # Documentation
├── examples/               # Example code and usage
├── scripts/                # Development and deployment scripts
├── tests/                  # Integration tests
├── benches/                # Benchmarks
└── .github/                # GitHub workflows and templates
```

## Development Setup

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/arbitrum-reth.git
cd arbitrum-reth

# Add the original repository as upstream
git remote add upstream https://github.com/longcipher/arbitrum-reth.git
```

### 2. Install Dependencies

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install development tools
cargo install cargo-nextest  # Faster test runner
cargo install cargo-watch    # File watcher for development
cargo install cargo-audit    # Security audit tool
cargo install cargo-tarpaulin # Code coverage
rustup component add clippy  # Linter
rustup component add rustfmt # Code formatter
```

### 3. Build the Project

```bash
# Build all crates
cargo build

# Build with optimizations for testing
cargo build --release

# Build with all features
cargo build --all-features
```

### 4. Run Tests

```bash
# Run all tests
cargo nextest run

# Run tests for a specific crate
cargo nextest run -p arbitrum-consensus

# Run with coverage
cargo tarpaulin --all-features --workspace
```

### 5. Development Workflow

```bash
# Start development mode with file watching
cargo watch -x "check" -x "test" -x "run"

# Format code automatically on save
cargo watch -x fmt

# Run clippy on changes
cargo watch -x clippy
```

## Contribution Guidelines

### Types of Contributions

We welcome various types of contributions:

1. **Bug fixes** - Fix issues in existing code
2. **Feature implementation** - Add new functionality
3. **Performance improvements** - Optimize existing code
4. **Documentation** - Improve or add documentation
5. **Tests** - Add or improve test coverage
6. **Examples** - Create usage examples
7. **Tooling** - Improve development tools

### Before You Start

1. **Check existing issues** - Look for related issues or feature requests
2. **Create an issue** - For significant changes, create an issue first to discuss
3. **Join discussions** - Participate in issue discussions before starting work
4. **Claim issues** - Comment on issues you'd like to work on

### Development Process

1. **Create a feature branch** from `main`
2. **Make your changes** following our coding standards
3. **Write tests** for new functionality
4. **Update documentation** as needed
5. **Run the full test suite** to ensure nothing is broken
6. **Submit a pull request** with a clear description

## Code Standards

### Rust Code Style

We follow the official Rust style guidelines with some additional conventions.

#### Formatting

```bash
# Format all code (required before submitting)
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check
```

#### Linting

```bash
# Run clippy (required before submitting)
cargo clippy --all-features --all-targets

# Fix clippy suggestions automatically
cargo clippy --all-features --all-targets --fix
```

#### Code Structure

```rust
// File: crates/arbitrum-consensus/src/lib.rs

//! Arbitrum consensus implementation
//! 
//! This crate provides the consensus mechanism for Arbitrum L2,
//! handling block validation, finality, and state transitions.

#![deny(missing_docs)]
#![warn(unused_crate_dependencies)]

use std::sync::Arc;
use eyre::Result;
use tracing::{info, warn, error};

/// Arbitrum consensus engine
/// 
/// Handles L2 consensus logic including:
/// - Block validation
/// - State transitions  
/// - Finality determination
pub struct ArbitrumConsensus {
    // ... fields
}

impl ArbitrumConsensus {
    /// Create a new consensus engine
    /// 
    /// # Arguments
    /// 
    /// * `config` - Configuration for the consensus engine
    /// 
    /// # Returns
    /// 
    /// Returns a new `ArbitrumConsensus` instance
    /// 
    /// # Errors
    /// 
    /// Returns an error if initialization fails
    pub async fn new(config: &ArbitrumConfig) -> Result<Self> {
        // Implementation
    }
}
```

#### Error Handling

```rust
use eyre::{Result, WrapErr};

// Use eyre for error handling
pub fn process_block(block: Block) -> Result<ProcessedBlock> {
    let validated = validate_block(&block)
        .wrap_err("Failed to validate block")?;
    
    let processed = execute_block(validated)
        .wrap_err_with(|| format!("Failed to execute block {}", block.hash()))?;
    
    Ok(processed)
}

// Use thiserror for custom error types
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Invalid block: {reason}")]
    InvalidBlock { reason: String },
    
    #[error("State transition failed")]
    StateTransitionFailed(#[from] StateError),
}
```

#### Logging

```rust
use tracing::{info, warn, error, debug, trace, instrument};

#[instrument(skip(self), fields(block_hash = %block.hash()))]
pub async fn process_block(&self, block: Block) -> Result<()> {
    info!("Processing block {}", block.number());
    
    debug!("Block validation started");
    if let Err(e) = self.validate_block(&block).await {
        warn!("Block validation failed: {}", e);
        return Err(e);
    }
    
    trace!("Block validation completed");
    
    // ... rest of implementation
    
    info!("Block processed successfully");
    Ok(())
}
```

#### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_consensus_creation() {
        let config = ArbitrumConfig::default();
        let consensus = ArbitrumConsensus::new(&config).await
            .expect("Failed to create consensus engine");
        
        assert!(consensus.is_ready());
    }
    
    #[tokio::test]
    async fn test_block_validation() {
        let consensus = create_test_consensus().await;
        let block = create_test_block();
        
        let result = consensus.validate_block(&block).await;
        assert!(result.is_ok());
    }
    
    // Helper function for test setup
    async fn create_test_consensus() -> ArbitrumConsensus {
        let config = ArbitrumConfig::test_config();
        ArbitrumConsensus::new(&config).await
            .expect("Failed to create test consensus")
    }
}
```

### Documentation Standards

#### Public API Documentation

All public APIs must have comprehensive documentation:

```rust
/// Validates an Arbitrum block according to L2 consensus rules
/// 
/// This function performs comprehensive validation including:
/// - Transaction validity
/// - State root consistency  
/// - Gas limit compliance
/// - Timestamp validation
/// 
/// # Arguments
/// 
/// * `block` - The block to validate
/// * `parent_state` - The state after the parent block
/// 
/// # Returns
/// 
/// Returns `Ok(())` if the block is valid, or an error describing
/// the validation failure.
/// 
/// # Errors
/// 
/// This function will return an error if:
/// - Block format is invalid
/// - Transactions are malformed
/// - State transitions are inconsistent
/// - Gas limits are exceeded
/// 
/// # Examples
/// 
/// ```rust
/// use arbitrum_consensus::{ArbitrumConsensus, Block};
/// 
/// # async fn example() -> eyre::Result<()> {
/// let consensus = ArbitrumConsensus::new(&config).await?;
/// let block = Block::new(/* ... */);
/// 
/// consensus.validate_block(&block).await?;
/// println!("Block is valid!");
/// # Ok(())
/// # }
/// ```
pub async fn validate_block(&self, block: &Block) -> Result<()> {
    // Implementation...
}
```

#### README Files

Each crate should have a README.md file:

```markdown
# arbitrum-consensus

Arbitrum L2 consensus implementation.

## Overview

This crate provides the consensus mechanism for Arbitrum-Reth, handling
block validation, state transitions, and finality determination.

## Features

- L2 block validation
- State transition processing
- Integration with L1 finality
- Fraud proof support

## Usage

```rust
use arbitrum_consensus::ArbitrumConsensus;

let consensus = ArbitrumConsensus::new(&config).await?;
consensus.start().await?;
```

## Architecture

[Describe the internal architecture]

## Testing

```bash
cargo test -p arbitrum-consensus
```
```

## Testing

### Test Structure

```
tests/
├── unit/           # Unit tests (also in src/ files)
├── integration/    # Integration tests
├── e2e/           # End-to-end tests
├── fixtures/      # Test data and fixtures
└── common/        # Shared test utilities
```

### Writing Tests

#### Unit Tests

```rust
// In src/consensus.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_consensus_validation() {
        // Test specific function behavior
    }
    
    #[tokio::test]
    async fn test_async_consensus() {
        // Test async functions
    }
}
```

#### Integration Tests

```rust
// In tests/integration/consensus_integration.rs
use arbitrum_consensus::ArbitrumConsensus;
use arbitrum_config::ArbitrumConfig;

#[tokio::test]
async fn test_consensus_with_real_blocks() {
    let config = ArbitrumConfig::test_config();
    let consensus = ArbitrumConsensus::new(&config).await
        .expect("Failed to create consensus");
    
    // Test with realistic data
}
```

#### End-to-End Tests

```rust
// In tests/e2e/full_node_test.rs
#[tokio::test]
async fn test_full_node_sync() {
    // Start test node
    // Connect to test network
    // Verify full functionality
}
```

### Test Utilities

```rust
// In tests/common/mod.rs
use arbitrum_config::ArbitrumConfig;

pub fn test_config() -> ArbitrumConfig {
    ArbitrumConfig {
        // Test-specific configuration
        ..Default::default()
    }
}

pub async fn create_test_node() -> ArbitrumNode {
    // Helper to create test nodes
}
```

### Running Tests

```bash
# Run all tests
cargo nextest run

# Run specific test pattern
cargo nextest run test_consensus

# Run tests with coverage
cargo tarpaulin --all-features

# Run benchmarks
cargo bench

# Run with specific features
cargo test --features "test-utils"
```

## Documentation

### Types of Documentation

1. **Code Documentation** - Inline comments and doc comments
2. **API Documentation** - Generated from doc comments
3. **Architecture Documentation** - High-level design docs
4. **User Documentation** - Usage guides and tutorials
5. **Developer Documentation** - Contributing and development guides

### Building Documentation

```bash
# Generate and open API documentation
cargo doc --open --all-features

# Check for documentation issues
cargo doc --all-features --no-deps

# Test documentation examples
cargo test --doc
```

### Documentation Guidelines

- Use clear, concise language
- Include examples for public APIs
- Document error conditions
- Explain the "why" not just the "what"
- Keep documentation up to date with code changes

## Pull Request Process

### Before Submitting

1. **Sync with upstream**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run the full test suite**
   ```bash
   cargo nextest run --all-features
   cargo clippy --all-features --all-targets
   cargo fmt --all -- --check
   ```

3. **Update documentation**
   ```bash
   cargo doc --all-features --no-deps
   ```

### Pull Request Template

When creating a pull request, use this template:

```markdown
## Description

Brief description of the changes.

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] This change requires a documentation update

## Testing

- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] New tests added for new functionality

## Checklist

- [ ] My code follows the style guidelines of this project
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes

## Related Issues

Closes #(issue number)
```

### Review Process

1. **Automated checks** must pass (CI/CD pipeline)
2. **Code review** by at least one maintainer
3. **Testing** in development environment
4. **Documentation review** if applicable
5. **Final approval** and merge

## Issue Reporting

### Bug Reports

Use the bug report template:

```markdown
## Bug Description

A clear and concise description of what the bug is.

## To Reproduce

Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

## Expected Behavior

A clear and concise description of what you expected to happen.

## Environment

- OS: [e.g., Ubuntu 22.04]
- Rust version: [e.g., 1.88.0]
- Arbitrum-Reth version: [e.g., 0.1.0]

## Logs

```
Paste relevant log output here
```

## Additional Context

Add any other context about the problem here.
```

### Feature Requests

Use the feature request template:

```markdown
## Feature Description

A clear and concise description of what the problem is or what feature you'd like to see.

## Solution

A clear and concise description of what you want to happen.

## Alternatives

A clear and concise description of any alternative solutions or features you've considered.

## Additional Context

Add any other context or screenshots about the feature request here.
```

## Community

### Communication Channels

- **GitHub Issues** - Bug reports and feature requests
- **GitHub Discussions** - General discussions and questions
- **Discord** - Real-time chat (link to be provided)
- **Email** - For security issues: security@arbitrum-reth.dev

### Getting Help

1. **Check existing documentation** first
2. **Search existing issues** for similar problems
3. **Ask in GitHub Discussions** for general questions
4. **Create an issue** for bugs or feature requests

### Contributor Recognition

We maintain a list of contributors in:
- README.md
- CONTRIBUTORS.md
- GitHub repository contributors page

### Governance

This project follows a maintainer-led governance model:

- **Maintainers** have commit access and make final decisions
- **Contributors** can propose changes via pull requests
- **Community** provides feedback and suggestions

### Code of Conduct Enforcement

Violations of our Code of Conduct should be reported to:
- conduct@arbitrum-reth.dev
- GitHub's abuse reporting system

---

Thank you for contributing to Arbitrum-Reth! Your contributions help build the future of L2 infrastructure.

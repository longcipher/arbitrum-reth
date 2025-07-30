#!/usr/bin/env bash

# Arbitrum-Reth Reth SDK Integration Demo Script
# 
# This script demonstrates how to use the new Reth SDK integration to launch an Arbitrum-Reth node

set -e

echo "🚀 Arbitrum-Reth Reth SDK Integration Demo"
echo "==========================================="

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check dependencies
check_dependencies() {
    echo -e "${BLUE}Checking dependencies...${NC}"
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo not found. Please install Rust toolchain.${NC}"
        exit 1
    fi
    
    if ! command -v git &> /dev/null; then
        echo -e "${RED}Error: git not found.${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Dependency check passed${NC}"
}

# Build project
build_project() {
    echo -e "${BLUE}Building Arbitrum-Reth...${NC}"
    
    # Clean previous builds
    cargo clean
    
    # Build project (including Reth SDK dependencies)
    if cargo build --release; then
        echo -e "${GREEN}✓ Build successful${NC}"
    else
        echo -e "${RED}✗ Build failed${NC}"
        echo -e "${YELLOW}Note: Build may fail due to Reth SDK dependencies requiring specific versions.${NC}"
        echo -e "${YELLOW}This is normal as we are demonstrating the architectural design.${NC}"
        return 1
    fi
}

# Show Reth SDK integration architecture
show_architecture() {
    echo -e "${BLUE}Reth SDK Integration Architecture${NC}"
    echo "================================="
    
    cat << 'EOF'
┌─────────────────────────────────────────────────┐
│               Arbitrum-Reth Node                │
├─────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────────┐   │
│  │   Reth SDK      │  │  Arbitrum Components│   │
│  │                 │  │                     │   │
│  │ ┌─────────────┐ │  │ ┌─────────────────┐ │   │
│  │ │ NodeBuilder │ │  │ │ BatchSubmitter  │ │   │
│  │ │             │ │  │ │                 │ │   │
│  │ │ ┌─────────┐ │ │  │ │ ┌─────────────┐ │ │   │
│  │ │ │Network  │ │ │  │ │ │InboxTracker │ │ │   │
│  │ │ │         │ │ │  │ │ │             │ │ │   │
│  │ │ │Pool     │ │ │  │ │ │Validator    │ │ │   │
│  │ │ │         │ │ │  │ │ │             │ │ │   │
│  │ │ │Consensus│ │ │  │ │ │Storage      │ │ │   │
│  │ │ │         │ │ │  │ │ │             │ │ │   │
│  │ │ │EVM      │ │ │  │ │ └─────────────┘ │ │   │
│  │ │ └─────────┘ │ │  │ └─────────────────┘ │   │
│  │ └─────────────┘ │  └─────────────────────┘   │
│  └─────────────────┘                            │
└─────────────────────────────────────────────────┘
EOF
    
    echo ""
    echo -e "${GREEN}Core Features:${NC}"
    echo "• 🏗️  Modular architecture based on Reth SDK"
    echo "• ⚡ High-performance Ethereum execution client"
    echo "• 🔧 Customizable component system"
    echo "• 🎯 Arbitrum L2 specific functionality"
    echo "• 📊 Built-in metrics and monitoring"
}

# Show configuration examples
show_configuration() {
    echo -e "${BLUE}Configuration Examples${NC}"
    echo "======================"
    
    echo -e "${YELLOW}1. Basic node configuration:${NC}"
    echo "cargo run -- --datadir ./data"
    echo ""
    
    echo -e "${YELLOW}2. Sequencer mode:${NC}"
    echo "cargo run -- --sequencer --datadir ./data"
    echo ""
    
    echo -e "${YELLOW}3. Validator mode:${NC}"
    echo "cargo run -- --validator --datadir ./data"
    echo ""
    
    echo -e "${YELLOW}4. Enable metrics:${NC}"
    echo "cargo run -- --metrics --metrics-addr 127.0.0.1:9090"
    echo ""
    
    echo -e "${YELLOW}5. Use configuration file:${NC}"
    echo "cargo run -- --config config.toml --reth-debug"
}

# Show Reth SDK code examples
show_code_examples() {
    echo -e "${BLUE}Reth SDK Code Examples${NC}"
    echo "======================"
    
    echo -e "${YELLOW}NodeBuilder Pattern:${NC}"
    cat << 'EOF'
```rust
use reth_ethereum::EthereumNode;
use reth_node_builder::NodeBuilder;

// Build custom Arbitrum node
let node_handle = NodeBuilder::new(config)
    // Install Ethereum-specific node primitives
    .with_types::<EthereumNode>()
    .with_components(|ctx| {
        // Custom components
        ctx.components_builder()
            .network(|network_builder| {
                // Arbitrum network configuration
                network_builder.build()
            })
            .pool(|pool_builder| {
                // Arbitrum transaction pool configuration
                pool_builder.build()
            })
            .consensus(|consensus| {
                // Arbitrum consensus configuration
                consensus
            })
            .evm(|evm_builder| {
                // Arbitrum EVM configuration
                evm_builder.build()
            })
            .build()
    })
    .launch()
    .await?;
```
EOF
    echo ""
    
    echo -e "${YELLOW}Arbitrum Component Integration:${NC}"
    cat << 'EOF'
```rust
// Start Arbitrum-specific components
self.storage.start().await?;
self.consensus.start().await?;
self.tx_pool.start().await?;

if let Some(ref batch_submitter) = self.batch_submitter {
    batch_submitter.start().await?;
}

if let Some(ref validator) = self.validator {
    validator.start().await?;
}
```
EOF
}

# Show file structure
show_file_structure() {
    echo -e "${BLUE}Project File Structure${NC}"
    echo "======================"
    
    echo "arbitrum-reth/"
    echo "├── bin/"
    echo "│   └── arbitrum-reth/          # Main executable"
    echo "│       ├── Cargo.toml          # Reth SDK dependencies"
    echo "│       └── src/"
    echo "│           └── main.rs         # Enhanced main program"
    echo "├── crates/"
    echo "│   └── arbitrum-node/          # Core node implementation"
    echo "│       ├── Cargo.toml          # Reth SDK integration"
    echo "│       └── src/"
    echo "│           ├── lib.rs          # Reth SDK NodeBuilder"
    echo "│           └── reth_integration.rs  # Detailed integration example"
    echo "├── docs/"
    echo "│   ├── reth-sdk-integration.md # Integration guide"
    echo "│   └── reth_sdk_guide.rs       # Rust SDK guide"
    echo "├── Cargo.toml                  # Workspace configuration"
    echo "└── README.md                   # Updated documentation"
}

# Run demo
run_demo() {
    echo -e "${BLUE}Running Demo${NC}"
    echo "============"
    
    if [ -f "./target/release/arbitrum-reth" ]; then
        echo -e "${GREEN}Found compiled binary, starting demo...${NC}"
        echo -e "${YELLOW}Note: This will start a demo mode node${NC}"
        echo ""
        
        # Create data directory
        mkdir -p ./demo-data
        
        echo "Launch command: ./target/release/arbitrum-reth --datadir ./demo-data --reth-debug --log-level info"
        echo ""
        echo -e "${YELLOW}Press Ctrl+C to stop the node${NC}"
        echo ""
        
        # Optional execution
        read -p "Do you want to start the demo node? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            ./target/release/arbitrum-reth --datadir ./demo-data --reth-debug --log-level info
        fi
    else
        echo -e "${YELLOW}Compiled binary not found.${NC}"
        echo -e "${YELLOW}Run 'cargo build --release' to compile the project.${NC}"
    fi
}

# Main function
main() {
    echo "This script demonstrates the Reth SDK integration of Arbitrum-Reth"
    echo ""
    
    # Check dependencies
    check_dependencies
    echo ""
    
    # Show architecture
    show_architecture
    echo ""
    
    # Show configuration
    show_configuration
    echo ""
    
    # Show code examples
    show_code_examples
    echo ""
    
    # Show file structure
    show_file_structure
    echo ""
    
    # Ask whether to compile
    read -p "Do you want to compile the project? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if build_project; then
            echo ""
            run_demo
        fi
    fi
    
    echo ""
    echo -e "${GREEN}Demo completed!${NC}"
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "1. Check documentation: docs/reth-sdk-integration.md"
    echo "2. Review code: crates/arbitrum-node/src/reth_integration.rs"
    echo "3. Run tests: cargo test"
    echo "4. Start node: cargo run -- --help"
    echo ""
    echo -e "${YELLOW}For more information, please check the project README.md${NC}"
}

# Run main function
main "$@"

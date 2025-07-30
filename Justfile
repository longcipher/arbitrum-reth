# Arbitrum-Reth Development Commands

# Format code and configuration files
format:
  taplo fmt
  cargo +nightly fmt --all

# Check code formatting and linting
lint:
  taplo fmt --check
  cargo +nightly fmt --all -- --check
  cargo +nightly clippy --all -- -D warnings -A clippy::derive_partial_eq_without_eq -D clippy::unwrap_used -D clippy::uninlined_format_args
  cargo machete

# Run tests
test:
  cargo test

# Enable Reth SDK dependencies for development
enable-reth:
  sed -i.bak 's/# reth-/reth-/g' Cargo.toml
  sed -i.bak 's/# reth-/reth-/g' crates/arbitrum-node/Cargo.toml
  echo "Reth dependencies enabled. Run 'just build-reth' to compile."

# Disable Reth SDK dependencies for quick builds
disable-reth:
  sed -i.bak 's/^reth-/# reth-/g' Cargo.toml
  sed -i.bak 's/^reth-/# reth-/g' crates/arbitrum-node/Cargo.toml
  echo "Reth dependencies disabled for quick compilation."

# Build with Reth SDK (slow but full functionality)
build-reth:
  just enable-reth
  cargo build --release

# Quick build without Reth SDK
build-quick:
  just disable-reth
  cargo build

# Run the node
run *args:
  cargo run --bin arbitrum-reth -- {{args}}

# Run the demo
demo:
  rustc demo.rs && ./demo

# Clean up generated files
clean:
  cargo clean
  rm -f demo reth_sdk_guide
  rm -f *.bak crates/*/*.bak

# Show project status
status:
  @echo "📊 Arbitrum-Reth Project Status"
  @echo "==============================="
  @echo ""
  @echo "🔧 Reth Dependencies:"
  @if grep -q "^reth-" Cargo.toml; then echo "  ✅ Enabled"; else echo "  ❌ Disabled"; fi
  @echo ""
  @echo "📁 Project Structure:"
  @echo "  • bin/arbitrum-reth - Main binary file"
  @echo "  • crates/arbitrum-* - Core components"
  @echo "  • examples/ - SDK usage examples"
  @echo "  • docs/ - Documentation and guides"
  @echo ""
  @echo "🚀 Commands:"
  @echo "  • just enable-reth  - Enable Reth SDK"
  @echo "  • just disable-reth - Disable Reth SDK"
  @echo "  • just build-reth   - Complete build"
  @echo "  • just build-quick  - Quick build"
  @echo "  • just demo         - Run demo"
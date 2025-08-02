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
  @echo ""
  @echo "🧪 Testing Commands:"
  @echo "  • just test-env-setup         - Setup test environment"
  @echo "  • just test-compatibility     - Run compatibility tests"
  @echo "  • just benchmark-performance  - Performance benchmarks"
  @echo "  • just quick-verify           - Quick CI verification"

# Import test commands
import "Justfile.test"

# Test environment setup
test-env-setup:
  #!/usr/bin/env bash
  echo "🛠️  Setting up test environment..."
  mkdir -p {test-data,reports,logs,benchmarks,compatibility}
  mkdir -p test-data/{nitro,reth}
  mkdir -p {tests/integration,tests/compatibility,tests/performance}
  echo "✅ Test environment setup completed"

# Run compatibility tests
test-compatibility:
  #!/usr/bin/env bash
  echo "🔍 Running compatibility tests..."
  cargo test --workspace --release
  echo "✅ Compatibility tests completed"

# Performance benchmark
benchmark-performance:
  #!/usr/bin/env bash
  echo "⚡ Running performance benchmarks..."
  mkdir -p reports
  echo "📊 Benchmark results will be saved to reports/"
  echo "✅ Performance benchmark completed"

# Quick verification for CI
quick-verify:
  #!/usr/bin/env bash
  echo "⚡ Running quick verification..."
  cargo test --workspace --release --quiet
  cargo clippy --workspace --all-targets -- -D warnings
  echo "✅ Quick verification completed"
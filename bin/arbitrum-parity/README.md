# arbitrum-parity

A tiny JSON-RPC parity harness to diff responses between two endpoints (e.g., Rust L2 node vs Nitro).

## Usage

```bash
# Simple run comparing a few methods (result-only by default)
arbitrum-parity --left http://127.0.0.1:8545 --right http://127.0.0.1:8546 \
  --methods eth_blockNumber,eth_chainId,net_version,eth_gasPrice \
  --params []

# Use a matrix file with per-method params
arbitrum-parity --left http://127.0.0.1:8545 --right http://127.0.0.1:8546 \
  --matrix bin/arbitrum-parity/examples/matrix.basic.json

# Compare full responses; ignore certain paths
arbitrum-parity --left http://127.0.0.1:8545 --right http://127.0.0.1:8546 \
  --methods eth_getBlockByNumber --params '["latest", false]' \
  --compare full --ignore "/result/timestamp"
```

- --methods: comma-separated list (used if --matrix not provided).
- --params: JSON array or @file path containing the JSON (shared across methods if --matrix not used).
- --iters: run multiple iterations to catch flakiness.
- --matrix: JSON array of { method, params } objects for per-method configuration.
- Each entry may include optional overrides: `compare` ("result"|"full"), `ignore` (comma-separated paths), `sort_logs` (bool).
- --compare: "result" (default) or "full".
- --ignore: comma-separated JSON pointer paths to remove before comparing.
- --report: write a JSON report with summary and cases.
- --timeout: request timeout in seconds (default: 10).
- --sort-logs: when comparing result arrays of logs, sort them by blockNumber/transactionIndex/logIndex.

Examples:

```bash
# Matrix for blocks
arbitrum-parity --left http://127.0.0.1:8545 --right http://127.0.0.1:8546 \
  --matrix bin/arbitrum-parity/examples/matrix.blocks.json \
  --compare full --ignore "/result/timestamp" --report report.blocks.json

# Matrix for logs with per-entry overrides
arbitrum-parity --left http://127.0.0.1:8545 --right http://127.0.0.1:8546 \
  --matrix bin/arbitrum-parity/examples/matrix.logs.json --report report.logs.json
```


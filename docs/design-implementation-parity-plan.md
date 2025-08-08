# Arbitrum-Reth: Design, Gap Analysis, and Implementation Plan

This document captures the current status vs. Nitro, the missing features, a concrete implementation roadmap, and the parity testing strategy to validate each milestone. It’s grounded on the existing crates in this repo and the Reth SDK patterns.

## objectives

- Achieve functional parity with Nitro while leveraging Reth’s performance and modularity
- Provide a staged plan with acceptance checks and regression tests
- Enable continuous, measurable parity against a reference Nitro node

## architecture snapshot (today)

- Node orchestration: `arbitrum-node` (lifecycles, status, TODO Reth wiring)
- Config: `arbitrum-config` (validated TOML/env)
- Consensus: `arbitrum-consensus` (skeleton; TODO execution, fork-choice, state roots)
- Pool: `arbitrum-pool` (skeleton; TODO signature/nonce/balance/pricing)
- Storage: `arbitrum-storage` (MDBX infra done; CRUD mostly TODO)
- Inbox Tracker, Batch Submitter, Validator: async loops scaffolded; real L1 integration TODO
- Tests/harness: `tests/rpc_compatibility_tester.rs`, `tests/data_consistency_checker.rs`, `tests/performance_benchmark.rs`

## gaps vs. nitro

1. Reth SDK integration

- Missing NodeBuilder launch and component customization

1. EVM & L2 semantics

- Precompiles (ArbSys, ArbGasInfo, NodeInterface), L2 gas metering/opcode diffs

1. L1 integration

- Inbox events, batch posting transactions, challenge protocol; need robust L1 JSON-RPC client and contract bindings

1. Storage CRUD

- Block/tx/receipt/state trie/batch/message read/write paths and indexing

1. Consensus/execution

- Deterministic ordering, state root computation, genesis setup, fork choice, chain spec

1. Batch pipeline

- Batch encoding/compression, SequencerInbox/Bridge interactions, confirmations, reorg resistance

1. Validator/challenges

- Re-execution parity check, fraud-proof generation hooks, dispute lifecycle monitoring

1. RPC/network

- Expose full `eth_*` and `arb_*` namespaces; ensure network syncing interop

## implementation roadmap

Milestone A: Reth SDK wiring + storage CRUD (foundation)

- Implement `reth_integration::launch_reth_node()` using NodeBuilder with EthereumNode
- Expose HTTP/WS RPC and basic net/web3/eth methods via Reth
- Implement storage CRUD for blocks/txs/receipts/batches/messages and basic genesis init
- Acceptance:
  - Build & run; `rpc_compatibility_tester` quick suite ≥95% pass
  - `data_consistency_checker` over 0..N by 100s shows 0 structural mismatches for basic fields

Milestone B: Inbox tracking + execution parity (read path)

- L1 client: JSON-RPC (HTTPS/WS), log/event filters, pagination, retries
- Process historical Inbox events into storage; deterministic inclusion into L2 blocks
- Genesis and chain spec refined; state root calc path in consensus
- Acceptance:
  - `rpc_compatibility_tester` full `eth_*` core and selected `arb_*` pass with tolerances
  - `data_consistency_checker` headers/txs/receipts parity on sampled windows

Milestone C: EVM precompiles + L2 gas

- Register Arbitrum precompiles and implement NodeInterface queries; align gas pricing/estimation semantics
- Extend RPCs to `arb_*` where needed (e.g., estimate components)
- Acceptance:
  - Precompile call vectors match Nitro outputs
  - Gas estimates within defined tolerance across samples

Milestone D: Batch pipeline (sequencer path)

- Batch building/ordering/compression, SequencerInbox posting, confirmation tracking
- Reorg resistance parameters and monitoring
- Acceptance:
  - Batches posted and recognized identically by Nitro
  - End-to-end L1→L2→L1 roundtrip tests pass

Milestone E: Validator & challenges (happy path)

- Re-execution checks, proof scaffolding, onchain challenge tx stubs
- Monitor challenge lifecycle; basic assertions in dev environments
- Acceptance:
  - Triggered challenges follow expected transitions; receipts/logs consistent

## testing and parity strategy

Tools

- RPC parity: `tests/rpc_compatibility_tester.rs` (quick/full suites, tolerance knobs), and a new CLI `bin/arbitrum-parity` to diff JSON-RPC outputs across two endpoints.
- Data consistency: `tests/data_consistency_checker.rs` (headers/tx/receipts across ranges)
- Performance: `tests/performance_benchmark.rs` (throughput, latency, reliability)

Execution

- Run Nitro reference (Docker or binary) at 8547; Arbitrum-Reth at 8548
- After each milestone, execute:
  - Quick RPC suite (smoke)
  - Data consistency sampling (e.g., 0..10,000 step 100)
  - Performance snapshot (short duration)
  - Optional: run `just parity -- --left http://127.0.0.1:8548 --right http://127.0.0.1:8547 --methods eth_blockNumber,eth_chainId --params []`

Acceptance gates

- Functional: RPC methods parity (≥99% pass after Milestone C), data structures match
- Performance: Track trend; final goal ≥5–10x TPS vs Nitro on comparable hardware
- Stability: 24h continuous run w/o crashes, memory steady

## component contracts (concise)

- reth_integration
  - Input: validated `ArbitrumRethConfig`
  - Output: running Reth node handle (async launch)
  - Errors: config invalid, port conflicts, DB/open failures

- storage
  - CRUD for: blocks, txs, receipts, state trie nodes, batches, l1 messages, metadata
  - Concurrency: shared reads, serialized writes; atomic batch operations

- inbox tracker
  - Source: L1 RPC; decode sequencer inbox logs
  - Output: ordered messages/events persisted; progress cursor

- batch submitter
  - Input: ordered L2 blocks/txs; Output: L1 tx; Confirmations tracked

- validator
  - Input: posted batches and local re-exec; Output: challenges (when mismatch)

## risks and mitigations

- Reth API drift → pin git SHA; wrap usages in integration module; CI check
- L1 RPC variance → retries/backoff, pagination, log range chunking, reorg handling
- Data model mismatches → golden vectors from Nitro; dual-run compare tools (already present)

## immediate work items

- Add `reth_integration::launch_reth_node()` scaffold and call it from node start (no-op for now)
- Implement storage CRUD for blocks/tx/receipts (start with block by number/hash; unit tests)
- Prepare L1 client interface (traits + basic HTTP calls)

## how to validate each commit

- Build + clippy + fmt
- Smoke: `rpc_compatibility_tester --test-suite quick`
- Data: `data_consistency_checker` on a tiny range when applicable

---

Maintainers can extend this plan by linking PRs to milestones and updating acceptance criteria as features land.


## progress update (latest)

- Implemented storage support for receipts and logs, with codecs and CRUD.
- Expanded mock JSON-RPC to include:
  - eth_getTransactionReceipt
  - eth_getLogs, eth_newFilter, eth_getFilterChanges, eth_uninstallFilter
- Added integration tests that verify:
  - Basic getLogs address/topic filtering across a block range
  - Filter lifecycle with incremental polling and uninstall
  - Multi-address and topic semantics: OR within each topic position, AND across positions, and wildcard via null (new test added)
  - Blocks, transactions, balances remain covered; includeTxs=true returns full tx objects
- All workspace tests pass. A minor lint in consensus tests was fixed.

Additionally:

- Introduced `bin/arbitrum-parity`, a small parity harness to compare JSON-RPC responses between two endpoints with configurable methods/params and iterations. A Just task `parity` was added.

Next steps:

1. Integrate/proxy the experimental Reth NodeBuilder RPC into the current HTTP surface under a feature flag.
2. Extend topic/address filtering to cover OR semantics and arrays; add tests for multi-address and multi-topic cases.
3. Add basic arb_* RPC stubs and start a Nitro vs. Rust RPC parity harness run for the new endpoints.

## Recent updates (Aug 2025)

This iteration adds two targeted improvements to JSON-RPC logs/filters that boost performance and durability while maintaining Ethereum-correct semantics verified by tests:

- Persistent filter cursors
  - New LMDB table: `filter_cursors` (key: filter id u64, value: last processed block u64)
  - On `eth_getFilterChanges`, in-memory cursor is merged with persisted value and advanced in bounded chunks; we best-effort persist after each chunk.
  - Survives process restarts; avoids re-scanning previously polled ranges.

- Simple per-block logs index
  - New LMDB table: `logs_by_block` (key: block number, value: Vec\<Log\>)
  - Populated on `store_receipt`: logs are enriched with block/tx context (blockNumber, blockHash, transactionHash, transactionIndex, logIndex) and appended to the block’s entry.
  - `eth_getLogs` first tries the index for the block; falls back to concurrent receipt scans if absent. Output shape matches the receipt path to preserve parity.

Validation

- All existing `arbitrum-node` RPC smoke tests remain green, including topic semantics and filter lifecycle.
- Added internal storage APIs: `set_filter_cursor`, `get_filter_cursor`, `index_logs_for_block`, `get_indexed_logs_in_range`.

Impact and next steps

- Performance: Indexed blocks return logs without decoding all receipts; chunked polling bounds latency and memory.
- Durability: Filters progress survives restarts. In-memory definitions remain transient by design.
- Follow-ups:
  - TTL/expiry for filters and periodic pruning of `filter_cursors`.
  - Backfill/indexer for historical logs and secondary indexes by address/topic for faster filtered queries.
  - Optional CI smoke parity job using the `arbitrum-parity` harness focusing on logs-heavy matrices.

Configuration

- New knob: `[rpc].filter_ttl_ms` controls the TTL for JSON-RPC log filters. Set to `0` to use the built-in default (5 minutes). This is applied when the server initializes `FiltersManager`.

1. Add lightweight indexing for logs (by topic/address) to avoid linear scans; keep the current behavior as fallback.
2. Begin wiring L1 inbox tracker to feed real receipts/logs for more realistic getLogs coverage.

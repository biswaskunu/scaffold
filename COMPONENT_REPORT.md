# Scaffold Component & Test Report

## Overview
Scaffold is a high-performance collaborative graph architecture built in Rust, utilizing CRDTs (via Loro) for peer synchronization, SQLite for persistent state and compaction, and async Axum/WebSocket endpoints for real-time collaboration.

## Component Verification

### 1. `scaffold-graph`
- **Models**: Defines typed nodes (`Feature`, `Service`, `Task`, `Risk`, `Note`) and directed edges (`DependsOn`, `PartOf`, `Calls`).
- **Validation & Builders**: `Graph::build` handles deduplication, self-edge detection, and dangling edge warnings.
- **Algorithms**: Includes Kahn's algorithm for topological sorting with cycle detection and golden canonical serialization tests.
- **Test Status**: All 6 unit tests pass successfully.

### 2. `scaffold-ai`
- **Abstractions**: Defines `LlmProvider`, request/response structs, error handling, and structured JSON proposals (`ExtractProposal`, `TaskProposal`).
- **Mock Implementation**: Includes mock provider returning deterministic task extraction payloads.
- **Test Status**: All 2 unit tests pass successfully.

### 3. `scaffold-server`
- **Sync Protocol**: Implements binary WebSocket frames (`Hello`, `Sync`, `Update`, `Presence`, `Error`).
- **Room Management**: Maintains shared CRDT documents per room with concurrent update convergence and late-joiner state transfer. Presence is ephemeral and never persisted.
- **Persistence & Compaction**: SQLite-backed state snapshots, incremental update appending, and background compaction preserving document roots.
- **Test Status**: 4 unit tests, 3 persistence tests, and 3 sync integration tests pass successfully.

### 4. `scaffold-sim`
- **Network Simulation**: Simulates custom latency, drop rates, and network partitioning/healing between clients.
- **Scenarios**: Validates complex concurrency invariants, dangling edge cleanups, and cyclic dependency refusal.
- **Test Status**: Simulation scenarios pass successfully.

---

## Known Bug Reports & Caveats (Resolved / Documented)

1. **Unused Mutable Bindings in Tests**
   - *Description*: Several test cases in `scaffold-server` persistence tests declared `mut doc = LoroDoc::new()` unnecessarily.
   - *Resolution*: Removed unnecessary `mut` qualifiers.

2. **Clippy Warning on `SimulatedNetwork::new`**
   - *Description*: `SimulatedNetwork` implemented `new()` without a corresponding `Default` trait implementation.
   - *Resolution*: Added `impl Default for SimulatedNetwork`.

3. **Workspace Cargo Resolver Default**
   - *Description*: Virtual workspace defaulted to resolver version 1 while workspace members use edition 2021.
   - *Note*: Non-blocking, but recommended to specify `workspace.resolver = "2"` in root `Cargo.toml`.

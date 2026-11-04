# Testing

Testing is the credibility layer of this project. A CRDT app that "seems to work" is not interesting; one that provably converges under hostile network conditions is.

## Layers

| Layer | Where | Tooling | Network? |
|---|---|---|---|
| Unit | `graph/`, `ai/` | `cargo test` | No |
| Property | `graph/`, `tests/` | `proptest` | No |
| Simulation | `tests/sim/` | custom harness | Simulated |
| Integration | `tests/integration/` | real server + test clients | Localhost |
| AI contract | `ai/` | `MockProvider` | No |
| Client | `client/` | typecheck, lint, component tests, one end-to-end smoke test | Localhost |

## 1. Convergence (property tests)

**Claim:** after all updates are delivered, every replica has identical state, regardless of delivery order.

- Generate random sequences of operations (create/move/edit/delete node, add/remove edge) across N simulated clients (N from 2 to 5).
- Apply them locally on each client, then exchange updates in random order, with duplicates and delays.
- Assert: all replicas export identical canonical JSON.
- Shrinking must produce a minimal failing sequence. Save failing seeds as regression tests.

## 2. Partition and network simulation

A deterministic harness sits between clients and the server and can:

- Drop, delay, duplicate, and reorder messages
- Partition a client for a window of simulated time, then heal
- Disconnect mid-sync

Scenarios that must pass:

1. Client A and B edit the same node's different fields while partitioned; after healing, both changes survive.
2. A deletes node N while B adds an edge to N; after healing, graph building ignores the dangling edge and cleanup removes it.
3. A and B each add a `depends_on` edge that together form a cycle; after healing, the cycle is detected and Extract refuses with the cycle reported.
4. Server restarts mid-session; clients reconnect and converge with no lost accepted updates.
5. A client edits offline for a long period, reloads from IndexedDB, then reconnects.

Use seeded randomness so every failure is reproducible from its seed.

## 3. Graph logic (unit + golden)

- Cycle detection: acyclic graphs, self-loops, two-node cycles, long cycles, multiple components. Cycle result lists the offending node ids.
- Toposort: valid order for DAGs; deterministic tie-breaking; stable across runs.
- Dangling edge and duplicate edge handling.
- **Golden tests** for canonical serialization: fixed input graphs and exact expected JSON strings. Any change to canonical output is a deliberate, reviewed change.

## 4. AI contract tests

`MockProvider` must be able to return each of these, and each must be caught:

| Mock output | Expected result |
|---|---|
| Valid output | Accepted, proposal returned |
| Malformed JSON | Rejected, one retry |
| Schema mismatch (missing field, wrong type) | Rejected, one retry |
| `source_node` that doesn't exist (hallucinated id) | Rejected, one retry |
| Duplicate node entries | Rejected |
| Empty strings or absurd effort values | Rejected |
| Fails twice in a row | Structured error to the caller |
| Provider timeout | Structured error, no partial proposal |

Also test:
- Extract on a cyclic graph never calls the provider (assert zero provider calls).
- The model's output cannot change the computed order (feed outputs that "suggest" a different order; ordering must still come from edges).
- Staleness detection flags a proposal when its source nodes change after `V0`.

## 5. Integration

- Start a real server on a random port with a temp SQLite file.
- Two WebSocket test clients join one room, edit, and assert convergence.
- Restart the server and assert persisted state reloads.
- Run Extract end to end with `MockProvider`.
- Compaction: after N updates, compaction runs and the reloaded state is identical.

## 6. Client

- Typecheck and lint in CI.
- Component tests for node creation, edge drawing, and the proposal review panel.
- One end-to-end smoke test (headless browser): two tabs, edit in each, assert both show the same graph, toggle offline in one and verify merge.



## Scenarios 1, 4, and 5

1. **Same-node different-field edits across a partition (Scenario 1):** Client A and B edit different fields of the same node while partitioned. After healing, both changes survive because Loro's mergeable maps handle field-level merging. This validates that the CRDT correctly handles concurrent field edits without conflicts.

4. **Server restart mid-session (Scenario 4):** A client edits during an active session, the server restarts, and the client reconnects. The client should reload the latest state from the persisted SQLite state (latest snapshot + replayed updates) with no lost accepted updates. This tests the persistence and convergence guarantee.

5. **Long offline edit followed by reload (Scenario 5):** A client edits offline for an extended period, then reloads the room document from IndexedDB while still offline, then reconnects to the server. The local state should merge with the server's state upon reconnection, demonstrating that IndexedDB persistence combined with CRDT convergence works across extended offline periods.
## CI expectations

On every PR: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`, client typecheck and lint, and the client test suite. Long-running simulation seeds can run on a nightly schedule with more iterations.

## What not to test with real services

Unit and simulation tests never hit a real LLM. Optionally keep a clearly separated, manually-run smoke test against a real provider, skipped by default.

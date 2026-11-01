# Decision Records

Lightweight ADRs. Add a new entry at the bottom when making a significant choice. Never rewrite history; supersede an entry with a new one instead.

Template:

```
## ADR-NNN: Title
- Status: proposed | accepted | superseded by ADR-XXX
- Date: YYYY-MM-DD
- Context: why this came up
- Decision: what we chose
- Consequences: what gets easier, what gets harder
```

---

## ADR-001: The LLM never decides structure
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** AI features on whiteboards usually let the model free-write plans from flattened text. That makes output unverifiable and structure unreliable.
- **Decision:** Dependencies come from canvas edges. Order comes from a deterministic toposort in Rust. The LLM may only attach content (subtasks, descriptions, estimates) to existing nodes, and all output is validated before use. AI output is a proposal until the user accepts it.
- **Consequences:** Output is checkable and reproducible, and the project has a clear technical story. The model is less "creative" about structure; users who want that must edit the canvas.

## ADR-002: Loro as the CRDT
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** Candidates were `yrs` (Yjs port), Automerge, and Loro.
- **Decision:** Use Loro: the `loro` crate on the server and the `loro-crdt` WASM package in the browser.
- **Consequences:** Rust-native API on the server with snapshot and version features. The ecosystem around providers, presence, and editor bindings is younger than Yjs, so Scaffold defines its own thin sync framing (see `ARCHITECTURE.md`) and may relay presence itself. APIs evolve, so pin versions and verify against docs before relying on specific calls.

## ADR-003: Web UI client in TypeScript
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** A Rust-in-WASM UI (Leptos/egui) is possible but adds friction for a canvas-heavy interface.
- **Decision:** Build the client as a web app in TypeScript. The server and all core logic stay in Rust.
- **Consequences:** Faster UI iteration and better coding-agent reliability. Two languages in the repo. The CRDT runs in the browser via Loro's WASM package.

## ADR-004: SQLite for persistence
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** Zero budget, local-first spirit, single-process server.
- **Decision:** SQLite via `sqlx`, with an append-only update log plus snapshot compaction.
- **Consequences:** Trivial to self-host and test. Not designed for multi-node scaling, which is out of scope.

## ADR-005: Presence is ephemeral and never persisted
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** Cursors and selections change constantly and have no value after disconnect.
- **Decision:** Presence travels over the same WebSocket but is never written to the document or database.
- **Consequences:** Small documents and clean history. Requires timeout-based ghost cleanup.

## ADR-006: Project name is Scaffold
- **Status:** accepted
- **Date:** 2026-10-31
- **Context:** The name should fit "structure first, tasks hang off it".
- **Decision:** Call the project Scaffold. Crate names use the `scaffold-` prefix (`scaffold-server`, `scaffold-graph`, `scaffold-ai`).
- **Consequences:** "Scaffold" is a very common word in software (framework code generators, for example), so search results and registry names may collide. Check availability of the GitHub repo name, crate names, npm package, and domain before publishing, and consider a distinguishing suffix if needed.

## ADR-007: M1 WebSocket sync framing
- **Status:** accepted
- **Date:** 2026-11-01
- **Context:** M1 needs versioned framing for room join, initial sync, live updates, and protocol errors over the existing WebSocket endpoint.
- **Decision:** Use binary frames with a one-byte type tag and a versioned JSON payload for control messages. Encode the room id and client version vector in `Hello`; return missing update bytes and the server version vector in `SyncReply`; carry raw Loro update bytes in `Update`; use structured code/message data in `Error`. Reject unknown message types, malformed frames, oversized frames, and protocol-version mismatches. The server keeps one in-memory Loro document per room and broadcasts accepted updates to the other room peers.
- **Consequences:** Control frame structure is inspectable and can evolve under an explicit protocol version; update payloads stay in Loro's binary format. JSON adds small control-frame overhead. Room state is lost on server restart until M2 persistence.

## ADR-008: Loro dependency pin and ephemeral presence relay
- **Status:** accepted
- **Date:** 2026-11-01
- **Context:** We need a pinned version of the `loro` Rust crate and a resolution for how presence is handled before M4.
- **Decision:** Pin `loro = "1.17.1"` in workspace dependencies and scaffold-server. For presence (resolving OD-4), relay plain messages via the `0x04` presence frame on the WebSocket, avoiding premature dependency on Loro's ephemeral API.
- **Consequences:** Stable, reproducible builds with Loro 1.17.1. Presence is decoupled from CRDT history and ready for M4.

## ADR-009: Client framework and canvas rendering (React Flow)
- **Status:** accepted
- **Date:** 2026-11-01
- **Context:** Selecting a web framework and canvas approach for the TypeScript client (resolving OD-2 and OD-3).
- **Decision:** Use React with React Flow (xyflow) for the client web UI.
- **Consequences:** Mature node-graph library with built-in node dragging, connection handles, and zoom/pan; highly reliable with coding agents; lets us focus on the Rust/CRDT sync experience rather than building canvas primitives from scratch.

## ADR-010: Dedicated simulation workspace crate (`scaffold-sim`)
- **Status:** accepted
- **Date:** 2026-11-01
- **Context:** M3 and M5 testing require simulating multi-client convergence, network partitions, dangling edge cleanup, and cycle detection without coupling graph logic to Axum servers or tokio runtimes.
- **Decision:** Introduce a dedicated workspace crate `scaffold-sim` containing the network simulation harness and simulation test scenarios (such as Scenarios 2 and 3).
- **Consequences:** Clean separation of concerns between pure graph logic, server persistence/sync, and network simulation harnesses. Reusable across property and partition tests.

---

## Open decisions

| # | Question | Notes |
|---|---|---|
| OD-1 | Default LLM provider for demos | Undecided. Candidates: local Ollama model, or a free-tier OpenAI-compatible API. Check current limits before choosing. |
| OD-5 | Node body type | Plain string (v1 default) vs Loro text container. Revisit if collaborative text editing is wanted. |
| OD-6 | License | MIT proposed. |

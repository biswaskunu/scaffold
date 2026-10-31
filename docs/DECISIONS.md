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

---

## Open decisions

| # | Question | Notes |
|---|---|---|
| OD-1 | Default LLM provider for demos | Undecided. Candidates: local Ollama model, or a free-tier OpenAI-compatible API. Check current limits before choosing. |
| OD-2 | Client framework | React, Svelte, or Solid. Pick based on the best node-graph/canvas library available and how well the agent handles it. |
| OD-3 | Canvas rendering approach | Existing node-graph library vs custom SVG/Canvas. Library is faster to ship; custom gives control over presence rendering. |
| OD-4 | Presence transport | Use Loro's built-in ephemeral support if sufficient, otherwise plain relayed messages. Decide during M4. |
| OD-5 | Node body type | Plain string (v1 default) vs Loro text container. Revisit if collaborative text editing is wanted. |
| OD-6 | License | MIT proposed. |

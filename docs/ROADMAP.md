# Roadmap

Milestones are strictly ordered. Do not start milestone N+1 until every acceptance criterion of N passes (see [`AGENTS.md`](../AGENTS.md)).

Legend: `[ ]` not started, `[~]` in progress, `[x]` done.

---

## M0: Skeleton `[x]`

**Goal:** a workspace that builds, lints, tests, and runs in CI.

- Cargo workspace with `server/`, `graph/`, `ai/`; `client/` scaffolded
- `rustfmt`, `clippy`, CI workflow, `.env.example`, `.gitignore`
- Server starts and echoes over a WebSocket at `/ws/:room_id`

**Acceptance**
- [x] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace` pass in CI
- [x] Client installs, typechecks, and builds
- [x] A test connects to the WebSocket and gets an echo

---

## M1: Sync `[x]`

**Goal:** two clients edit one room and converge through the server.

- Loro document per room on the server
- Hello / SyncReply / Update protocol from [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Minimal client: create nodes, move them, see the other client's changes

**Acceptance**
- [x] Integration test: two clients make concurrent edits; both end with identical canonical JSON
- [x] Late-joining client receives full state
- [x] Protocol version mismatch produces a clear `Error` frame

---

## M2: Persistence `[x]`

**Goal:** rooms survive restarts.

- SQLite schema, update log, snapshot compaction
- Load = latest snapshot + replay newer updates

**Acceptance**
- [x] Restart test: edit, stop server, start server, state is identical
- [x] Compaction is atomic; killing the process mid-compaction loses nothing
- [x] Reloaded state after compaction equals pre-compaction state

---

## M3: Offline `[x]`

**Goal:** clients edit offline and merge on reconnect.

- IndexedDB persistence on the client
- Reconnect with backoff, resync using version vectors
- Visible online/offline indicator

**Acceptance**
- [ ] Simulation scenarios 1, 4, and 5 from [`TESTING.md`](TESTING.md) pass
- [ ] Browser reload while offline restores the latest local state
- [ ] Property test: random concurrent edits across 2 to 5 replicas always converge

---

## M4: Presence `[x]`

**Goal:** see other people.

- Live cursors, selections, names, colors
- Ghost cleanup on disconnect and by timeout
- Presence is never persisted

**Acceptance**
- [x] Two tabs show each other's cursors and selected nodes
- [x] Closing a tab removes its presence within the timeout
- [x] Presence data does not appear in the document or database

---

## M5: Graph logic `[ ]`

**Goal:** the deterministic core, fully tested.

- Graph builder with read-path invariants
- Canonical serializer
- Cycle detection that reports the cycle
- Toposort with deterministic tie-breaking

**Acceptance**
- [ ] Unit and golden tests per [`TESTING.md`](TESTING.md) section 3
- [ ] Simulation scenarios 2 and 3 pass
- [ ] `graph/` has no I/O or async dependencies

---

## M6: Extract `[ ]`

**Goal:** canvas to validated proposal to accepted tasks.

- `LlmProvider` trait, `MockProvider`, Ollama and OpenAI-compatible providers
- Prompt builder, JSON schema, validator, single retry
- `POST /api/rooms/:id/extract` returning a proposal with `V0`
- Client review panel, accept/reject, staleness handling

**Acceptance**
- [ ] Every row of the AI contract table in [`TESTING.md`](TESTING.md) section 4 has a passing test
- [ ] Extract on a cyclic graph returns the cycle and makes zero provider calls
- [ ] Order of the returned plan is identical for any model output
- [ ] Accepting a proposal creates `ai_generated` tasks via ordinary CRDT edits
- [ ] A stale proposal is flagged

---

## M7: Versions and export `[ ]`

**Goal:** named history and a portable plan.

- Create, list, restore named snapshots
- Markdown checklist export in topological order

**Acceptance**
- [ ] Restoring a snapshot converges across connected clients
- [ ] Exported checklist order matches the plan order

---

## M8: Polish and release `[ ]`

**Goal:** something a stranger can understand and run in five minutes.

- README with demo GIF and quickstart, `LICENSE`
- Docker Compose for local run (optional)
- Deployment notes
- Docs reviewed against the actual implementation

**Acceptance**
- [ ] A fresh clone to a running demo in under ten minutes following only the README
- [ ] No doc contradicts the code

---

## Stretch backlog (unordered)

- GitHub Issues export
- Share links with view/edit roles
- Comments pinned to nodes
- Scoped Extract on a subgraph
- "Explain this architecture" for a selected subgraph
- Collaborative text editing inside node bodies
- Undo/redo that respects multiplayer semantics

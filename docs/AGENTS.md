# AGENTS.md

Working agreement for AI coding agents in this repository. Read this fully before making changes. Humans: this is also the shortest summary of the project's rules.

## What this project is

Scaffold is a local-first, multiplayer canvas of typed nodes and edges. An **Extract** feature turns the graph into an ordered task plan. Read `README.md` and `docs/` before coding.

## The non-negotiable rule

**The LLM never decides structure.**

- Dependencies come from the canvas edges. Ordering comes from a deterministic topological sort in Rust.
- The LLM may only produce: subtasks, descriptions, effort estimates for nodes that already exist.
- Every LLM output is validated in Rust before it is shown to a user. Invalid output is rejected or retried, never "fixed up" silently.
- AI output is always a **proposal**. It never writes to the CRDT without explicit user acceptance.

If a change would weaken any of the above, stop and ask.

## Stack

| Area | Choice |
|---|---|
| Server | Rust (stable), Axum, Tokio |
| CRDT | `loro` (Rust crate on the server, `loro-crdt` WASM package on the client) |
| Persistence | SQLite via `sqlx` |
| Client | TypeScript web UI (framework to be fixed in `docs/DECISIONS.md`) |
| AI | `LlmProvider` trait; Ollama and OpenAI-compatible endpoints |

Do not add a new framework, database, or major dependency without recording the decision in `docs/DECISIONS.md` first.

## Workspace rules

- `graph/` is **pure**: no network, no filesystem, no async runtime. It must stay trivially unit-testable.
- `ai/` depends on `graph/`; `server/` depends on both. Never the reverse.
- IDs are ULIDs (strings). Never use array indices as identity.
- No `unwrap()` / `expect()` in non-test code. Use typed errors (`thiserror`) and propagate.
- No `unsafe` unless justified in a comment and in `docs/DECISIONS.md`.
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --workspace` must pass before you call a task done.
- Client: strict TypeScript, no `any` without a comment, lint and typecheck clean.

## How to work

1. **One milestone at a time.** Milestones are in `docs/ROADMAP.md`. Do not start milestone N+1 until N's acceptance criteria pass.
2. **Tests first for logic.** For anything in `graph/`, `ai/` validation, or the sync protocol, write the failing test, then the code.
3. **Small, reviewable commits.** One concern per commit. Conventional Commit messages (`feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `chore:`).
4. **Don't invent APIs.** The Loro APIs evolve. Check the current crate and package docs for the exact version pinned in `Cargo.toml` / `package.json` rather than relying on memory. If unsure, write a small spike test to confirm behavior.
5. **Ask before big changes.** Schema changes to the Loro document, protocol changes, and anything touching the non-negotiable rule need a note in `docs/DECISIONS.md` and human approval.
6. **Explain the hard parts.** When you implement sync, merge, or validation logic, add a short comment or doc section explaining *why* it is correct. The maintainer needs to be able to explain it.

## Definition of done (per task)

- [ ] Acceptance criteria from the milestone are met
- [ ] New logic has tests (unit, property, or integration as appropriate)
- [ ] fmt, clippy, tests, and client lint/typecheck all pass
- [ ] Docs updated if behavior, schema, or protocol changed
- [ ] No TODOs left without an issue reference

## Things to avoid

- Auto-applying AI output to the document
- Letting the model reorder, add, or remove edges
- Storing secrets (API keys) in the repo; use environment variables and `.env.example`
- Network calls in unit tests (use the mock provider)
- Premature optimization; this is built for hundreds of nodes, not millions

## Environment variables

Documented in `.env.example` once M0 lands. Expected: `DATABASE_URL`, `LLM_PROVIDER`, `LLM_BASE_URL`, `LLM_MODEL`, `LLM_API_KEY` (optional for local providers).

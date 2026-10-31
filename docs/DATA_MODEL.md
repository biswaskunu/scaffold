# Data Model

One room = one Loro document. All durable canvas state lives in it. Ephemeral state (presence) does not.

## Entities

### Node

| Field | Type | Notes |
|---|---|---|
| `id` | ULID string | Identity. Also the map key. Never reused. |
| `type` | enum | `feature`, `service`, `task`, `risk`, `note` |
| `title` | string | Short label shown on the canvas |
| `body` | string | Plain text description (no rich text in v1) |
| `x`, `y` | number | Canvas position |
| `status` | enum | `todo`, `doing`, `done`, `blocked` (meaningful for `task`; optional elsewhere) |
| `effort` | number or null | Optional estimate in hours; set by user or accepted from AI |
| `created_by` | string | Client/user id |
| `ai_generated` | bool | `true` if created by accepting an Extract proposal |
| `source_node` | ULID or null | For AI-generated tasks: the node they were derived from |

### Edge

| Field | Type | Notes |
|---|---|---|
| `id` | ULID string | Identity and map key |
| `from` | ULID | Source node id |
| `to` | ULID | Target node id |
| `kind` | enum | `depends_on`, `part_of`, `calls` |

**Edge semantics**
- `depends_on`: `from` cannot start until `to` is done. **This is the only kind that affects ordering.**
- `part_of`: `from` is a component or child of `to`. Used for grouping and for scoping Extract.
- `calls`: runtime relationship between services. Informational; used for architecture explanation, not ordering.

## Loro layout (proposed)

```
root
├── meta      : Map   { schema_version, name }
├── nodes     : Map   { <node_id> : Map { type, title, body, x, y, status, effort, created_by, ai_generated, source_node } }
└── edges     : Map   { <edge_id> : Map { from, to, kind } }
```

Notes:
- Use one nested map per node and edge so concurrent edits to *different fields* of the same node merge independently (one user moves it while another renames it).
- Position (`x`, `y`) should be written as two plain values, and drag updates should be throttled so a drag does not flood the update log.
- Text fields (`title`, `body`) start as plain string values (last-writer-wins per field). If collaborative text editing inside a node becomes a requirement, switch `body` to Loro's text container and bump `schema_version`.
- Evaluate whether Loro's tree or movable containers add value here. A flat map plus `part_of` edges is simpler and probably enough.
- Check the pinned Loro version's docs for exact container APIs before implementing.

## Invariants

These must hold on the **read path** (the `graph` crate), because a CRDT merge cannot enforce cross-object constraints.

1. Every edge's `from` and `to` should reference an existing node. Concurrent "delete node" and "add edge to that node" can produce a **dangling edge**. Policy: dangling edges are ignored when building the graph and are garbage-collected by a periodic cleanup edit.
2. `depends_on` edges must form a DAG. Concurrent edits can create a cycle even when each client's local graph was acyclic. Policy: the UI warns when a cycle exists, and Extract refuses to run until it is resolved (and reports the cycle).
3. No self-edges (`from == to`). Rejected by the UI; ignored by the graph builder if present.
4. Duplicate edges (same `from`, `to`, `kind`) are collapsed when building the graph.
5. Unknown `type` / `kind` values (from a newer client) are preserved in the document but treated as `note` / ignored by validation, so older clients do not crash.

## Canonical serialization

`graph` exposes a function that turns a document snapshot into a **canonical JSON** form used for prompts, hashing, and golden tests:

- Nodes sorted by `id`, edges sorted by `(kind, from, to)`.
- Only fields relevant to planning are included (no `x`, `y`).
- Stable key order, no whitespace dependence.
- Same graph in, same bytes out, regardless of which client produced it or in what order edits arrived.

This guarantees reproducible prompts and lets tests compare against fixed expected output.

## Schema evolution

- `meta.schema_version` is an integer, starting at `1`.
- Additive changes (new optional fields) do not need a bump.
- Breaking changes need a bump, a migration step on load, and an entry in [`DECISIONS.md`](DECISIONS.md).

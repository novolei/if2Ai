# `src/stores/` — Per-feature UI session stores

> Phase M2 source-of-truth boundary. Last updated: 2026-04-20 (M2 audit).

## Decision (M2.4 retroactive)

If2Ai has **two** front-end state layers after M2:

1. **`src/runtime-projection/runtime-projection-store.ts`** — canonical
   single source of truth for **backend event projections** produced
   by the M0.3 / M0.4 / M0.5 contracts:

   | Field                    | Source                                     |
   | ------------------------ | ------------------------------------------ |
   | `runs`                   | `agent-token` events via translator        |
   | `approvals`              | `permission-request` events via translator |
   | `memory.recentEvents`    | `memory_event` events via translator       |
   | `memory.lastRecallItems` | `stream_complete.memoryItems`              |
   | `activation`             | `activation_get_status` fetch seam         |
   | `executionMode`          | `request_intelligence_classify` fetch seam |

   This store is **append-only via canonical events**. Every mutation
   goes through `translator → queue → reducer`. Page components MUST
   consume via `useRuntimeProjection*` hooks; they MUST NOT mutate it.

2. **`src/stores/*-slice.ts`** — per-feature UI session state that
   does **not** map to a single canonical backend event source:

   | Slice                   | Owns                                                                              | Why kept (not folded into projection store)                                                                                                                                                                                                                                                                                                 |
   | ----------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | `conversation-slice.ts` | per-session `Conversation` (messages timeline, todos, title state, loading flags) | UI-derived per-session bundle; the projection store carries `runs[runId]` (per-run streaming text/tool calls), but the chat surface needs a longer-lived per-session message log layered on top. M2.8+ will progressively read from `runs` and synthesise this slice from it; full retire is M3+ when memory coordinator owns the timeline. |
   | `browser-slice.ts`      | active browser sessions                                                           | Driven by `BrowserStatusEvent` (a different Tauri event family) consumed directly by `BrowserCard`. Not a runtime-projection citizen. M3+ may consider folding once memory / browser interplay matures.                                                                                                                                     |

## Removed (M2 audit)

- `memory-slice.ts` — **deleted**. Had zero in-tree consumers and its
  contract (`showPermissionPrompt`, `recordMemoryWriteEvent`,
  `setRecallInProgress`) was already covered by the canonical
  `runtime-projection-store.snapshot.{approvals, memory}`. Keeping it
  would have produced two competing sources of truth for permission
  + memory events.

## Hard rules

1. **No double-source of truth**: any data already projected by
   `runtime-projection-store` MUST NOT also live in a slice here.
2. **Slices may not subscribe to backend events directly**. If a new
   backend event source needs to feed UI state, add a translator in
   `src/runtime-projection/` first. Slices then derive from the
   projection if needed.
3. **No new slice files** without first checking whether the data
   belongs in `runtime-projection-store`.
4. New transport-level event sources land via the translator family,
   never via a fresh `listen(...)` inside a slice.

## Migration trajectory

- M2.8 (chat main path) → `ChatWorkspace` reads `snapshot.runs` for
  the active stream, then synthesises `Conversation` via
  `conversation-slice` only for cross-stream history.
- M3 (memory coordinator) → `memory.lastRecallItems` +
  `memory.recentEvents` plus a future `memory.writeDecisions` field
  fully replaces any per-session memory cache the chat UI
  accumulated. No slice work needed if the projection covers it.
- Eventually `conversation-slice` and `browser-slice` may either
  retire (if projection covers them) or be re-homed under
  `src/state/` per the phase-m2 YAML naming. Both options are
  acceptable; what is **not** acceptable is letting them drift back
  into the same field space as the projection store.

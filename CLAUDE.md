# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

If2Ai is a Tauri 2 + Rust + React (TypeScript) AI agent desktop app. Backend binary is `if2ai-backend`. Frontend dev server runs on `http://localhost:9527`.

The codebase is mid-migration to a "vNext session runtime" model:

```
Session = durable event log + runtime supervisor + frontend projection
```

- `SessionMeta` holds only lightweight product metadata.
- `Run/Event Log` is the append-only source of truth (`src-tauri/src/modules/runtime/`).
- `Projection` is the UI read model (`src/runtime-projection/`).
- `Session Supervisor` owns lifecycle (active run, cancel, resume, pending permission, retry) — see [`src-tauri/src/modules/runtime/supervisor.rs`](src-tauri/src/modules/runtime/supervisor.rs).
- All runtime events flow through the canonical `runtime_event` Tauri channel via [`runtime_event::dispatch`](src-tauri/src/modules/runtime/runtime_event.rs) (legacy `agent-token` / `permission-request` / `memory_event` channels were retired in PR D-1, 2026-05-02).

New features should consume the typed API facade (`src/api/*`) and the projection read model — not raw `invoke` calls or new transcript stores. `src/App.tsx`, `src/components/ui/chat-ui.tsx`, and `src-tauri/src/modules/application/turn_service/stream_task.rs` are the priority split targets; avoid piling new logic onto them.

## Development Workflow (Superpowers — mandatory)

All development work — features, behavior changes, non-trivial refactors, bug fixes, performance and security changes — **must** use the **Superpowers SKILLS** flow. Skills override default system-prompt behavior; user instructions in this file remain highest priority.

The canonical loop:

1. **`using-superpowers`** — invoked at session start; establishes how to find and use skills.
2. **`brainstorming`** — required before any creative work (new feature, component, behavior change).
3. **`writing-plans`** — required when you have a spec or requirements for a multi-step task, before touching code.
4. **`test-driven-development`** — required when implementing any feature or bugfix, before writing implementation code.
5. **`systematic-debugging`** — required when encountering any bug, test failure, or unexpected behavior, before proposing fixes.
6. **`subagent-driven-development`** / **`dispatching-parallel-agents`** — for plans with independent tasks; dispatch parallel agents.
7. **`verification-before-completion`** — required before claiming work complete; run verification commands and confirm output.
8. **`requesting-code-review`** — required before merging or marking a major feature complete.
9. **`receiving-code-review`** — required when implementing review feedback.
10. **`finishing-a-development-branch`** — required when implementation is complete and integration is needed.

Domain-specific skills (`rust-best-practices`, `rust-async-patterns`, `claude-api`, `swiftui-pro`, etc.) layer on top of the process skills above. If a skill might apply with even 1% probability, invoke it. See `~/.claude/skills/using-superpowers/` for the full discipline.

The legacy **Pack pipeline** (`./scripts/pack run <PACK-ID>`, `docs/packs/CHARTER.md`, `docs/packs/REGISTRY.md`, GFR/FEAT/CPD/BUG/PERF/DEP types, `## Files (scope)` constraints, ≤ 60-line Pack docs, Refactor Pack Invariants I1–I7) is **deprecated** as a mandatory entry point. Existing Pack files under `docs/packs/**` remain readable for historical context but **must not** be treated as a required workflow gate. Do not author new Pack files; use the Superpowers loop above.

## Common Commands

| Command | Purpose |
| --- | --- |
| `npm install` | Install frontend deps |
| `npm run tauri:dev` | Start desktop dev (Vite on :9527 + Tauri host) |
| `npm run dev` | Vite-only frontend dev |
| `npm run build:web` | Build frontend |
| `npm run build` | Build Tauri desktop app (runs `build:web` first) |
| `npm test` | Node test runner over `src/**/*.test.ts` (uses `scripts/test-loader.mjs`) |
| `npm run check:version` | Verify `package.json` / `Cargo.toml` / `tauri.conf.json` versions match |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Fast Rust check |
| `cargo test --manifest-path src-tauri/Cargo.toml <filter> --lib` | Targeted Rust tests |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | Required format check |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Required lint check |
| `npm run release:manifest:validate -- <manifest> stable` | Validate release manifest |

`verification-before-completion` requires running `cargo fmt --check`, `cargo clippy -D warnings`, the relevant `cargo test` filter, and (when frontend is touched) `npm test` plus `npm run build:web` before claiming done.

## Hard Rules

- ❌ Do NOT browse `docs/_legacy/` (frozen exec-plans, old implementation-packs, refactor v1) — content is retired.
- ❌ Do NOT browse `docs/staff-remediation/` unless a plan or skill explicitly names a path.
- ❌ Do NOT author new files under `docs/packs/**`. Existing files are historical context only.
- ❌ No `unwrap()` / `expect()` / `todo!()` / `unimplemented!()` in non-test Rust code.
- ❌ No cross-module `use crate::xxx` shortcuts; always go through `use crate::modules::*`.
- ❌ Do NOT bypass the `src/api/*` facade with raw `invoke` calls. New UI must depend on `src/api/*` and `src/runtime-projection/*`, not `@/lib/tauri` raw IPC.
- ❌ Do NOT introduce new Tauri event channels parallel to `runtime_event`; emit via `runtime_event::dispatch` and consume via the projection bridge.
- ❌ Do NOT extend `session.json` to carry transcript / projection / resume facts. Transcript truth lives in the run event log.
- ✅ All `pub fn` need `///` doc comments. Async = tokio (no `std::thread`).
- ✅ All runtime events must populate `CorrelationIds` (`session_id`, `run_id`, plus `team_id` / `member_id` / `delegation_id` when Teams ships).
- ✅ Plans, brainstorms, and verification artifacts produced by skills should be saved under `docs/superpowers/plans/` (dated filename).

Lint contract details: [docs/references/coding-style-and-lint-contract.md](./docs/references/coding-style-and-lint-contract.md).

## Architecture Boundaries

The codebase is a strict layered architecture. Dependencies flow downward only; commands never import from each other, and runtime never imports from the application layer.

### Backend (`src-tauri/src/modules/`)

```
commands/                IPC adapters (thin; no business logic; AppState injection only)
   ↓
application/             TurnService + orchestration services
   ↓
control_plane/           Policy boundary, session context, permission prep, audit
   ↓
runtime/                 Event log, supervisor, contracts, conversation loop, history
   ↓
domain modules           memory, identity, projects, session, tools, provider,
                         harness, security, browser, smart_browser, learning,
                         scheduler, skills, desktop_host, observability, …
```

| Module | Ownership |
| --- | --- |
| [`commands/`](src-tauri/src/commands/) | Tauri IPC layer. Aggregated in `commands/mod.rs` AppState. Adapters are 5–15 LOC; no policy or orchestration. |
| [`bootstrap/`](src-tauri/src/bootstrap/) | App startup, AppState construction, SQLite/LanceDB wiring, legacy data-dir migration (MEM-MOD-PATH-FIX → unified `~/.if2ai/`). |
| [`modules/application/turn_service/`](src-tauri/src/modules/application/turn_service/) | Canonical chat orchestrator. `mod.rs`, `run.rs` (non-streaming), `stream*.rs` (streaming entry, task, finalize, iteration, tool execution, run-log helpers), `work_loop.rs`. **Do not pile new logic into `stream_task.rs` or `work_loop.rs`** — extract sibling modules. |
| [`modules/application/`](src-tauri/src/modules/application/) (other) | `prompt_coordinator`, `prompt_planner/`, `memory_coordinator`, `memory_injection_service`, `memory_candidate_extractor`, `memory_*` (quality / conflict / promotion), `request_intelligence_service`, `provider_service`, `permission_service`, `tool_executor`, `stream_emitter_service`, `trajectory_service`. Each owns one decision boundary. |
| [`modules/control_plane/`](src-tauri/src/modules/control_plane/) | `SessionContextResolver`, `prepare_step_execution`, `boundary_resolver`, `tool_execution_broker`, `audit`, `ingress_classifier`, `session_bridge` (AppSession ↔ RuntimeSession). Centralizes pre-execution policy. |
| [`modules/runtime/`](src-tauri/src/modules/runtime/) | vNext core. `event_log.rs` (append-only JSONL truth source), `history.rs` (paged replay; `session.json` only as fallback when no run-log JSONL exists), `pending_permission.rs`, `supervisor.rs` (MIG-020 lifecycle snapshot owner), `runtime_event.rs::dispatch` (canonical emit helper), `evolution_emitter.rs`, `stream_emitter.rs`, `conversation.rs`, `run_delegate.rs`, `agent_loop/`, `attempt_ledger.rs` (MIG-022 8-state ledger), `contracts/` (`common`, `agent_loop`, `execution_mode`, `memory`, `prompt`, `activation`), `budget.rs`, `compact.rs`, `context_compression/`, `mcp*`, `resume_cursor.rs`, `working_checkpoint.rs`, `self_repair.rs`, `cost_guard`, `lifecycle_hooks`. |
| [`modules/memory/`](src-tauri/src/modules/memory/) | Layered: providers (SQLite, LanceDB vector, hybrid), `pinned/`, `summary/`, `retrieval/`, `quality/`, `security/`, `cognitive/`, `learning_traits.rs`, `promotion.rs`, `job_runner.rs`, `ticker.rs`, `inject.rs`, `compiler.rs`, `llm.rs` (UtilityLlm shim), `embedding.rs`. |
| [`modules/identity/`](src-tauri/src/modules/identity/) | Soul / Persona / ResolvedIdentity, identity packs, identity injection. |
| [`modules/session/`](src-tauri/src/modules/session/) | Product session metadata, `session.json` compat storage, undo. |
| [`modules/projects/`](src-tauri/src/modules/projects/) | Workspace / project boundary. |
| [`modules/provider/`](src-tauri/src/modules/provider/) | Provider registry, known models, capability probing, resilience decorator (retry / circuit breaker). |
| [`modules/tools/`](src-tauri/src/modules/tools/) | Tool registry, toolset, builtin tools (bash, file ops, http, memory, search, cron, …), execution outputs. |
| [`modules/learning/`](src-tauri/src/modules/learning/) | `trajectory.rs` (ShareGPT JSONL), `reflection.rs`, `self_model.rs`, `trust_tracker.rs`, `promotion_gate.rs`, `failure_taxonomy.rs`, `strategy_registry.rs`. Per-turn callbacks wired into TurnService finalization. |
| [`modules/harness/`](src-tauri/src/modules/harness/) | Independent observability tier. `event_bus.rs` (`tokio::broadcast`, capacity 256), `telemetry.rs`, `session_recorder.rs` (JSONL trace), `trace_aggregator.rs` (`HarnessRunReport`), `report_persistence.rs`, `compare`, `gate`, `graders`. **Note:** harness is currently a parallel truth source — MIG-023 is partial; long-term it should derive from canonical event log. |
| [`modules/smart_browser/`](src-tauri/src/modules/smart_browser/) + [`modules/browser/`](src-tauri/src/modules/browser/) | Smart Browser contract / policy / backend adapter (local CDP via chromiumoxide, browser-use MCP, future cloud). |
| [`modules/security/`](src-tauri/src/modules/security/) | Redaction, safety policy, ThreatScanner, secret detection. |
| [`modules/scheduler/`](src-tauri/src/modules/scheduler/), [`modules/skills/`](src-tauri/src/modules/skills/), [`modules/desktop_host/`](src-tauri/src/modules/desktop_host/), [`modules/observability/`](src-tauri/src/modules/observability/), [`modules/onboarding/`](src-tauri/src/modules/onboarding/), [`modules/jiaochang_audio/`](src-tauri/src/modules/jiaochang_audio/), [`modules/updater/`](src-tauri/src/modules/updater/) | Specialized subsystems. |

### Frontend (`src/`)

```
App.tsx                     Boot sequencer + projection wiring (priority split target)
   ↓
modules/app-shell/          AppShell + ContentRouter (pure render; no business logic)
   ↓
modules/{chat,memory,settings,skills,jiaochang,browser,…}/   Per-domain UI
   ↓ uses
api/  +  runtime-projection/  +  state/  +  stores/   (typed surfaces)
   ↓
transport/contracts.ts                              (canonical wire envelopes)
   ↓
lib/tauri.ts → @tauri-apps/api                      (transport only)
```

| Surface | Ownership |
| --- | --- |
| [`src/api/`](src/api/) | Typed Tauri command facades — `client.ts` (transport seam), `conversations.ts` (`startChatTurn` → `ChatStreamHandle`), `streaming.ts` (`listenToStream` filters `runtime_event` by `correlation.streamId`), `sessions.ts`, `memory.ts` (with `useMemoryEntries` / invalidation hooks), `models.ts`, `projects.ts`, `identity.ts`, `updater.ts`, `onboarding.ts`, `window.ts`, `slash.ts`. **All UI must depend on these, not raw `invoke`.** |
| [`src/transport/`](src/transport/) | `contracts.ts` is the single source of truth for `RuntimeEventEnvelope`, `CorrelationIds`, `RuntimeEventType`, payload shapes. Mirrors `src-tauri/src/modules/runtime/contracts/`. |
| [`src/runtime-projection/`](src/runtime-projection/) | Canonical UI read model. `runtime-projection-bridge.ts` subscribes to the single `runtime_event` channel; `envelope-router.ts` family-routes; `runtime-event-translator.ts` + `runtime-event-reducer.ts` produce immutable `RuntimeProjectionSnapshot`; `chat-run-projection.ts` projects runs → messages; `history-replay.ts` restores from event-log paging; `runtime-event-queue.ts` micro-task batches. Hooks: `use-runtime-projection.ts`, `use-execution-mode-preview.ts`. |
| [`src/state/`](src/state/) | `bootstrap-store.ts` (boot phase, project list), `evolution-event-store.ts`. |
| [`src/stores/`](src/stores/) | `session-store.ts` (active session cursor only), `conversation-slice.ts` (per-session UI state — loading, todos, title, abort handles; **not** transcript truth), `chat-store.ts` (facade), `browser-slice.ts`. Goal: keep transcript and runtime facts in `runtimeProjectionStore` only. |
| [`src/modules/app-shell/`](src/modules/app-shell/) | `AppShell.tsx` (compose) + `ContentRouter.tsx` (`AppSection` → module). Pure render. |
| [`src/modules/chat/`](src/modules/chat/), [`src/modules/memory/`](src/modules/memory/), [`src/modules/settings/`](src/modules/settings/), [`src/modules/skills/`](src/modules/skills/), [`src/modules/jiaochang/`](src/modules/jiaochang/), [`src/modules/browser/`](src/modules/browser/), [`src/modules/onboarding/`](src/modules/onboarding/), [`src/modules/smart-browser/`](src/modules/smart-browser/), [`src/modules/git/`](src/modules/git/), [`src/modules/prompt-diagnostics/`](src/modules/prompt-diagnostics/), [`src/modules/execution-mode/`](src/modules/execution-mode/), [`src/modules/browser-viewer/`](src/modules/browser-viewer/) | Per-domain UI. |
| [`src/components/`](src/components/) | Shared components. **`src/components/ui/chat-ui.tsx` (~5K LOC) is a god-component** — do not add features here; extract pickers / slash UI / virtual list / file explorer / browser card into siblings. |
| [`src/lib/tauri.ts`](src/lib/tauri.ts) | Legacy DTO + transport bridge. Being progressively thinned; new code must not import DTOs from here. |

### Updater

Tauri updater endpoint is in `src-tauri/tauri.conf.json`. Default If2Ai release manifest URL is defined in `src-tauri/src/modules/updater/mod.rs` and overridable via `IF2AI_UPDATE_MANIFEST_URL`.

## Runtime Data Locations

User-level files live under `~/.if2ai/` (provider/model config, prompt control plane, identity pack, updater preferences, run logs at `runtime/run-log/<session>/<run>.jsonl`, supervisor snapshots at `runtime/supervisor/<session>.json`, pending permissions, session state). Use the current Rust config/storage modules as authoritative, not these notes.

## Authoritative References

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — full system architecture, data flows, vNext target, Teams design.
- [`AGENTS.md`](AGENTS.md) — agent-facing rules summary.
- [`docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`](docs/design-docs/if2ai-vnext-session-runtime-blueprint.md) — vNext north star.
- [`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md) — joint audit baseline (paired with ARCHITECTURE.md).
- [`docs/superpowers/plans/`](docs/superpowers/plans/) — dated plan files produced by `writing-plans`.

## Retired (Do Not Use)

- `docs/exec-plans/`, `docs/implementation-packs/` (old), `docs/refactor/` v1 — moved under `docs/_legacy/`. Do not read.
- `harness run --slice / --diff-gate / --review / --promote / --check-slice` — removed.
- The old 17-step SOP and the 5-step Pack pipeline — both replaced by the Superpowers SKILLS flow above.
- Legacy Tauri channels `agent-token`, `permission-request`, `memory_event`, `memory_after_turn` — retired in PR C-1/C-2/C-3/D-1 (2026-05-01 → 2026-05-02). Use `runtime_event` envelope only.
- Stale README claims of "Svelte frontend" or hosted vector DBs (Pinecone/Weaviate) — current stack is React + local SQLite/LanceDB.

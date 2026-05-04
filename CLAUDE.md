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
- `Session Supervisor` owns lifecycle (active run, cancel, resume, pending permission, retry).

New features should consume the typed API facade (`src/api/*`) and the projection read model — not raw `invoke` calls or new transcript stores. `src/App.tsx`, `src/components/ui/chat-ui.tsx`, and `src-tauri/src/modules/application/turn_service/stream_task.rs` are the priority split targets; avoid piling new logic onto them.

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
| `cargo fmt --check` / `cargo clippy -D warnings` | Required by Pack invariant I7 |
| `./scripts/pack run <PACK-ID>` | One-shot Pack pipeline: lint-architecture → snapshot → verify (build/test/clippy) → review prompt |
| `./scripts/pack verify <PACK-ID>` | Verify-only step |
| `npm run release:manifest:validate -- <manifest> stable` | Validate release manifest |

## Pack Workflow (mandatory)

This repo uses a **Pack-based** development pipeline as the only entry point. Every change goes through:

```
① PACK    a human writes one ≤ 60-line Pack file (design + spec + plan in one)
② BUILD   agent reads ONLY that one Pack + files it names
③④ RUN    ./scripts/pack run <PACK-ID>   (lint + snapshot + verify + review prompt)
⑤ COMMIT  one PR / one commit / update REGISTRY status
```

The whole cycle must complete in a single agent session. Three failures auto-STOP.

**Pack types:**
- `GFR-` Refactor Pack (`docs/packs/refactor/`) — zero behavior change. Snapshot diff is the core check. Function bodies must not change; only `use` paths and `pub(crate)` visibility may be adjusted to compile.
- `FEAT-` / `CPD-` Feature Pack (`docs/packs/feature/`) — must add ≥ 1 cargo test or e2e step covering each `## Spec` line.
- `BUG-` — first commit must be a failing regression test, then the fix (reviewer checks git timeline).
- `PERF-` — Pack must record `Baseline:` / `Target:`; commit message must include `Before:` / `After:`.
- `DEP-` — `## Files (scope)` may only contain lockfiles / package manifests; any business-code change forces a separate FEAT pack.

Source of truth: [docs/packs/CHARTER.md](./docs/packs/CHARTER.md). Active pack list: [docs/packs/REGISTRY.md](./docs/packs/REGISTRY.md).

## Hard Rules

- ❌ Do NOT browse `docs/design-docs/`, `docs/product-specs/`, or `docs/staff-remediation/` unless the active Pack names a specific path/section.
- ❌ Do NOT read anything under `docs/_legacy/` (frozen exec-plans, old implementation-packs, refactor-v1).
- ❌ Do NOT edit Pack files (humans write/edit them; agents only read).
- ❌ Do NOT bundle two Packs into one PR.
- ❌ No `unwrap()` / `expect()` / `todo!()` / `unimplemented!()` in non-test Rust code.
- ❌ No cross-module `use crate::xxx`; always go through `use crate::modules::*`.
- ❌ Do NOT add dependencies the Pack didn't declare.
- ❌ Do NOT bypass the `src/api/*` facade with raw `invoke` calls unless the Pack explicitly allows it.
- ✅ All `pub fn` need `///` doc comments. Async = tokio (no `std::thread`).
- ✅ Inside files listed under a Pack's `## Files (scope)`, any structural change is fine if Spec/Contract holds.

Lint contract details: [docs/references/coding-style-and-lint-contract.md](./docs/references/coding-style-and-lint-contract.md).

## File Size Targets

| Type | Target | Hard cap |
| --- | --- | --- |
| Backend Rust module | ≤ 500 lines | 800 lines (god-file watchlist beyond) |
| `.tsx` component | ≤ 300 lines | 500 lines |
| Frontend hook | ≤ 200 lines | 400 lines |
| Pack doc | ≤ 60 lines | 100 lines |

## Refactor Pack Invariants (checked by `scripts/pack verify`)

| ID | Invariant |
| --- | --- |
| I1 | Moved code's `pub` symbol set unchanged (signatures + names) |
| I2 | Test name set does not shrink; pass/fail does not regress |
| I3 | Event names / IPC command names / event string literals unchanged |
| I4 | Persistence keys unchanged (localStorage / sqlite columns / config fields) |
| I5 | God-file leaves `pub use` shim; caller import paths unchanged within the pack |
| I6 | Function bodies untouched (only `use`-path fixes allowed) |
| I7 | `cargo fmt --check` and `cargo clippy -D warnings` pass |

## Architecture Boundaries

### Backend (`src-tauri/src/modules/`)

- `application/` — TurnService and prompt/memory/provider/tool orchestration.
- `control_plane/` — pre-execution policy, context resolution, permission prep, audit.
- `runtime/` — event log, history, resume, pending permission, stream outcome (the truth source for the UI).
- `identity/` — Soul / Persona / ResolvedIdentity, identity injection.
- `memory/` — storage (SQLite + LanceDB), recall, compilation, promotion policy.
- `provider/` — provider registry, known models, capability probing, resilience.
- `tools/` — tool registry, toolset, builtin tools, execution outputs.
- `smart_browser/` — Smart Browser contract / policy / backend adapter (unifies local CDP, browser-use MCP, future cloud).
- `commands/` is the Tauri IPC layer — keep it thin.

### Frontend (`src/`)

- `api/` — typed Tauri command facades. **Pages depend on these, not raw `invoke`.**
- `runtime-projection/` — canonical run/event projection; the UI truth in vNext.
- `modules/app-shell/`, `modules/chat/`, `modules/settings/`, `modules/skills/`, `modules/jiaochang/`, `modules/browser/` — per-domain UI.
- `stores/` — selection pointers and bootstrap. Goal is to stop duplicating transcript/runtime facts here.

### Updater

Tauri updater endpoint is in `src-tauri/tauri.conf.json`. Default If2Ai release manifest URL is defined in `src-tauri/src/modules/updater/mod.rs` and overridable via `IF2AI_UPDATE_MANIFEST_URL`.

## Runtime Data Locations

User-level files live under `~/.if2ai/` (provider/model config, prompt control plane, identity pack, updater preferences, run logs, session state). Use the current Rust config/storage modules as authoritative, not these notes.

## Retired (Do Not Use)

- `docs/exec-plans/`, `docs/implementation-packs/` (old), `docs/refactor/` v1 — all moved under `docs/_legacy/`. Agents must not read them.
- `harness run --slice / --diff-gate / --review / --promote / --check-slice` — removed. Only `./scripts/pack suite <yaml>` (i.e. `harness run --suite`) remains as optional integration testing.
- The old 17-step SOP — replaced by the 5-step Pack pipeline.
- Stale README claims of "Svelte frontend" or hosted vector DBs (Pinecone/Weaviate) — current stack is React + local SQLite/LanceDB.

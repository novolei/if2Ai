# A.2 Daydream — Memory Consolidation Design Spec

> **Date**: 2026-05-08
> **Wave**: A.2 of memory + evolution roadmap (see [`2026-05-05-memory-evolution-product-replan.md`](../plans/2026-05-05-memory-evolution-product-replan.md))
> **Status**: design approved; ready for `writing-plans`
> **Depends on**: A.1 (PR-2 + PR-6a + PR-3 + PR-4 + PR-5) — schema columns, QualityScorer, ForgettingCurveEngine, ConflictResolver, ProceduralMemoryManager all live on vnext.

---

## 1. Purpose

Background memory consolidation — pruning low-value entries, deduping near-duplicates, and producing procedural insights from recent activity. Opt-in, runs on idle or on demand.

This is the orchestrator that wires the A.1 building blocks together into a single timed cycle.

## 2. Decisions (from 2026-05-08 brainstorm)

### 2.1 Default-off, opt-in

`enabled: false` by default. Users explicitly turn it on in Memory pane settings. Rationale: any background LLM cost should be a deliberate user choice, not a surprise on first launch.

### 2.2 Triggers — Manual + Idle (drop session-end)

Two triggers only:

- **Manual** — button in Memory pane → `commands/memory/daydream_run.rs`.
- **Idle** — supervisor timer fires after `idle_trigger_minutes` (default 30) of no user activity. Cancelled on any user message.

Session-end trigger from earlier draft is dropped — duplicates idle for the common case (user closes app == idle), and risks blocking shutdown.

### 2.3 Strategy enum is the budget knob

`llm_budget_tokens` field is dropped. The `ConsolidationStrategy` enum already encodes user intent; a separate budget field creates incoherence ("Conservative + 8000 tokens?"). Strategy → implicit budget mapping:

| Strategy | Steps | Approx. tokens / cycle |
|----------|-------|------------------------|
| `Conservative` | Prune only | 0 |
| `Balanced` (default) | Prune + Merge + Reflect | ~3000 |
| `Aggressive` | Prune + Merge + Refresh + Reflect | ~8000 |

Each step caps its own prompt budget by truncating input batches; cap exceeded → step short-circuits and reports the early-exit in the status row.

### 2.4 Default = Balanced

Opted-in user expects more than "decay sweep." Balanced delivers visible value (merge counts, new insights) at modest cost (~$0.004/cycle Haiku). Aggressive's Refresh step is least legible per token; Conservative under-delivers on opt-in signal.

### 2.5 Failure visibility — status row, no toasts

Memory pane settings shows a "Last cycle" row:
- Success: `2 min ago — pruned 12, merged 3, 1 insight`
- Failure: `5 min ago — failed: provider timeout`

No toasts (interruptive for non-critical bg work). Clicking the row opens cycle history (last N cycles, each with full `DayDreamReport`).

### 2.6 Internal safety cap

`max_entries_per_cycle: 100` — hard limit on entries any single step processes. Prevents runaway cost on first-cycle large memories. Not user-facing.

## 3. Config (final shape)

```rust
pub struct DayDreamConfig {
    pub enabled: bool,                   // default: false
    pub idle_trigger_minutes: u64,       // default: 30
    pub max_entries_per_cycle: usize,    // default: 100
    pub strategy: ConsolidationStrategy, // default: Balanced
}

pub enum ConsolidationStrategy {
    Conservative,
    Balanced,
    Aggressive,
}
```

Dropped from earlier seed (`memory/daydream/report.rs`): `session_end_trigger`, `llm_budget_tokens`.

## 4. Pipeline

```
DayDreamEngine::run_cycle(strategy) -> DayDreamReport
  │
  ├─ Step 1: Prune  (always)
  │    └─ ForgettingCurveEngine::sweep — already shipped in PR-3
  │
  ├─ Step 2: Merge  (Balanced + Aggressive)
  │    └─ Find near-duplicates by embedding cosine ≥ 0.92
  │    └─ LLM fan-in summarization → ConflictResolver::resolve
  │
  ├─ Step 3: Refresh  (Aggressive only)
  │    └─ Re-embed entries with last_validated_at older than 30d
  │
  └─ Step 4: Reflect  (Balanced + Aggressive)
       └─ SelfReflector — already shipped in PR-5
       └─ ProceduralMemoryManager::store_insight on confidence ≥ 0.7
```

Steps 5 (Cognitive-enforce) and 6 (Graph-discover) from the original 6-step replan are **deferred** — no consumer.

Each step takes a `&mut DayDreamReport` and appends counters / errors. Errors in one step do not abort the cycle; they're recorded and the next step proceeds. Cycle is atomic per step (no partial-mutation rollback).

## 5. Triggering

### 5.1 Idle timer

`runtime/supervisor` already tracks last-activity timestamp per session. Add:

```rust
// supervisor.rs
async fn maybe_fire_daydream(&self) {
    if !self.daydream_config.enabled { return; }
    let idle = self.last_activity.elapsed();
    if idle >= Duration::from_secs(self.daydream_config.idle_trigger_minutes * 60) {
        self.daydream_engine.run_cycle_async();
    }
}
```

Wire into existing supervisor tick (no new task). Any user message resets `last_activity`, which naturally cancels the next-tick trigger.

### 5.2 Manual

```rust
// commands/memory/daydream_run.rs
#[tauri::command]
pub async fn daydream_run_cycle(state: State<AppState>) -> Result<DayDreamReport, ...>
```

Bypasses idle check; runs immediately with current strategy.

### 5.3 Concurrency

Engine holds a `tokio::Mutex<Option<RunningCycle>>`. Second trigger while one is running → returns "already running" error to manual trigger / silent no-op for idle.

## 6. Status row & event surface

New `runtime_event` variant:

```rust
RuntimeEventPayload::DaydreamCycleCompleted {
    report: DayDreamReport,
}
```

Frontend `runtime-projection` adds a `daydreamHistory: DayDreamReport[]` slice (capped at last 20 cycles). Memory pane settings reads via `useDaydreamHistory()` hook.

The status row component (`src/modules/memory/daydream-status-row.tsx`) renders the most recent report. Click → expand into history modal.

## 7. Components affected

| File | Δ scope | LOC est. |
|------|---------|----------|
| `src-tauri/src/modules/memory/daydream/report.rs` | Trim config (drop 2 fields), keep `DayDreamReport` struct | ~−15 net |
| `src-tauri/src/modules/memory/daydream/engine.rs` | NEW — orchestrator, `run_cycle`, mutex, dispatch by strategy | +220 |
| `src-tauri/src/modules/memory/daydream/prune.rs` | NEW — thin wrapper over `ForgettingCurveEngine::sweep` | +60 |
| `src-tauri/src/modules/memory/daydream/merge.rs` | NEW — embedding similarity scan + LLM fan-in + ConflictResolver call | +180 |
| `src-tauri/src/modules/memory/daydream/refresh.rs` | NEW — re-embed stale entries (Aggressive only) | +90 |
| `src-tauri/src/modules/memory/daydream/reflect.rs` | NEW — SelfReflector → ProceduralMemoryManager wiring | +110 |
| `src-tauri/src/modules/memory/daydream/mod.rs` | Re-export engine + report | +12 |
| `src-tauri/src/modules/runtime/supervisor.rs` | Idle-timer hook + `maybe_fire_daydream` | +35 |
| `src-tauri/src/modules/runtime/contracts/common.rs` | Add `DaydreamCycleCompleted` payload variant | +20 |
| `src-tauri/src/commands/memory/daydream_run.rs` | NEW — manual trigger command | +35 |
| `src-tauri/src/commands/command_surface.rs` | Register new command | +2 |
| `src-tauri/src/bootstrap/memory.rs` | Construct `DayDreamEngine`, wire into AppState | +25 |
| `src-tauri/src/bootstrap/app.rs` | Pass config from settings to supervisor | +8 |
| `src/transport/contracts.ts` | Mirror `DaydreamCycleCompleted` payload | +18 |
| `src/runtime-projection/runtime-event-translator.ts` | Route into `daydreamHistory` slice | +25 |
| `src/runtime-projection/runtime-event-reducer.ts` | History slice (cap 20) | +30 |
| `src/api/memory.ts` | `runDaydreamCycle()` facade | +20 |
| `src/modules/memory/daydream-status-row.tsx` | NEW component | +120 |
| `src/modules/memory/daydream-history-modal.tsx` | NEW component | +90 |
| `src/modules/memory/memory-settings-panel.tsx` | Add enable toggle, strategy dropdown, status row mount | +40 |

**Total**: ~1100 LOC across 19 files.

## 8. Tests

### 8.1 Backend unit

- `prune::tests::respects_max_entries_per_cycle`
- `merge::tests::skips_when_no_pair_exceeds_threshold`
- `merge::tests::short_circuits_at_token_cap`
- `refresh::tests::skips_recent_entries`
- `reflect::tests::filters_low_confidence_insights`
- `engine::tests::strategy_conservative_runs_only_prune`
- `engine::tests::strategy_balanced_runs_prune_merge_reflect`
- `engine::tests::concurrent_trigger_returns_already_running`
- `engine::tests::step_failure_does_not_abort_cycle`

### 8.2 Backend integration

- `supervisor::tests::idle_threshold_fires_daydream_when_enabled`
- `supervisor::tests::user_activity_resets_idle_timer`
- `supervisor::tests::disabled_config_never_fires`

### 8.3 Frontend

- `runtime-event-reducer.test.ts::daydream_history_caps_at_20`
- `daydream-status-row.test.tsx::renders_success_counts`
- `daydream-status-row.test.tsx::renders_failure_reason`

## 9. Out of scope (deferred)

- Cognitive-layer enforcement step (no consumer)
- Graph-discovery step (no graph engine)
- Session-end trigger (covered by idle)
- LLM-based extraction in Reflect (PR-5 ships rule-based; LLM upgrade is Wave A.5)
- Configurable prompts per step (use defaults, revisit via telemetry)
- Cycle-level rollback / atomicity (per-step granularity is enough)
- Cost telemetry surfaced to user (status row shows counts, not dollars; can layer later)

## 10. Acceptance criteria

A.2 ships when:
- `cargo check --workspace` clean on vnext
- `cargo test --lib memory::daydream` passes (full suite above)
- `cargo test --lib runtime::supervisor` passes (idle-fire tests)
- `npm test` passes (reducer + component tests)
- `npm run build:web` clean
- Manual smoke: enable in settings → wait 30 min idle → status row shows last cycle with non-zero prune count
- Manual smoke: trigger via button → report visible in history modal
- Disabled config (default) — no idle activity, no commands fired, zero LLM calls

## 11. Open follow-ups (not blocking)

- Telemetry dashboard for daydream cost per session (Wave A.4 UI work)
- Adaptive idle threshold (Q-learning over user response patterns) — research-stage
- Per-strategy cost telemetry to inform default-strategy tuning

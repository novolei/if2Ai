# A.2-Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the inert A.2 daydream module to AppState, expose a manual trigger, persist the user toggle, and surface the status row in the Memory pane. Modules from PR #2 (commits `05a8586`..`52a3113`) are already on `vnext-base-2026-05-08`.

**Architecture:** Bootstrap constructs `DayDreamEngine` + `DayDreamCoordinator` after `memory_provider` is built; threads coordinator through `MemoryBootstrap` → `AppStateConfig` → `AppState`. Tauri `setup` hook spawns the poll loop with an `AppHandle` closure that emits `RuntimeEventType::DaydreamCycle` envelopes. `turn_service` calls `coordinator.note_activity()` at run + stream entry. Frontend reduces cycle events into a cap-20 history slice and renders a settings-panel status row.

**Tech Stack:** Rust 2021 / Tokio / Tauri 2 / React+TS / Vite. Same stack as A.2.

---

## Brainstorm decisions (2026-05-08)

- **Q1 = A (stub TrajectorySource)** — reflect step ships inert (`mutated: 0`) until Wave A.3 builds a real producer for `evolution::Trajectory`. The two `Trajectory` types (ShareGPT-shaped `learning::trajectory::Trajectory` vs task-execution-shaped `evolution::trajectory::Trajectory`) are structurally incompatible; a lossy adapter would feed garbage insights into procedural memory and pollute agent behavior. Stub is honest.
- **Q2 = B (JSON file at `~/.if2ai/daydream.json`)** — small, isolated, mirrors existing `~/.if2ai/*.json` pattern (browser-cold-state.json, etc.). Read on boot, write on toggle.
- **Q3 = B (bump activity at both turn_service entry points)** — `run.rs::run` (non-streaming) and `stream_task.rs::run_stream_task` (streaming, the common path).

---

## File map

**Backend new**
| Path | Responsibility |
|------|----------------|
| `src-tauri/src/modules/memory/daydream/trajectory_source.rs` | `EmptyTrajectorySource` stub |
| `src-tauri/src/bootstrap/daydream_config.rs` | Read/write `~/.if2ai/daydream.json` |
| `src-tauri/src/commands/memory/daydream.rs` | `daydream_run_cycle` + `daydream_get_config` + `daydream_set_config` |

**Backend modified**
| Path | Change |
|------|--------|
| `src-tauri/src/modules/memory/daydream/mod.rs` | Re-export trajectory stub |
| `src-tauri/src/bootstrap/memory.rs` | Construct engine + coordinator; expose on `MemoryBootstrap` |
| `src-tauri/src/bootstrap/app.rs` | Thread coordinator into `AppStateConfig` |
| `src-tauri/src/commands/mod.rs` | `AppStateConfig` + `AppState` get `daydream_coordinator` field |
| `src-tauri/src/commands/memory/mod.rs` | `pub mod daydream;` |
| `src-tauri/src/commands/command_surface.rs` | Register 3 new commands |
| `src-tauri/src/main.rs` | `.setup(|app| spawn_poll_loop(...))` |
| `src-tauri/src/modules/application/turn_service/run.rs` | One `note_activity()` call at entry |
| `src-tauri/src/modules/application/turn_service/stream_task.rs` | One `note_activity()` call at stream-task entry |

**Frontend new**
| Path | Responsibility |
|------|----------------|
| `src/modules/memory/daydream/use-daydream-history.ts` | Hook into projection slice |
| `src/modules/memory/daydream/daydream-status-row.tsx` | Last-cycle row + "Run now" |
| `src/modules/memory/daydream/daydream-history-modal.tsx` | Last-20 cycles |

**Frontend modified**
| Path | Change |
|------|--------|
| `src/transport/contracts.ts` | `'daydream_cycle'` event-type union + `DaydreamReportPayload` interface |
| `src/runtime-projection/runtime-event-translator.ts` | Route `daydream_cycle` family |
| `src/runtime-projection/runtime-event-reducer.ts` | `daydreamHistory: DaydreamReportPayload[]` slice (cap 20) |
| `src/api/memory.ts` | `runDaydreamCycle()` / `getDaydreamConfig()` / `setDaydreamConfig()` |
| `src/modules/memory/memory-settings-panel.tsx` | Mount status row + enable toggle + strategy dropdown |

---

## Task 1: Stub `TrajectorySource`

**Files:** Create `src-tauri/src/modules/memory/daydream/trajectory_source.rs`; modify `mod.rs`.

- [ ] **Step 1.1: Create stub**

```rust
//! Stub `TrajectorySource` — honest no-op until Wave A.3 ships a real
//! `evolution::Trajectory` producer in turn_service.

use async_trait::async_trait;

use crate::modules::memory::evolution::trajectory::Trajectory;

use super::engine::TrajectorySource;

pub struct EmptyTrajectorySource;

#[async_trait]
impl TrajectorySource for EmptyTrajectorySource {
    async fn recent_trajectories(&self, _max: usize) -> Result<Vec<Trajectory>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_source_returns_empty_vec() {
        let s = EmptyTrajectorySource;
        let v = s.recent_trajectories(100).await.unwrap();
        assert!(v.is_empty());
    }
}
```

- [ ] **Step 1.2: Re-export in `mod.rs`**

Append to `src-tauri/src/modules/memory/daydream/mod.rs`:
```rust
pub mod trajectory_source;
pub use trajectory_source::EmptyTrajectorySource;
```

- [ ] **Step 1.3: Test + commit**

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
cargo test --manifest-path src-tauri/Cargo.toml --lib memory::daydream::trajectory_source
git add src-tauri/src/modules/memory/daydream/
git commit -m "feat(memory/daydream): add EmptyTrajectorySource stub (A.2-wiring Task 1)"
```

---

## Task 2: Config persistence at `~/.if2ai/daydream.json`

**Files:** Create `src-tauri/src/bootstrap/daydream_config.rs`; modify `bootstrap/mod.rs`.

- [ ] **Step 2.1: Create persistence helpers**

```rust
//! Read/write `DayDreamConfig` at `<if2ai_dir>/daydream.json`.

use std::fs;
use std::path::Path;

use crate::modules::memory::daydream::DayDreamConfig;

const FILENAME: &str = "daydream.json";

/// Read the persisted config. Returns `DayDreamConfig::default()` (`enabled: false`)
/// if the file is missing or invalid — matches the opt-in safety contract.
pub fn load(if2ai_dir: &Path) -> DayDreamConfig {
    let path = if2ai_dir.join(FILENAME);
    let Ok(raw) = fs::read_to_string(&path) else {
        return DayDreamConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "[init] daydream.json invalid; falling back to defaults"
        );
        DayDreamConfig::default()
    })
}

/// Persist the config. Best-effort: returns `Err(...)` so the command
/// layer can surface a toast, but the in-memory engine still updates.
pub fn save(if2ai_dir: &Path, cfg: &DayDreamConfig) -> std::io::Result<()> {
    let path = if2ai_dir.join(FILENAME);
    let raw = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let cfg = load(dir.path());
        assert_eq!(cfg, DayDreamConfig::default());
    }

    #[test]
    fn round_trip_preserves_fields() {
        let dir = tempdir().unwrap();
        let cfg = DayDreamConfig {
            enabled: true,
            idle_trigger_minutes: 45,
            max_entries_per_cycle: 50,
            strategy: crate::modules::memory::daydream::ConsolidationStrategy::Aggressive,
        };
        save(dir.path(), &cfg).unwrap();
        let read = load(dir.path());
        assert_eq!(read, cfg);
    }
}
```

- [ ] **Step 2.2: Register module**

In `src-tauri/src/bootstrap/mod.rs`, add (alphabetical):
```rust
pub mod daydream_config;
```

- [ ] **Step 2.3: Test + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib bootstrap::daydream_config
git add src-tauri/src/bootstrap/
git commit -m "feat(bootstrap): persist DayDreamConfig at ~/.if2ai/daydream.json (A.2-wiring Task 2)"
```

---

## Task 3: Construct engine + coordinator in bootstrap

**Files:** Modify `src-tauri/src/bootstrap/memory.rs`.

The constructor needs `QualityScorer`, `ForgettingCurveEngine`, `SelfReflector`, `ProceduralMemoryManager`, `EmptyTrajectorySource`, the existing `utility_llm`, and the persisted config.

- [ ] **Step 3.1: Extend `MemoryBootstrap`**

Add field:
```rust
pub daydream_coordinator: std::sync::Arc<crate::modules::memory::daydream::DayDreamCoordinator>,
```

- [ ] **Step 3.2: Construct after `memory_provider` exists** (around line 160 of `memory.rs`, after the ticker is finalized)

```rust
// A.2 — daydream consolidation engine.
let daydream_config = crate::bootstrap::daydream_config::load(&paths.if2ai_dir);
let daydream_scorer = std::sync::Arc::new(modules::memory::quality::QualityScorer::new());
let daydream_forgetting =
    std::sync::Arc::new(modules::memory::forgetting::ForgettingCurveEngine::new());
let daydream_reflector = std::sync::Arc::new(
    modules::memory::evolution::reflector::SelfReflector::new(memory_provider.clone())
        .with_llm(utility_llm.clone()),
);
let daydream_procedural = std::sync::Arc::new(
    modules::memory::evolution::procedural::ProceduralMemoryManager::new(memory_provider.clone()),
);
let daydream_trajectories: std::sync::Arc<dyn modules::memory::daydream::TrajectorySource> =
    std::sync::Arc::new(modules::memory::daydream::EmptyTrajectorySource);
let daydream_engine = std::sync::Arc::new(modules::memory::daydream::DayDreamEngine::new(
    memory_provider.clone(),
    daydream_scorer,
    daydream_forgetting,
    utility_llm.clone(),
    daydream_reflector,
    daydream_procedural,
    daydream_trajectories,
    daydream_config,
));
let daydream_coordinator =
    std::sync::Arc::new(modules::memory::daydream::DayDreamCoordinator::new(daydream_engine));
```

- [ ] **Step 3.3: Add to struct return**

```rust
MemoryBootstrap {
    // ...existing fields...
    daydream_coordinator,
}
```

- [ ] **Step 3.4: Build + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/bootstrap/memory.rs
git commit -m "feat(bootstrap): construct DayDreamEngine + Coordinator in MemoryBootstrap (A.2-wiring Task 3)"
```

> **Verify:** `MemoryProvider::clone()` is `Arc::clone` (since `SharedMemoryProvider = Arc<dyn MemoryProvider>`). `utility_llm.clone()` is also Arc-clone. No additional cargo deps needed.

---

## Task 4: Thread coordinator through AppState

**Files:** Modify `src-tauri/src/bootstrap/app.rs`, `src-tauri/src/commands/mod.rs`.

- [ ] **Step 4.1: Add field to `AppStateConfig`**

In `src-tauri/src/commands/mod.rs`, find `pub struct AppStateConfig` (use `grep -n "pub struct AppStateConfig" src-tauri/src/commands/mod.rs` to locate). Add:

```rust
pub daydream_coordinator: std::sync::Arc<crate::modules::memory::daydream::DayDreamCoordinator>,
```

In the same file find `pub struct AppState`. Add the same field. In `AppState::new` (or whichever constructor copies fields out of `AppStateConfig`), copy the coordinator across.

- [ ] **Step 4.2: Pass through in `app.rs`**

In `src-tauri/src/bootstrap/app.rs`, in the `AppBootstrap { app_state_config: commands::AppStateConfig { ... } }` block, add:

```rust
daydream_coordinator: memory_bootstrap.daydream_coordinator.clone(),
```

- [ ] **Step 4.3: Build + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/bootstrap/app.rs src-tauri/src/commands/mod.rs
git commit -m "feat(commands): thread daydream_coordinator through AppStateConfig + AppState (A.2-wiring Task 4)"
```

---

## Task 5: Manual + config commands

**Files:** Create `src-tauri/src/commands/memory/daydream.rs`; modify `commands/memory/mod.rs`, `commands/command_surface.rs`.

- [ ] **Step 5.1: Create command file**

```rust
//! A.2 daydream Tauri commands — manual trigger + config get/set.

use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::daydream::{DayDreamConfig, DayDreamReport};

/// Trigger one daydream cycle on demand. Bypasses the idle gate.
#[tauri::command]
pub async fn daydream_run_cycle(state: State<'_, AppState>) -> Result<DayDreamReport, String> {
    state
        .daydream_coordinator
        .trigger_manual()
        .await
        .map_err(|e| format!("daydream cycle failed: {e}"))
}

/// Read the persisted config from `~/.if2ai/daydream.json`. Returns
/// defaults (`enabled: false`) when the file is missing.
#[tauri::command]
pub async fn daydream_get_config(state: State<'_, AppState>) -> Result<DayDreamConfig, String> {
    let _ = state; // path comes from PROCESS_RUNTIME paths; load via shared helper below
    Ok(crate::bootstrap::daydream_config::load(
        &crate::bootstrap::current_if2ai_dir(),
    ))
}

/// Persist the config. The in-memory engine config does NOT update until
/// next launch — this is intentional to keep the engine immutable per
/// boot. Toggling `enabled` to `false` takes effect at next startup.
/// (If live-toggling is needed later, expose a setter on the coordinator.)
#[tauri::command]
pub async fn daydream_set_config(
    state: State<'_, AppState>,
    config: DayDreamConfig,
) -> Result<(), String> {
    let _ = state;
    crate::bootstrap::daydream_config::save(&crate::bootstrap::current_if2ai_dir(), &config)
        .map_err(|e| format!("write daydream.json failed: {e}"))
}
```

> **Note:** `bootstrap::current_if2ai_dir()` may not exist. Verify with `grep -n "fn current_if2ai_dir\|IF2AI_DIR\|paths.if2ai_dir" src-tauri/src/bootstrap/mod.rs`. If absent, options:
> 1. Add a `pub fn if2ai_dir() -> PathBuf` in `bootstrap/mod.rs` that resolves via `dirs::home_dir().join(".if2ai")` (mirroring `load_context_budget` pattern in `app.rs:140-143`).
> 2. Stash the path in `AppStateConfig` as a new field `if2ai_dir: PathBuf` and read it via `state.if2ai_dir`.
>
> Pick (1) — minimal change.

- [ ] **Step 5.2: Re-export and register**

In `src-tauri/src/commands/memory/mod.rs`:
```rust
pub mod daydream;
pub use daydream::{daydream_get_config, daydream_run_cycle, daydream_set_config};
```

In `src-tauri/src/commands/command_surface.rs`, locate the `tauri::generate_handler!` block (around line 328). Add three lines:
```rust
            crate::commands::memory::daydream_run_cycle,
            crate::commands::memory::daydream_get_config,
            crate::commands::memory::daydream_set_config,
```

- [ ] **Step 5.3: Build + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/commands/ src-tauri/src/bootstrap/mod.rs
git commit -m "feat(commands/daydream): manual trigger + config get/set commands (A.2-wiring Task 5)"
```

---

## Task 6: Spawn poll loop in `setup` hook

**Files:** Modify `src-tauri/src/main.rs`.

- [ ] **Step 6.1: Update setup**

In `src-tauri/src/main.rs`, replace the `.setup(...)` line:

```rust
.setup({
    let coord = host_composition.app_state.daydream_coordinator.clone();
    move |app| {
        modules::desktop_host::setup_desktop_host(app)?;
        let app_handle = app.handle().clone();
        modules::memory::daydream::spawn_poll_loop(coord, move |report| {
            let family = if report.all_succeeded() {
                modules::runtime::contracts::common::daydream_family::COMPLETED
            } else {
                modules::runtime::contracts::common::daydream_family::FAILED
            };
            let correlation =
                modules::runtime::contracts::common::CorrelationIds::default();
            let _ = modules::runtime::runtime_event::dispatch(
                Some(&app_handle),
                modules::runtime::contracts::common::RuntimeEventType::DaydreamCycle,
                family,
                correlation,
                &report,
                None,
            );
        });
        Ok(())
    }
})
```

> **Verify:** `host_composition.app_state` exposes `daydream_coordinator` directly. If `app_state` is wrapped (e.g. `Arc<AppState>`), unwrap with `.daydream_coordinator.clone()` accordingly. The `app.handle().clone()` API matches Tauri 2.

- [ ] **Step 6.2: Build + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/main.rs
git commit -m "feat(main): spawn daydream poll loop in setup hook (A.2-wiring Task 6)"
```

---

## Task 7: Activity bump in `turn_service`

**Files:** Modify `src-tauri/src/modules/application/turn_service/run.rs`, `src-tauri/src/modules/application/turn_service/stream_task.rs`.

- [ ] **Step 7.1: Locate entry points**

```bash
grep -n "pub async fn run\b\|pub async fn run_stream_task" src-tauri/src/modules/application/turn_service/run.rs src-tauri/src/modules/application/turn_service/stream_task.rs
```

The non-streaming entry is `turn_service::run::run`. The streaming entry is `turn_service::stream_task::run_stream_task` (verify exact name). Both take some form of `&AppState` or `&Arc<ServiceRegistry>` and the request.

- [ ] **Step 7.2: Add bump at top of each function body**

```rust
state.daydream_coordinator.note_activity().await;
```

If the function takes `&AppState` directly, that compiles as written. If it takes a partial slice (e.g. only `service_registry`), you may need to thread `daydream_coordinator: Arc<DayDreamCoordinator>` through the call site. Prefer not threading — instead, find the call site that has `&AppState` and bump there. Most likely: the Tauri command at `commands/conversations.rs` (or similar) that invokes `turn_service::run` is the cleaner bump point.

> **Pragmatic fallback:** if turn_service signatures don't have AppState handy, bump in the **command-level entry** that invokes turn_service. One bump per command (`start_chat_turn`, `start_agent_stream`, etc.) is functionally equivalent and avoids signature churn.

- [ ] **Step 7.3: Build + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/modules/application/turn_service/ src-tauri/src/commands/
git commit -m "feat(turn_service): bump daydream activity stamp on run + stream entry (A.2-wiring Task 7)"
```

---

## Task 8: Frontend transport + projection slice

**Files:** Modify `src/transport/contracts.ts`, `src/runtime-projection/runtime-event-translator.ts`, `src/runtime-projection/runtime-event-reducer.ts`, `src/api/memory.ts`.

- [ ] **Step 8.1: Mirror payload type**

In `src/transport/contracts.ts`, locate the `RuntimeEventType` string union (search `'daemon_health'` to find it). Add `'daydream_cycle'`. Append at file bottom:

```ts
export interface DaydreamStepOutcome {
  step: 'prune' | 'merge' | 'refresh' | 'reflect';
  examined: number;
  mutated: number;
  durationMs: number;
  error: { kind: string; message: string } | null;
}

export interface DaydreamReportPayload {
  cycleId: string;
  trigger: 'idle' | 'manual';
  strategy: 'conservative' | 'balanced' | 'aggressive';
  startedAt: string;
  finishedAt: string;
  steps: DaydreamStepOutcome[];
}

export interface DaydreamConfig {
  enabled: boolean;
  idleTriggerMinutes: number;
  maxEntriesPerCycle: number;
  strategy: 'conservative' | 'balanced' | 'aggressive';
}
```

> Verify the project's existing camelCase / snake_case convention for serde. Rust `#[serde(rename_all = "camelCase")]` on `DayDreamReport` produces `cycleId`, etc. — already correct in this plan.
>
> For `DayDreamConfig`, the Rust struct has `idle_trigger_minutes` and is **not** `#[serde(rename_all = "camelCase")]`. Either: (a) add `#[serde(rename_all = "camelCase")]` to `DayDreamConfig` in `memory/daydream/config.rs` (one-line change, has no negative impact since the field names are local), or (b) match snake_case in TS. Pick (a) — TS-side cleaner.

- [ ] **Step 8.2: Reducer slice**

In `src/runtime-projection/runtime-event-reducer.ts`, find the snapshot interface (search `interface RuntimeProjectionSnapshot` or similar). Add field:
```ts
daydreamHistory: DaydreamReportPayload[];
```

In the initial-state factory: `daydreamHistory: []`.

In the reducer switch (or family-routing function — adapt to the file's actual pattern):
```ts
case 'daydream_cycle': {
  const payload = action.payload as DaydreamReportPayload;
  const next = [payload, ...state.daydreamHistory];
  return { ...state, daydreamHistory: next.slice(0, 20) };
}
```

- [ ] **Step 8.3: Translator route**

In `src/runtime-projection/runtime-event-translator.ts`, add a case for `daydream_cycle` that dispatches the reducer action. Match the pattern of an existing family (e.g. `daemon_health`).

- [ ] **Step 8.4: API facade**

In `src/api/memory.ts`:

```ts
import type { DaydreamConfig, DaydreamReportPayload } from '@/transport/contracts';
import { invoke } from './client';

export async function runDaydreamCycle(): Promise<DaydreamReportPayload> {
  return invoke<DaydreamReportPayload>('daydream_run_cycle');
}

export async function getDaydreamConfig(): Promise<DaydreamConfig> {
  return invoke<DaydreamConfig>('daydream_get_config');
}

export async function setDaydreamConfig(config: DaydreamConfig): Promise<void> {
  return invoke<void>('daydream_set_config', { config });
}
```

- [ ] **Step 8.5: Build + commit**

```bash
npm run build:web
git add src/transport/contracts.ts src/runtime-projection/ src/api/memory.ts \
        src-tauri/src/modules/memory/daydream/config.rs   # if camelCase serde added
git commit -m "feat(web/daydream): transport + projection slice + api facade (A.2-wiring Task 8)"
```

---

## Task 9: UI — status row + history modal + settings panel mount

**Files:** Create three files; modify `memory-settings-panel.tsx`.

- [ ] **Step 9.1: History hook**

Create `src/modules/memory/daydream/use-daydream-history.ts`:

```ts
import { useRuntimeProjection } from '@/runtime-projection/use-runtime-projection';

export function useDaydreamHistory() {
  return useRuntimeProjection((s) => s.daydreamHistory);
}
```

> Verify the hook name. If it's `useRuntimeProjectionSelector` instead, adapt accordingly. Search: `grep -rn "useRuntimeProjection" src/runtime-projection/ | head -3`.

- [ ] **Step 9.2: Status row**

Create `src/modules/memory/daydream/daydream-status-row.tsx`:

```tsx
import { useState } from 'react';
import { useDaydreamHistory } from './use-daydream-history';
import { DaydreamHistoryModal } from './daydream-history-modal';
import { runDaydreamCycle } from '@/api/memory';

export function DaydreamStatusRow() {
  const history = useDaydreamHistory();
  const [open, setOpen] = useState(false);
  const last = history[0];

  let summary: string;
  if (!last) {
    summary = 'Never run';
  } else if (last.steps.some((s) => s.error)) {
    const failed = last.steps.find((s) => s.error)!;
    summary = `failed: ${failed.error!.message}`;
  } else {
    const counts = last.steps
      .filter((s) => s.mutated > 0 || s.examined > 0)
      .map((s) => `${s.step} ${s.mutated}/${s.examined}`)
      .join(', ');
    summary = counts || 'no-op';
  }

  return (
    <div className="daydream-status-row" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <button onClick={() => setOpen(true)} style={{ flex: 1, textAlign: 'left' }}>
        <span style={{ opacity: 0.6 }}>Last cycle: </span>
        <span>{summary}</span>
      </button>
      <button onClick={() => runDaydreamCycle().catch(console.error)}>Run now</button>
      {open && <DaydreamHistoryModal onClose={() => setOpen(false)} history={history} />}
    </div>
  );
}
```

- [ ] **Step 9.3: History modal**

Create `src/modules/memory/daydream/daydream-history-modal.tsx`:

```tsx
import type { DaydreamReportPayload } from '@/transport/contracts';

interface Props {
  history: DaydreamReportPayload[];
  onClose: () => void;
}

export function DaydreamHistoryModal({ history, onClose }: Props) {
  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
        display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'var(--surface)', maxWidth: 640, width: '90%',
          maxHeight: '80vh', overflow: 'auto', padding: 16, borderRadius: 8,
        }}
      >
        <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <h3 style={{ margin: 0 }}>Daydream history</h3>
          <button onClick={onClose}>×</button>
        </header>
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {history.length === 0 && <li style={{ opacity: 0.6 }}>No cycles yet.</li>}
          {history.map((r) => {
            const failed = r.steps.some((s) => s.error);
            return (
              <li
                key={r.cycleId}
                style={{
                  padding: 8, borderTop: '1px solid var(--border)',
                  color: failed ? 'var(--error)' : undefined,
                }}
              >
                <div style={{ fontSize: 12, opacity: 0.7 }}>
                  {new Date(r.finishedAt).toLocaleString()} — {r.trigger} — {r.strategy}
                </div>
                <ol style={{ margin: '4px 0 0 16px' }}>
                  {r.steps.map((s) => (
                    <li key={s.step}>
                      {s.step}: examined {s.examined} / mutated {s.mutated} ({s.durationMs}ms)
                      {s.error ? ` — ${s.error.kind}: ${s.error.message}` : ''}
                    </li>
                  ))}
                </ol>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
```

- [ ] **Step 9.4: Settings panel mount**

In `src/modules/memory/memory-settings-panel.tsx`, add at the top:
```tsx
import { useEffect, useState } from 'react';
import { DaydreamStatusRow } from './daydream/daydream-status-row';
import { getDaydreamConfig, setDaydreamConfig } from '@/api/memory';
import type { DaydreamConfig } from '@/transport/contracts';
```

Inside the component (after existing state), add:
```tsx
const [daydream, setDaydreamLocal] = useState<DaydreamConfig | null>(null);

useEffect(() => {
  getDaydreamConfig().then(setDaydreamLocal).catch(console.error);
}, []);

const updateDaydream = (patch: Partial<DaydreamConfig>) => {
  if (!daydream) return;
  const next = { ...daydream, ...patch };
  setDaydreamLocal(next);
  setDaydreamConfig(next).catch(console.error);
};
```

In the render (next to other settings sections), add:
```tsx
{daydream && (
  <section className="settings-section">
    <h4>Daydream consolidation</h4>
    <p style={{ opacity: 0.7, fontSize: 12 }}>
      Background pruning + dedup that runs after 30 min of inactivity. Off by default.
    </p>
    <label>
      <input
        type="checkbox"
        checked={daydream.enabled}
        onChange={(e) => updateDaydream({ enabled: e.target.checked })}
      />
      Enable (takes effect at next launch)
    </label>
    <label style={{ display: 'block', marginTop: 8 }}>
      Strategy:{' '}
      <select
        value={daydream.strategy}
        onChange={(e) =>
          updateDaydream({
            strategy: e.target.value as 'conservative' | 'balanced' | 'aggressive',
          })
        }
      >
        <option value="conservative">Conservative — prune only</option>
        <option value="balanced">Balanced — prune + merge + reflect (recommended)</option>
        <option value="aggressive">Aggressive — adds refresh</option>
      </select>
    </label>
    <DaydreamStatusRow />
  </section>
)}
```

> Verify the panel's existing styling pattern (CSS-modules vs inline vs class names) and adapt; the inline styles above are placeholder — match what's already in the file.

- [ ] **Step 9.5: Build + commit**

```bash
npm run build:web
git add src/modules/memory/
git commit -m "feat(web/daydream): status row + history modal + settings toggle (A.2-wiring Task 9)"
```

---

## Acceptance criteria

- [ ] `cargo fmt --check --manifest-path src-tauri/Cargo.toml` (allow pre-existing baseline drift)
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` (allow pre-existing `redundant_closure` in `context_compression/mod.rs`)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --lib memory::daydream bootstrap::daydream_config` passes
- [ ] `npm test` passes
- [ ] `npm run build:web` clean
- [ ] **Manual smoke (default-off)**: launch fresh — no daydream activity in `~/.if2ai/runtime/run-log/*.jsonl` after 31 min idle (`daydream.json` absent → defaults to `enabled: false`)
- [ ] **Manual smoke (opt-in)**: settings → enable → restart → wait 30 min → status row shows last cycle with non-zero counts
- [ ] **Manual smoke (manual)**: click "Run now" → `daydreamHistory` slice updates → modal renders the cycle

---

## Out of scope (kept deferred)

- Real `evolution::Trajectory` producer (Wave A.3)
- Live-toggling enabled/strategy without restart (engine config is immutable per boot today)
- Cost-in-dollars telemetry in status row (counts only)
- Cognitive-layer enforcement step
- Graph-discovery step
- Refresh step real re-embedding (Wave A.5 once embedding-update API exists)

---

## Self-review

- **Spec coverage:** every Tasks-9-11 item in the original A.2 plan + the 7 deviations in §"Execution status" gets a task. The 3 brainstorm decisions (Q1=A stub, Q2=B JSON file, Q3=B both bumps) are explicit.
- **Type consistency:** `DaydreamReportPayload` (TS) ↔ `DayDreamReport` (Rust, camelCase serde already on the struct). `DaydreamConfig` (TS) requires the one-line `#[serde(rename_all = "camelCase")]` add in Task 8.1 — flagged inline. `daydream_cycle` family wire string consistent across Tasks 6 (Rust dispatch), 8.1 (TS union), 8.2 (TS reducer case).
- **Verify-before-implement notes** appear at every API surface that may not match exactly: `current_if2ai_dir` (Task 5), `host_composition.app_state` shape (Task 6), `pub async fn run` exact name (Task 7), `useRuntimeProjection` hook name (Task 9.1).

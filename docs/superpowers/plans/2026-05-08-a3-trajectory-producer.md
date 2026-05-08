# A.3 Trajectory Producer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activate the daydream reflect step. Currently `EmptyTrajectorySource` returns `Vec::new()` so `SelfReflector` runs over an empty input every cycle. A.3 wires `evolution::trajectory::TrajectoryCollector` (already shipped in PR #2) into the turn execution path so real `Trajectory` records flow into daydream.

**Architecture:** Reuse the existing in-memory `TrajectoryCollector`. Add it to `AppState`. At run entry, call `start_trajectory(session_id, task_description)`. At run exit (success or failure), build one `TurnRecord` from observed tool calls + outcome, call `record_turn` then `finish_trajectory(outcome)`. A new `CollectorTrajectorySource` wrapper replaces `EmptyTrajectorySource` in the daydream engine constructor.

**Tech Stack:** Same as A.2.

---

## Brainstorm decisions (2026-05-08)

- **Q1 = B (per-run trajectory)** — one user message → one Trajectory keyed by session_id (the collector's natural key). Multi-iteration tool-using runs aggregate into one `TurnRecord` whose `tool_calls` list captures every observed call.
- **Q2 = A (in-memory, no persistence)** — `TrajectoryCollector` already has a 100-element ring. Insights extracted from trajectories are written to procedural memory (SQLite — durable). Restart loses unconsumed trajectories only; bounded blast radius for an opt-in feature. JSONL persistence shelved as Wave A.5+ work.
- **Q3 = B (full TurnRecord)** — capture `agent_action`, `tool_calls: Vec<ToolCallRecord>`, `success`. SelfReflector's rule-based extraction (PR-5) keys off these fields; minimal data starves it.
- **Q4 = A (outcome derived at run end)** — supervisor's run-completed → `TaskOutcome::Success { quality_score: 1.0 }`; recoverable/final failure → `TaskOutcome::Failure { error_category, root_cause }`. No LLM classification.
- **Q5 = A (instrument turn_service `run.rs`)** — entry/exit hooks with access to `state.trajectory_collector`. Streaming path (`stream_task.rs`) is the same caller layer; the patch covers both via the same `AppState` field.

---

## File map

**Backend new**
| Path | Responsibility |
|------|----------------|
| `src-tauri/src/modules/memory/daydream/collector_trajectory_source.rs` | `CollectorTrajectorySource` impl of `TrajectorySource` |

**Backend modified**
| Path | Change |
|------|--------|
| `src-tauri/src/modules/memory/daydream/mod.rs` | Re-export `CollectorTrajectorySource` |
| `src-tauri/src/bootstrap/memory.rs` | Construct `Arc<TrajectoryCollector>`; pass to `DayDreamEngine::new` (replacing `EmptyTrajectorySource`); expose on `MemoryBootstrap` |
| `src-tauri/src/commands/mod.rs` | Add `trajectory_collector: Arc<TrajectoryCollector>` field to `AppStateConfig` + `AppState` |
| `src-tauri/src/bootstrap/app.rs` | Thread collector through `AppStateConfig` literal |
| `src-tauri/src/modules/application/turn_service/run.rs` | At entry: `start_trajectory`. At success exit: build `TurnRecord`, `record_turn`, `finish_trajectory(Success)`. At failure exit: same with `Failure { ... }` |
| `src-tauri/src/modules/application/turn_service/stream_task.rs` | Mirror instrumentation in the streaming path |

---

## Task 1: `CollectorTrajectorySource`

**Files:**
- Create: `src-tauri/src/modules/memory/daydream/collector_trajectory_source.rs`
- Modify: `src-tauri/src/modules/memory/daydream/mod.rs`

- [ ] **Step 1.1: Create wrapper**

```rust
//! Production `TrajectorySource` — delegates to the in-memory
//! `TrajectoryCollector` shipped in `evolution::trajectory`.
//!
//! Replaces `EmptyTrajectorySource` once turn_service starts feeding
//! the collector with real `start_trajectory` / `record_turn` /
//! `finish_trajectory` calls (A.3).

use std::sync::Arc;

use async_trait::async_trait;

use crate::modules::memory::evolution::trajectory::{Trajectory, TrajectoryCollector};

use super::engine::TrajectorySource;

pub struct CollectorTrajectorySource {
    collector: Arc<TrajectoryCollector>,
}

impl CollectorTrajectorySource {
    pub fn new(collector: Arc<TrajectoryCollector>) -> Self {
        Self { collector }
    }
}

#[async_trait]
impl TrajectorySource for CollectorTrajectorySource {
    async fn recent_trajectories(&self, max: usize) -> Result<Vec<Trajectory>, String> {
        Ok(self.collector.recent_trajectories(max).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::evolution::trajectory::TaskOutcome;

    #[tokio::test]
    async fn empty_collector_returns_empty() {
        let collector = Arc::new(TrajectoryCollector::new());
        let source = CollectorTrajectorySource::new(collector);
        let v = source.recent_trajectories(10).await.unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn finished_trajectory_visible_to_source() {
        let collector = Arc::new(TrajectoryCollector::new());
        collector.start_trajectory("s1", "test task").await;
        collector
            .finish_trajectory(
                "s1",
                TaskOutcome::Success {
                    quality_score: 1.0,
                },
            )
            .await;
        let source = CollectorTrajectorySource::new(collector);
        let v = source.recent_trajectories(10).await.unwrap();
        assert_eq!(v.len(), 1);
        assert!(matches!(
            v[0].outcome,
            Some(TaskOutcome::Success { .. })
        ));
    }
}
```

- [ ] **Step 1.2: Re-export**

In `src-tauri/src/modules/memory/daydream/mod.rs`, append:
```rust
pub mod collector_trajectory_source;
pub use collector_trajectory_source::CollectorTrajectorySource;
```

- [ ] **Step 1.3: Test + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib memory::daydream::collector_trajectory_source
git add src-tauri/src/modules/memory/daydream/
git commit -m "feat(memory/daydream): add CollectorTrajectorySource (A.3 Task 1)"
```

---

## Task 2: AppState wiring + bootstrap construction

**Files:**
- Modify: `src-tauri/src/bootstrap/memory.rs`, `src-tauri/src/bootstrap/app.rs`, `src-tauri/src/commands/mod.rs`

- [ ] **Step 2.1: Add field to `MemoryBootstrap`**

In `bootstrap/memory.rs`, in the `MemoryBootstrap` struct (the one with `daydream_coordinator`), append:

```rust
    pub trajectory_collector:
        std::sync::Arc<crate::modules::memory::evolution::trajectory::TrajectoryCollector>,
```

- [ ] **Step 2.2: Construct + wire into engine**

In `bootstrap/memory.rs`, where `daydream_engine` is built today, replace the `EmptyTrajectorySource` line and add the collector construction BEFORE it:

```rust
let trajectory_collector = std::sync::Arc::new(
    modules::memory::evolution::trajectory::TrajectoryCollector::new(),
);

// ... existing daydream construction unchanged until trajectories assignment:
let daydream_trajectories: std::sync::Arc<dyn modules::memory::daydream::TrajectorySource> =
    std::sync::Arc::new(modules::memory::daydream::CollectorTrajectorySource::new(
        trajectory_collector.clone(),
    ));
// (rest of DayDreamEngine::new call unchanged)
```

Add `trajectory_collector,` to the `MemoryBootstrap { ... }` struct return literal at the end of the function.

- [ ] **Step 2.3: Add field to `AppStateConfig` + `AppState`**

In `src-tauri/src/commands/mod.rs`, find both struct definitions (the same pattern A.2 Task 4 used). In `AppState` add (near other memory fields):

```rust
    /// A.3 — in-memory trajectory collector. Producer side is `turn_service::run`
    /// (start_trajectory at entry, record_turn + finish_trajectory at exit).
    /// Consumer side is daydream's reflect step via `CollectorTrajectorySource`.
    pub trajectory_collector:
        Arc<crate::modules::memory::evolution::trajectory::TrajectoryCollector>,
```

In `AppStateConfig`, append the same field with the same doc.

In `AppState::new` body, copy `trajectory_collector: cfg.trajectory_collector,` into the `Self { ... }` literal.

- [ ] **Step 2.4: Pass through in `bootstrap/app.rs`**

In the `commands::AppStateConfig { ... }` block, add:
```rust
            trajectory_collector: memory_bootstrap.trajectory_collector.clone(),
```

- [ ] **Step 2.5: Verify + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib memory::daydream
git add src-tauri/src/bootstrap/ src-tauri/src/commands/mod.rs
git commit -m "feat(bootstrap): wire TrajectoryCollector through AppState; daydream now reads real trajectories (A.3 Task 2)"
```

---

## Task 3: Producer instrumentation in `turn_service`

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/run.rs` (non-streaming `run_turn`)
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs` (streaming `run_stream_task`)

This is the largest task. Producers must:

1. **At run entry** — call `state.trajectory_collector.start_trajectory(session_id, task_description).await`. Use the user's first message text (truncated to 200 chars) as `task_description`.
2. **During execution** — accumulate `ToolCallRecord`s in a local `Vec<ToolCallRecord>` per tool execution observed.
3. **At run exit (success path)** — build `TurnRecord` with `agent_action: AgentAction::ToolUse` if any tool calls happened else `AgentAction::Reply`; `tool_calls: <accumulated vec>`; `success: true`. Call `record_turn` then `finish_trajectory(TaskOutcome::Success { quality_score: 1.0 })`.
4. **At run exit (failure path)** — same `TurnRecord` build but `success: false`, `agent_action: AgentAction::Error`. Outcome is `TaskOutcome::Failure { error_category: <classified>, root_cause: <error.to_string()> }`. Categorize by error: `"timeout"`, `"tool_error"`, `"provider_error"`, `"cancelled"`, `"unknown"`.

### Step 3.1: Identify hook points

Run synchronously to confirm:
```
grep -n "pub async fn run_turn\|run_completed\|run_failed_recoverable\|run_failed_final" src-tauri/src/modules/application/turn_service/run.rs | head -10
```

Verified earlier: `pub async fn run_turn` at line ~107; success-side log at line ~824 (`"run_completed"`); failure-side at line ~922 (`run_failed_recoverable`/`run_failed_final`). Use these as anchors.

### Step 3.2: Add a `TrajectoryRecording` helper struct (in `run.rs`)

To avoid scattering logic, add at top of `run.rs`:

```rust
/// Per-run trajectory recording state. Built fresh at run entry,
/// finalized at run exit. Tool-call observations accumulate during
/// execution.
struct TrajectoryRecording {
    session_id: String,
    tool_calls: Vec<crate::modules::memory::evolution::trajectory::ToolCallRecord>,
}

impl TrajectoryRecording {
    async fn start(
        collector: &crate::modules::memory::evolution::trajectory::TrajectoryCollector,
        session_id: &str,
        task_description: &str,
    ) -> Self {
        let task = task_description.chars().take(200).collect::<String>();
        collector.start_trajectory(session_id, &task).await;
        Self {
            session_id: session_id.to_string(),
            tool_calls: Vec::new(),
        }
    }

    fn observe_tool(
        &mut self,
        tool_name: &str,
        success: bool,
        duration_ms: u64,
        error: Option<&str>,
    ) {
        self.tool_calls
            .push(crate::modules::memory::evolution::trajectory::ToolCallRecord {
                tool_name: tool_name.to_string(),
                args_summary: String::new(),
                success,
                duration_ms,
                error_message: error.map(|s| s.to_string()),
            });
    }

    async fn finish(
        self,
        collector: &crate::modules::memory::evolution::trajectory::TrajectoryCollector,
        success: bool,
        outcome: crate::modules::memory::evolution::trajectory::TaskOutcome,
    ) {
        use crate::modules::memory::evolution::trajectory::{AgentAction, TurnRecord};
        let agent_action = if !success {
            AgentAction::Error
        } else if self.tool_calls.is_empty() {
            AgentAction::Reply
        } else {
            AgentAction::ToolUse
        };
        let turn = TurnRecord {
            turn_id: 0,
            timestamp: chrono::Utc::now(),
            user_input_summary: None,
            agent_action,
            tool_calls: self.tool_calls,
            success,
            self_assessment: None,
        };
        collector.record_turn(&self.session_id, turn).await;
        collector.finish_trajectory(&self.session_id, outcome).await;
    }
}
```

### Step 3.3: Wire entry + exits in `run_turn`

At the **top of `run_turn`** (after argument validation but before main work):

```rust
let mut trajectory = TrajectoryRecording::start(
    &state.trajectory_collector,
    &request.session_id,
    &request.user_message,
)
.await;
```

Where `state` is the AppState reference accessible in the function (search for how `request.user_message` is accessed; both come from the same `RunTurnRequest`).

At the **success exit** (where the function currently returns `Ok(...)` with the final response): replace the `Ok(response)` line:

```rust
trajectory
    .finish(
        &state.trajectory_collector,
        true,
        crate::modules::memory::evolution::trajectory::TaskOutcome::Success { quality_score: 1.0 },
    )
    .await;
return Ok(response);
```

At each **failure exit** (search for `Err(...)` returns paired with logged `run_failed_*`):

```rust
let outcome = crate::modules::memory::evolution::trajectory::TaskOutcome::Failure {
    error_category: classify_error_category(&error_message),
    root_cause: error_message.clone(),
};
trajectory.finish(&state.trajectory_collector, false, outcome).await;
return Err(error_message);
```

Add the helper:
```rust
fn classify_error_category(msg: &str) -> String {
    let lower = msg.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        "timeout".into()
    } else if lower.contains("cancel") {
        "cancelled".into()
    } else if lower.contains("tool") {
        "tool_error".into()
    } else if lower.contains("provider") || lower.contains("api") {
        "provider_error".into()
    } else {
        "unknown".into()
    }
}
```

### Step 3.4: Tool-call observation

In `run.rs` the tool execution call sites already log `"tool_call_failed"` and the success path. Find these (search `"tool_call_failed"` and the matching success branch). Add `trajectory.observe_tool(name, success, duration_ms, error.as_deref())` at the same point.

> **Pragmatic fallback:** if threading `&mut trajectory` through the tool-execution closure is too invasive, ship Task 3 with **just** the entry + exit hooks (no per-tool observation). `tool_calls` stays empty — `agent_action` collapses to `Reply` — but `Trajectory.outcome` carries success/failure signal which is enough for SelfReflector's failure-side rules to fire on `Failure` outcomes. Per-tool observation can be a follow-up commit.
>
> Pick the simpler path first; smoke-test; only add tool-call observation if reflect step still produces zero insights on failure runs.

### Step 3.5: Mirror instrumentation in `stream_task.rs`

The streaming path uses the same `state` shape and follows the same lifecycle (entry → tool calls → success/failure). Add:
- `TrajectoryRecording::start` at the top of `run_stream_task` (locate via `grep -n "pub async fn run_stream_task" stream_task.rs`)
- `trajectory.finish(...)` calls at the success exit and at each failure exit

Reuse the `TrajectoryRecording` struct from `run.rs` by exporting it: in `run.rs` change `struct TrajectoryRecording` → `pub(super) struct TrajectoryRecording` and add `pub(super) async fn start/finish/observe_tool`. In `stream_task.rs` import via `use super::run::TrajectoryRecording;`.

### Step 3.6: Verify + commit

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib memory::evolution::trajectory
git add src-tauri/src/modules/application/turn_service/
git commit -m "feat(turn_service): record evolution::Trajectory at run entry/exit (A.3 Task 3)"
```

---

## Task 4: Smoke test

After Tasks 1-3 land, run a manual smoke:

- [ ] **Step 4.1: Boot + opt-in**

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
npm run tauri:dev
```

Open Memory Settings → Daydream consolidation. Verify still off by default. Toggle ON.

- [ ] **Step 4.2: Drive a real turn**

In the chat, send any message (e.g. "what's 2+2"). Wait for response. Send another, ideally one that uses a tool (e.g. "list files in /tmp" → bash tool).

- [ ] **Step 4.3: Manual daydream cycle**

Quit + restart `npm run tauri:dev` (engine config is per-boot — needs the `enabled: true` to take effect). Open Memory Settings → click "Run now".

Expected:
- toast `Daydream cycle complete`
- status row shows reflect step now with **non-zero `examined`** (was 0 before A.3)
- backend log: `[daydream] dispatching ... steps=3` (Balanced) or `steps=4` (Aggressive)

The crucial signal: `reflect: examined N / mutated M` where `N >= 1` (one trajectory per run you executed). `M` may still be 0 if no trajectory crossed `MIN_INSIGHT_CONFIDENCE = 0.7` — that's expected for a single happy-path turn; failure trajectories are more likely to produce insights.

- [ ] **Step 4.4: Verify procedural memory accumulation (optional)**

After several turns including a failure (e.g. cancel a slow tool call), run another daydream cycle. Inspect:

```bash
sqlite3 ~/.if2ai/memory/memory.db "SELECT key, content FROM memory_entries WHERE category = 'procedural' OR key LIKE 'proc:%' ORDER BY updated_at DESC LIMIT 5"
```

Expected: rows with keys like `proc:tool_failure_pattern:<hash>` containing extracted insights.

---

## Acceptance criteria

- [ ] `cargo test --lib memory::daydream::collector_trajectory_source` passes
- [ ] `cargo test --lib memory::evolution::trajectory` passes (existing tests)
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` clean
- [ ] `npm run build:web` clean (no frontend changes expected; this is backend-only)
- [ ] **Smoke** Step 4.3 — reflect step shows `examined >= 1` after at least one chat turn

---

## Out of scope

- JSONL persistence at `~/.if2ai/trajectories/evolution/` — Wave A.5 if telemetry shows trajectory loss matters
- Multi-iteration LLM call tracking inside one tool-using run (the run currently aggregates into one `TurnRecord`; per-iteration capture is a refinement for A.5)
- LLM-driven `self_assessment` field — too expensive per turn
- Per-task explicit boundaries (Q1=C deferred)
- Frontend UI surfacing trajectory contents — telemetry / debug only for now

---

## Self-review

- **Brainstorm coverage:** Q1=B → trajectory keyed by session_id, finalized per run. Q2=A → no JSONL, in-memory ring. Q3=B → full TurnRecord with tool_calls (with pragmatic fallback to entry+exit only if threading is hard, see Task 3 note). Q4=A → outcome from supervisor lifecycle in run.rs. Q5=A → instrumented in turn_service.
- **Type consistency:** `Arc<TrajectoryCollector>` flows through bootstrap/memory.rs (Task 2) → AppStateConfig (Task 2) → AppState (Task 2) → run.rs/stream_task.rs (Task 3) → DayDreamEngine via CollectorTrajectorySource (Task 1+2). All `evolution::trajectory::*` paths use the existing module exports — no new types introduced.
- **Verify-before-implement notes:** Task 3.1 spells out the grep + line numbers needed before instrumenting; Task 3.4 + 3.5 carry pragmatic fallbacks for the two highest-risk threading points.

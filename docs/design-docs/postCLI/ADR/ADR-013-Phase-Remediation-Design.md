# ADR-013: Phase 6B/6BW/6E Remediation Design — Gap Closure Specifications

**Status**: Proposed
**Date**: 2026-04-16
**Supersedes**: None (new document)
**Based on**: [phase-6b-6bw-6e-gap-audit-report.md](../../../generated/phase-6b-6bw-6e-gap-audit-report.md) v2

---

## Context

The Phase 6B/6BW/6E Gap Audit Report (v2) identified **36 NOT DONE** and **17 PARTIAL** tasks across 12 ADRs, yielding a **61% overall completion rate** (54/89). While all 573 unit tests pass and clippy is clean, a significant portion of implemented modules have **zero callers in production code**.

This ADR converts every identified gap into a **detailed, actionable design specification** with implementation steps, code modifications, and review criteria.

---

## Gap Taxonomy

| Severity | Count | Scope |
|----------|-------|-------|
| **Critical** (C1-C5) | 5 | Core production pipeline broken or incomplete |
| **High** (H1-H6) | 6 | Functionality partially non-functional |
| **Medium** (M1-M7) | 7 | Quality improvements, missing tooling |
| **Phase 6E** (E1-E7) | 7 | Entire harness framework missing (new module) |

---

## Critical Gaps

---

### C1: WorkingMemory Not Integrated into ConversationRuntime

**对应 TASK**: 012-05
**对应 ADR**: ADR-004 (Token Budget Allocation), ADR-012 (Wiring)

#### Purpose

`WorkingMemory` (max_turns=8, max_tokens=1600 sliding window) is a core component of the 4-slot ContextBudget architecture. Without it in `ConversationRuntime`, the agent sends **all** session messages to every LLM call, causing:
- Unbounded token growth as sessions lengthen
- 40% of the ContextBudget (Working slot) is wasted — no sliding window applied
- API costs scale linearly with session length instead of staying bounded

#### Current State

- `WorkingMemory` struct exists at [working_memory.rs](../../../src-tauri/src/modules/memory/working_memory.rs) — fully implemented, 7 tests pass
- `ConversationRuntime` at [conversation.rs:133-143](../../../src-tauri/src/modules/runtime/conversation.rs#L133-L143) has **no** `working_memory` field
- `run_turn()` at [conversation.rs:272-274](../../../src-tauri/src/modules/runtime/conversation.rs#L272-L274) sends `self.session.messages.clone()` — the full message list
- `agent.rs:999-1019` creates a **local** `WorkingMemory::default()` post-turn, only logs a warning — does not affect the actual API call

#### Implementation Plan

**Step 1**: Add `working_memory` field to `ConversationRuntime`

File: [conversation.rs:133-143](../../../src-tauri/src/modules/runtime/conversation.rs#L133-L143)

```rust
use super::working_memory::WorkingMemory;

pub struct ConversationRuntime<C, T> {
    session: Session,
    api_client: C,
    tool_executor: T,
    permission_policy: PermissionPolicy,
    system_prompt: Vec<String>,
    max_iterations: usize,
    context_budget: Option<ContextBudget>,
    usage_tracker: UsageTracker,
    hook_runner: HookRunner,
    working_memory: Option<WorkingMemory>,  // NEW: sliding window for LLM context
}
```

**Step 2**: Add builder method

File: [conversation.rs:191-205](../../../src-tauri/src/modules/runtime/conversation.rs#L191-L205) — after `with_context_budget`

```rust
/// Set the working memory sliding window.
///
/// When Some, only the most recent N turns (default: 8) within
/// max_tokens (default: 1600) are sent to the LLM. Older messages
/// remain in the session for continuity but are excluded from the API call.
#[must_use]
pub fn with_working_memory(mut self, working_memory: WorkingMemory) -> Self {
    self.working_memory = Some(working_memory);
    self
}
```

**Step 3**: Initialize `None` in constructors

File: [conversation.rs:178-188](../../../src-tauri/src/modules/runtime/conversation.rs#L178-L188)

```rust
Self {
    // ... existing fields ...
    working_memory: None,
}
```

**Step 4**: Apply WorkingMemory in `run_turn()` API call

File: [conversation.rs:272-276](../../../src-tauri/src/modules/runtime/conversation.rs#L272-L276)

```rust
// BEFORE:
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: Some(self.tool_executor.get_definitions()),
};

// AFTER:
let context_messages = match &self.working_memory {
    Some(wm) => wm.get_context(&self.session.messages),
    None => self.session.messages.clone(),
};

let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: context_messages,
    tools: Some(self.tool_executor.get_definitions()),
};
```

**Step 5**: Wire into agent.rs `run_agent_turn()`

File: [agent.rs](../../../src-tauri/src/commands/agent.rs) — where `ConversationRuntime` is constructed (around line 772-786)

```rust
// BEFORE: existing construction
let mut runtime = ConversationRuntime::new_with_features(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt, feature_config,
);

// AFTER: add WorkingMemory builder
let mut runtime = ConversationRuntime::new_with_features(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt, feature_config,
)
.with_working_memory(WorkingMemory::default());
```

**Step 6**: Remove redundant post-turn WorkingMemory check

File: [agent.rs:999-1019](../../../src-tauri/src/commands/agent.rs#L999-L1019) — remove the local `WorkingMemory::default()` creation and logging block. The working memory is now enforced at the API call level, making this post-turn check redundant.

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/runtime/conversation.rs` | Add `working_memory` field + builder + apply in `run_turn` |
| `src-tauri/src/commands/agent.rs` | Wire WorkingMemory into runtime construction, remove redundant post-turn check |

#### Risks

- **Context loss**: If WorkingMemory excludes messages the LLM needs. Mitigation: `WorkingMemory::get_context()` preserves the system message and recent N turns by design.
- **Test breakage**: Existing tests in `conversation.rs` don't set WorkingMemory. Mitigation: `Option<WorkingMemory>` defaults to `None` — existing tests unchanged.

#### Review Standards

- [ ] `ConversationRuntime` has `working_memory: Option<WorkingMemory>` field
- [ ] `with_working_memory()` builder exists with doc comment
- [ ] `run_turn()` applies WorkingMemory only when `Some`, passes `None` path for tests
- [ ] Post-turn redundant check in `agent.rs` removed
- [ ] All existing tests pass without modification
- [ ] No `unwrap()` on `self.working_memory`

---

### C2: AppState Missing 3 Critical Fields

**对应 TASK**: 012-01
**对应 ADR**: ADR-012 (Wiring Layer 1)

#### Purpose

`AppState` currently holds only `memory_provider` and `context_budget` of the 5 planned memory infrastructure fields. The missing `trajectory_manager`, `learning_module`, and `active_retrieval_manager` cause:
- **TrajectoryManager** re-created from scratch every turn (line 715-735 in agent.rs creates a local instance)
- **LearningModule** re-created every turn (line 971 in agent.rs creates a local instance) — state never accumulates across turns
- **ActiveRetrievalManager** not accessible at the AppState level — pre-LLM retrieval can't use shared memory provider

#### Current State

File: [commands/mod.rs:21-48](../../../src-tauri/src/commands/mod.rs#L21-L48)

```rust
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: ...,
    pub permission_overrides: ...,
    pub stream_cancel_senders: ...,
    pub memory_provider: SharedMemoryProvider,       // exists
    pub context_budget: ContextBudget,               // exists
    // MISSING:
    // pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    // pub learning_module: Option<Arc<Mutex<LearningModule>>>,
    // pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
}
```

#### Implementation Plan

**Step 1**: Extend `AppState` struct

File: [commands/mod.rs:21-48](../../../src-tauri/src/commands/mod.rs#L21-L48)

```rust
use crate::modules::learning::LearningModule;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::memory::retrieval::ActiveRetrievalManager;

pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: Arc<Mutex<HashMap<String, Sender<PermissionPromptDecision>>>>,
    pub permission_overrides: Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub stream_cancel_senders: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    pub memory_provider: SharedMemoryProvider,
    pub context_budget: ContextBudget,
    // NEW: Persistent memory infrastructure (Phase 6BW wiring)
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
}
```

**Step 2**: Update `AppState::new()` signature

File: [commands/mod.rs:50-71](../../../src-tauri/src/commands/mod.rs#L50-L71)

```rust
pub fn new(
    session_manager: SessionManager,
    tool_registry: ToolRegistry,
    project_manager: ProjectManager,
    memory_provider: SharedMemoryProvider,
    context_budget: ContextBudget,
    trajectory_manager: Option<Arc<TrajectoryManager>>,     // NEW
    learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>, // NEW
    active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,     // NEW
) -> Self {
    Self {
        session_manager: Arc::new(session_manager),
        tool_registry: Arc::new(tool_registry),
        project_manager: Arc::new(project_manager),
        permission_senders: Arc::new(Mutex::new(HashMap::new())),
        permission_overrides: Arc::new(Mutex::new(HashMap::new())),
        stream_cancel_senders: Arc::new(Mutex::new(HashMap::new())),
        memory_provider,
        context_budget,
        trajectory_manager,     // NEW
        learning_module,        // NEW
        active_retrieval_manager, // NEW
    }
}
```

**Step 3**: Update `main.rs` initialization

File: [main.rs:213-223](../../../src-tauri/src/main.rs#L213-L223)

```rust
// BEFORE:
let app_state = AppState::new(
    session_manager,
    tool_registry,
    project_manager,
    memory_provider,
    context_budget,
);

// AFTER:
// Initialize TrajectoryManager
let trajectory_base = if2ai_dir.join("trajectories");
let trajectory_manager = TrajectoryManager::new(trajectory_base)
    .ok()
    .map(Arc::new);

// Initialize LearningModule (depends on memory_provider + trajectory_manager)
let learning_module = match trajectory_manager.clone() {
    Some(tm) => {
        let memory_for_learning = memory_provider.clone();
        match tokio::runtime::Runtime::new().unwrap().block_on(
            LearningModule::new(memory_for_learning)
        ) {
            Ok(lm) => Some(Arc::new(tokio::sync::Mutex::new(lm))),
            Err(e) => {
                tracing::warn!("[memory] LearningModule failed to init: {e}");
                None
            }
        }
    }
    None => {
        tracing::info!("[memory] TrajectoryManager not available, skipping LearningModule");
        None
    }
};

// Initialize ActiveRetrievalManager (non-blocking, best-effort)
let active_retrieval_manager = Some(Arc::new(ActiveRetrievalManager::with_defaults()));

let app_state = AppState::new(
    session_manager,
    tool_registry,
    project_manager,
    memory_provider,
    context_budget,
    trajectory_manager,
    learning_module,
    active_retrieval_manager,
);
```

**Step 4**: Refactor `agent.rs` to use AppState-level instances

File: [agent.rs:715-735](../../../src-tauri/src/commands/agent.rs#L715-L735) — `record_trajectory_if_possible`

```rust
// BEFORE: creates local TrajectoryManager
async fn record_trajectory_if_possible(session: &RuntimeSession, system_prompt: &[String]) {
    match TrajectoryManager::new(/* base_path */) { ... }
}

// AFTER: use AppState-level instance
async fn record_trajectory_if_possible(
    state: &AppState,
    session: &RuntimeSession,
    system_prompt: &[String],
) {
    if let Some(ref tm) = state.trajectory_manager {
        // use tm.record(...)
    }
}
```

File: [agent.rs:971-981](../../../src-tauri/src/commands/agent.rs#L971-L981) — LearningModule usage

```rust
// BEFORE: creates local LearningModule every turn
if let Ok(mut learning) = LearningModule::new(state.memory_provider.clone()).await {
    learning.self_model_mut().record_turn(...);
}

// AFTER: use AppState-level instance
if let Some(ref lm) = state.learning_module {
    let mut learning = lm.lock().await;
    learning.self_model_mut().record_turn(/* success= */ true, /* response_time_ms= */ 0.0);
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/commands/mod.rs` | Add 3 fields to AppState + update `new()` signature |
| `src-tauri/src/main.rs` | Initialize 3 new fields + update `AppState::new()` call |
| `src-tauri/src/commands/agent.rs` | Refactor to use AppState-level instances instead of per-turn creation |

#### Risks

- **tokio runtime in main.rs**: `LearningModule::new` is async but `main()` is sync. Solution: use a temporary `tokio::runtime::Runtime` for init (same pattern as `create_memory_provider`).
- **Initialization ordering**: `LearningModule` depends on `memory_provider` — init in correct order.

#### Review Standards

- [ ] AppState has 3 new `Option<>` fields with graceful fallback
- [ ] `AppState::new()` accepts 3 new parameters
- [ ] `main.rs` initializes all 3 with `tracing::warn!` on failure
- [ ] `agent.rs` no longer creates local TrajectoryManager/LearningModule per turn
- [ ] No `unwrap()` in wiring code

---

### C3: Phase 6E Harness Framework Completely Missing

**对应 TASK**: 011-01 ~ 011-07
**对应 ADR**: ADR-011 (Agent Loop Harness Framework)

#### Purpose

Phase 6E provides **observability and control** for the agent loop. Without it:
- No structured telemetry for token usage, tool call statistics, or cost tracking
- No session recording/replay for debugging agent behavior
- No external control plane for pause/resume/inject decisions
- Cannot measure agent performance over time or reproduce bugs

This is a **new module** requiring ~1500 lines of code across 7 slices.

#### Current State

- `src-tauri/src/modules/harness/` directory **does not exist**
- No `EventBus`, `TelemetryCollector`, `SessionRecorder`, or harness IPC commands
- Phase 6E YAML declares 7 slices, all `status: pending`

#### Implementation Plan

Follow the Phase 6E exec-plan slices sequentially. Each slice is detailed below.

---

#### C3-E1: EventBus Core (slice 6e.1)

**New files to create**:

`src-tauri/src/modules/harness/mod.rs`:
```rust
pub mod event_bus;
pub use event_bus::{AgentEvent, EventBus, EventSubscriber, TurnOutcome, DecisionType};
```

`src-tauri/src/modules/harness/event_bus.rs`:
```rust
use std::sync::Arc;
use std::sync::mpsc;
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};

/// Outcome of an agent turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnOutcome {
    Completed,
    Error(String),
    Cancelled,
}

/// Types of decision points in the agent loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionType {
    ToolSelection { tool_name: String, confidence: f32 },
    PermissionRequest { tool_name: String },
    CompactionTrigger,
}

/// Events emitted at key points in the agent lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    TurnStart { stream_id: String, session_id: String },
    TurnEnd { stream_id: String, outcome: TurnOutcome },
    LlmStart { stream_id: String, request_id: String },
    LlmEnd { stream_id: String, request_id: String, tokens: u64 },
    ToolStart { stream_id: String, tool_name: String, tool_args: String },
    ToolEnd { stream_id: String, tool_name: String, duration_ms: u64, success: bool },
    DecisionPoint { stream_id: String, point: DecisionType, context: serde_json::Value },
    Error { stream_id: String, error: String },
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &AgentEvent);
}

/// Thread-safe event bus singleton
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<Box<dyn EventSubscriber>>>>,
    sender: mpsc::UnboundedSender<AgentEvent>,
}
```

Key design decisions:
- `mpsc::unbounded_channel` for non-blocking emit
- `parking_lot::Mutex` for subscriber list (not std::sync::Mutex)
- Global singleton via `EventBus::global()` using `std::sync::OnceLock`
- Dedicated background thread drains the channel and dispatches to subscribers

---

#### C3-E2: TelemetryCollector (slice 6e.2)

**New file**: `src-tauri/src/modules/harness/telemetry.rs`

Structures: `TokenStats`, `ToolStats`, `TelemetryCollector`
- Subscribes to `LlmEnd`/`ToolEnd` events
- Uses `RwLock` for concurrent read/write safety
- Methods: `record_llm_usage()`, `record_tool_call()`, `get_token_stats()`, `get_tool_stats()`

---

#### C3-E3: AgentLoopIntegration (slice 6e.3)

**Modified file**: [agent.rs](../../../src-tauri/src/commands/agent.rs)

Insert `EventBus::global().emit(...)` calls at 8 points in `start_agent_stream()`:
1. Turn start → `TurnStart`
2. LLM call before → `LlmStart`
3. LLM call after → `LlmEnd`
4. Tool call before → `ToolStart`
5. Tool call after → `ToolEnd`
6. Tool selection decision → `DecisionPoint`
7. Error path → `Error`
8. Turn end → `TurnEnd`

---

#### C3-E4: SessionRecorder (slice 6e.4)

**New file**: `src-tauri/src/modules/harness/recorder.rs`

SQLite tables: `harness_recordings`, `harness_events`
- `start_recording(session_id) -> RecordingId`
- `stop_recording(recording_id) -> RecordedSession`
- `list_recordings(session_id) -> Vec<RecordingMeta>`
- Subscribes to all `AgentEvent` types

---

#### C3-E5: HarnessControl IPC (slice 6e.5)

**New file**: `src-tauri/src/commands/harness.rs`

5 Tauri commands:
- `harness_get_telemetry(session_id)` → `TelemetrySnapshot`
- `harness_start_recording(session_id)` → `RecordingId`
- `harness_stop_recording(recording_id)` → `RecordedSession`
- `harness_get_decision_history(session_id)` → `Vec<DecisionRecord>`
- `harness_inject_pause_point(session_id, reason)` → `Result<(), String>` (debug only)

---

#### C3-E6: harness-cli (slice 6e.6)

**New directory**: `harness-cli/`

Workspace member with `clap` CLI:
- `harness-cli get-telemetry --session-id <id>`
- `harness-cli start-recording --session-id <id>`
- `harness-cli stop-recording --recording-id <id>`
- `harness-cli replay --recording-id <id> --speed 1.0`
- `harness-cli dashboard`

---

#### C3-E7: Integration Tests (slice 6e.7)

**New file**: `src-tauri/tests/harness_integration_test.rs`

4 test scenarios:
- `event_bus_delivers_to_subscriber`
- `telemetry_stats_accuracy`
- `recorder_records_full_session`
- `control_ipc_commands`

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/harness/mod.rs` | **CREATE** — module root |
| `src-tauri/src/modules/harness/event_bus.rs` | **CREATE** — EventBus + AgentEvent |
| `src-tauri/src/modules/harness/telemetry.rs` | **CREATE** — TelemetryCollector |
| `src-tauri/src/modules/harness/recorder.rs` | **CREATE** — SessionRecorder |
| `src-tauri/src/commands/harness.rs` | **CREATE** — IPC commands |
| `src-tauri/src/commands/agent.rs` | **MODIFY** — add event emit calls |
| `src-tauri/src/commands/mod.rs` | **MODIFY** — export harness commands |
| `src-tauri/Cargo.toml` | **MODIFY** — add `harness` module, `parking_lot` dep |
| `harness-cli/Cargo.toml` | **CREATE** — CLI binary |
| `harness-cli/src/main.rs` | **CREATE** — CLI entry point |
| `Cargo.toml` (workspace) | **MODIFY** — add harness-cli member |
| `src-tauri/tests/harness_integration_test.rs` | **CREATE** — integration tests |

#### Risks

- **EventBus overhead**: Each emit adds latency. Mitigation: `unbounded_channel` + background thread ensures emit is O(1) and non-blocking.
- **SQLite contention**: SessionRecorder writes to its own SQLite DB, separate from memory DB.
- **Workspace changes**: Adding `harness-cli` as a new workspace member requires top-level `Cargo.toml` update.

#### Review Standards

- [ ] `EventBus` is thread-safe singleton (`OnceLock`)
- [ ] `emit()` is non-blocking (unbounded channel)
- [ ] All 8 critical paths in `agent.rs` emit events
- [ ] `TelemetryCollector` uses `RwLock` not `Mutex`
- [ ] SessionRecorder uses separate SQLite DB path
- [ ] `harness_inject_pause_point` is `#[cfg(debug_assertions)]` gated
- [ ] All 7 slices pass acceptance tests

---

### C4: ReflectionEngine Has Zero Callers

**对应 TASK**: 008-08
**对应 ADR**: ADR-008 (Self-Learning Modules)

#### Purpose

`ReflectionEngine` is the core of the self-learning system. It analyzes tool sequences, outcomes, and topics to produce `Reflection` objects that update the `SelfModel`. Without callers:
- Agent never reflects on its own behavior patterns
- `SelfModel` only tracks turn counts, never learns capabilities/limitations
- Trust tracking is one-directional (only tool outcomes), never updated from reflection

#### Current State

- `ReflectionEngine` trait + `StandardReflectionEngine` exist at [reflection.rs](../../../src-tauri/src/modules/learning/reflection.rs)
- `LearningModule` creates a `StandardReflectionEngine` internally at [learning/mod.rs:64](../../../src-tauri/src/modules/learning/mod.rs#L64)
- **Zero callers**: no code invokes `learning.reflection_engine.analyze_session()` or `learning.reflect()`
- `agent.rs:971-981` calls `learning.self_model_mut().record_turn()` but never triggers reflection

#### Implementation Plan

**Step 1**: Add reflection trigger to post-turn sequence

File: [agent.rs:970-981](../../../src-tauri/src/commands/agent.rs#L970-L981) — after the existing `record_turn` call

```rust
// AFTER: add reflection trigger
if let Some(ref lm) = state.learning_module {
    let mut learning = lm.lock().await;

    // Record turn outcome
    learning.self_model_mut().record_turn(
        /* success= */ true, /* response_time_ms= */ 0.0
    );

    // Trigger reflection every N turns (configurable, default: 5)
    let reflect_interval = 5;
    if learning.self_model().turn_count > 0
        && learning.self_model().turn_count % reflect_interval == 0
    {
        match learning.reflection_engine.analyze_session(&trajectory_session).await {
            Ok(reflections) => {
                learning.self_model.update_from_reflections(&reflections);
                tracing::info!(
                    "[run_agent_turn] Reflection triggered: {} insights",
                    reflections.len()
                );
            }
            Err(e) => {
                tracing::warn!("[run_agent_turn] Reflection failed: {e}");
            }
        }
    }
}
```

**Step 2**: Ensure `SelfModel` has `update_from_reflections` method

File: [self_model.rs](../../../src-tauri/src/modules/learning/self_model.rs)

If not present, add:
```rust
impl SelfModel {
    pub fn update_from_reflections(&mut self, reflections: &[Reflection]) {
        for reflection in reflections {
            if let Some(pattern) = reflection.learned_pattern.as_ref() {
                self.learned_patterns.push(pattern.clone());
            }
            if let Some(cap) = reflection.new_capability.as_ref() {
                self.capabilities.push(cap.clone());
            }
            if let Some(limit) = reflection.limitation.as_ref() {
                self.limitations.push(limit.clone());
            }
        }
    }
}
```

**Step 3**: Remove `#![allow(dead_code)]` from learning module

File: [learning/mod.rs:10](../../../src-tauri/src/modules/learning/mod.rs#L10)

```rust
// REMOVE: #![allow(dead_code)]
// The module now has callers in the agent loop.
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/commands/agent.rs` | Add reflection trigger after turn recording |
| `src-tauri/src/modules/learning/self_model.rs` | Add `update_from_reflections()` if missing |
| `src-tauri/src/modules/learning/mod.rs` | Remove `#![allow(dead_code)]` |

#### Risks

- **Reflection latency**: `analyze_session` may be slow (potentially involves LLM). Mitigation: run asynchronously, don't block the response to the user.
- **Reflection interval**: Every 5 turns is a reasonable default but should be configurable.

#### Review Standards

- [ ] Reflection triggered at configurable interval (default: 5 turns)
- [ ] Reflection errors logged as `warn!`, not `error!`
- [ ] `SelfModel::update_from_reflections` exists and processes reflections
- [ ] `learning/mod.rs` no longer has module-level `#![allow(dead_code)]`
- [ ] Reflection doesn't block the user-facing response

---

### C5: WeibullDecay Results Never Applied to MemoryProvider

**对应 TASK**: 012-09
**对应 ADR**: ADR-012 (Wiring Layer 3)

#### Purpose

`WeibullDecay` computes importance decay factors for compacted entries, but the results are **never applied back** to the `MemoryProvider`. This means:
- Episodic entries retain their original importance indefinitely
- Memory retrieval returns stale entries with inflated importance scores
- The `compute_importance()` method exists but is never called on live data after compaction

#### Current State

- `WeibullDecay` exists at [episodic_compaction.rs:16-72](../../../src-tauri/src/modules/runtime/episodic_compaction.rs#L16-L72)
- `agent.rs:1021-1031` creates `WeibullDecay::default()` and computes `decay_factor()` but **only logs it**
- `MemoryProvider` trait has no `apply_importance_decay()` method — it needs to be added
- `compact_episodic_entries()` at [episodic_compaction.rs:113-173](../../../src-tauri/src/modules/runtime/episodic_compaction.rs#L113-L173) removes low-importance entries but doesn't update remaining entries

#### Implementation Plan

**Step 1**: Add `apply_importance_decay` to `MemoryProvider` trait

File: [mod.rs (memory)](../../../src-tauri/src/modules/memory/mod.rs)

```rust
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    // ... existing methods ...

    /// Apply importance decay to all episodic entries based on their age.
    /// Called after compaction to adjust remaining entries' importance scores.
    async fn apply_importance_decay(&self, decay: &WeibullDecay) -> Result<usize, MemoryError>;
}
```

**Step 2**: Implement for `SqliteMemoryProvider`

File: [sqlite_provider.rs](../../../src-tauri/src/modules/memory/providers/sqlite_provider.rs)

```rust
async fn apply_importance_decay(&self, decay: &WeibullDecay) -> Result<usize, MemoryError> {
    let conn = self.conn.lock().map_err(|_| MemoryError::Generic("lock poisoned".into()))?;

    // Get all episodic entries
    let mut stmt = conn.prepare("SELECT key, importance, created_at FROM memory_entries WHERE category = 'episodic'")
        .map_err(|e| MemoryError::Generic(format!("query failed: {e}")))?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?))
    })?;

    let mut updated = 0;
    for row in rows {
        let (key, _old_importance, created_at_str) = row?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| MemoryError::Generic(format!("parse date: {e}")))?;

        let age_hours = (chrono::Utc::now() - created_at).num_hours() as f32;
        let decay_factor = decay.decay_factor(age_hours);

        // Update importance = importance * decay_factor
        conn.execute(
            "UPDATE memory_entries SET importance = importance * ?1 WHERE key = ?2",
            params![decay_factor, key],
        )?;
        updated += 1;
    }

    Ok(updated)
}
```

**Step 3**: Implement for `VectorMemoryProvider`

File: [vector_provider.rs](../../../src-tauri/src/modules/memory/providers/vector_provider.rs)

Similar pattern — iterate entries in LanceDB, apply decay, update.

**Step 4**: Apply decay in agent.rs post-turn

File: [agent.rs:1021-1031](../../../src-tauri/src/commands/agent.rs#L1021-L1031)

```rust
// BEFORE: only logs decay factor
let decay = WeibullDecay::default();
let removed_count = pre_compact_message_count.saturating_sub(post_compact_message_count);
if removed_count > 0 {
    let decay_factor = decay.decay_factor(24.0);
    tracing::info!(...);
}

// AFTER: actually apply decay
let decay = WeibullDecay::default();
let removed_count = pre_compact_message_count.saturating_sub(post_compact_message_count);
if removed_count > 0 {
    match state.memory_provider.apply_importance_decay(&decay).await {
        Ok(updated) => {
            tracing::info!(
                "[run_agent_turn] WeibullDecay applied: {updated} entries updated"
            );
        }
        Err(e) => {
            tracing::warn!("[run_agent_turn] WeibullDecay failed: {e}");
        }
    }
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/mod.rs` | Add `apply_importance_decay()` to `MemoryProvider` trait |
| `src-tauri/src/modules/memory/providers/sqlite_provider.rs` | Implement `apply_importance_decay` |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | Implement `apply_importance_decay` |
| `src-tauri/src/commands/agent.rs` | Call `apply_importance_decay` post-turn |

#### Risks

- **SQLite write contention**: Decay updates happen while other operations may be reading. Mitigation: `Mutex<Connection>` already protects writes.
- **LanceDB update cost**: Vector DB updates can be slow. Mitigation: batch updates, run in background.

#### Review Standards

- [ ] `MemoryProvider` trait has `apply_importance_decay()` method
- [ ] `SqliteMemoryProvider` implements it with proper SQL UPDATE
- [ ] `VectorMemoryProvider` implements it (can be no-op with `tracing::info`)
- [ ] `agent.rs` calls it post-compaction with error handling
- [ ] No `unwrap()` in decay application code

---

## High Gaps

---

### H1: SessionManager Has No Active Retrieval Integration

**对应 TASK**: 002-P1-06
**对应 ADR**: ADR-002 (Active Retrieval vs Passive Invocation)

#### Purpose

`ActiveRetrievalManager` exists but is never integrated into `SessionManager`. The session lifecycle layer should support active retrieval for every new session.

#### Implementation Plan

**Step 1**: Add `active_retrieval` field to `SessionManager`

```rust
pub struct SessionManager {
    // ... existing fields ...
    active_retrieval: Option<Arc<ActiveRetrievalManager>>,
}
```

**Step 2**: Add builder method and constructor parameter

```rust
impl SessionManager {
    pub fn with_active_retrieval(mut self, arm: Arc<ActiveRetrievalManager>) -> Self {
        self.active_retrieval = Some(arm);
        self
    }
}
```

**Step 3**: Inject retrieval context during session creation

In `create_session()`, if active retrieval is enabled:
```rust
if let Some(ref arm) = self.active_retrieval {
    if let Ok(context) = arm.retrieve_as_context(&user_message).await {
        // inject into initial system prompt
    }
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/session/mod.rs` | Add `active_retrieval` field |
| `src-tauri/src/commands/session.rs` | Wire active retrieval into session creation |

#### Review Standards

- [ ] `SessionManager` has optional `active_retrieval` field
- [ ] Builder method `with_active_retrieval()` exists
- [ ] Session creation uses active retrieval when available

---

### H2: No IVF-PQ Index Configuration for LanceDB

**对应 TASK**: 003-07
**对应 ADR**: ADR-003 (FastEmbed + LanceDB Selection)

#### Purpose

Without IVF-PQ indexing, LanceDB performs brute-force vector search. For datasets >10K entries, this is O(n) per query — unacceptably slow.

#### Implementation Plan

Add index creation to `VectorMemoryProvider::new()`:

```rust
// After creating/opening the LanceDB table:
use lancedb::index::Index;
use lancedb::index::vector::IvfPqIndexBuilder;

table
    .create_index(
        &["embedding"],
        Index::IvfPq(IvfPqIndexBuilder::default()
            .num_partitions(32)
            .num_sub_vectors(24)),
    )
    .await?;
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/providers/lancedb.rs` | Add IVF-PQ index creation |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | Call index creation in `new()` |

#### Review Standards

- [ ] IVF-PQ index created during table initialization
- [ ] Index parameters configurable (num_partitions, num_sub_vectors)
- [ ] Index creation failure doesn't panic (logs warning, continues without index)

---

### H3: VectorMemoryProvider Doesn't Dual-Write to SQLite

**对应 TASK**: 003-08
**对应 ADR**: ADR-003 (FastEmbed + LanceDB Selection)

#### Purpose

`VectorMemoryProvider.store()` writes to LanceDB only. If the app restarts without FastEmbed re-initializing, vector entries are lost. SQLite should be the persistent backing.

#### Implementation Plan

**Step 1**: Add `sqlite_provider` field to `VectorMemoryProvider`

```rust
pub struct VectorMemoryProvider {
    lancedb: LanceDBMemory,
    embedder: Arc<FastEmbedProvider>,
    config: VectorProviderConfig,
    sqlite: Option<Arc<SqliteMemoryProvider>>, // NEW: dual-write backing store
}
```

**Step 2**: Dual-write in `store()`

```rust
async fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
    // Write to SQLite first (source of truth)
    if let Some(ref sqlite) = self.sqlite {
        sqlite.store(entry.clone()).await?;
    }

    // Write to LanceDB (vector index)
    let embedding = self.embedder.embed(&entry.content).await?;
    self.lancedb.insert(&entry, &embedding).await?;

    Ok(())
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | Add SQLite dual-write |

#### Review Standards

- [ ] `store()` writes to SQLite before LanceDB
- [ ] SQLite is optional (`Option<>`) — graceful fallback
- [ ] `recall()` can read from either backend

---

### H4: VectorProvider dead_code Suppression Not Cleaned

**对应 TASK**: 012-04
**对应 ADR**: ADR-012 (Wiring Layer 1)

#### Purpose

Module-level `#![allow(dead_code)]` at [vector_provider.rs:11](../../../src-tauri/src/modules/memory/providers/vector_provider.rs#L11) masks the fact that the module has real callers. After C1-C5 wiring, most functions will have callers.

#### Implementation Plan

Replace module-level `#![allow(dead_code)]` with targeted `#[allow(dead_code)]` on truly unused functions:

```rust
// Remove line 11: #![allow(dead_code)]
// Add #[allow(dead_code)] only to functions that remain uncalled
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | Replace module-level with function-level `#[allow(dead_code)]` |

---

### H5: HRR Tests Still `#[ignore]` — No MockEmbedder

**对应 TASK**: 012-10
**对应 ADR**: ADR-007 (HRR), ADR-012 (Wiring)

#### Purpose

HRR integration tests are ignored because they require FastEmbed model download at runtime. Without a mock embedder, HRR algebraic reasoning (bind/unbind/bundle) has zero test coverage.

#### Implementation Plan

**Step 1**: Create `MockEmbedder`

```rust
// src-tauri/src/modules/memory/embedding/mock.rs
pub struct MockEmbedder;

impl MockEmbedder {
    pub fn embed_sync(&self, text: &str) -> Vec<f32> {
        // Deterministic pseudo-embedding based on text hash
        let mut vec = vec![0.0f32; 384];
        let hash = std::collections::hash_map::DefaultHasher::new();
        // ... simple hash-based deterministic vector
        vec
    }
}
```

**Step 2**: Replace `#[ignore]` tests with MockEmbedder

In [hrr/integration.rs](../../../src-tauri/src/modules/memory/hrr/integration.rs), create tests that use `MockEmbedder` instead of `FastEmbedProvider`.

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/embedding/mock.rs` | **CREATE** — MockEmbedder |
| `src-tauri/src/modules/memory/embedding/mod.rs` | Export MockEmbedder |
| `src-tauri/src/modules/memory/hrr/integration.rs` | Replace `#[ignore]` tests with MockEmbedder |

---

### H6: Trajectory Tauri Commands Not Exported

**对应 TASK**: 009-06
**对应 ADR**: ADR-009 (Trajectory Learning)

#### Purpose

`export_trajectories` exists as a Tauri command but is not exported in `commands/mod.rs` invoke handler list.

#### Current State

- `export_trajectories` is imported in [main.rs:12](../../../src-tauri/src/main.rs#L12) but not in the `invoke_handler!` macro at [main.rs:295](../../../src-tauri/src/main.rs#L295) — actually it IS there at line 295.
- `get_trajectory_count` is NOT exported.

#### Implementation Plan

Add `get_trajectory_count` to the invoke handler:

File: [main.rs:228-296](../../../src-tauri/src/main.rs#L228-L296)

```rust
// Add to invoke_handler:
get_trajectory_count,  // NEW
```

And export in [commands/mod.rs:104](../../../src-tauri/src/commands/mod.rs#L104):

```rust
pub use settings::{
    export_trajectories, get_memory_config, set_memory_config,
    get_trajectory_count,  // NEW
    MemoryConfig, MemoryConfigInput,
};
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/main.rs` | Add `get_trajectory_count` to invoke_handler |
| `src-tauri/src/commands/mod.rs` | Export `get_trajectory_count` |

---

## Medium Gaps

---

### M1: No JSON-to-SQLite Migration Tool

**对应 TASK**: 001-08
**对应 ADR**: ADR-001 (SQLite P0 Persistence)

#### Purpose

Legacy `memory.json` files from earlier versions cannot be imported into SQLite. Users with existing data lose their memory when upgrading.

#### Implementation Plan

Add `migrate_from_json()` to `SqliteMemoryProvider`:

```rust
impl SqliteMemoryProvider {
    pub async fn migrate_from_json(&self, json_path: &Path) -> Result<usize, MemoryError> {
        let content = tokio::fs::read_to_string(json_path).await
            .map_err(|e| MemoryError::Generic(format!("read json: {e}")))?;

        let entries: Vec<MemoryEntry> = serde_json::from_str(&content)
            .map_err(|e| MemoryError::Generic(format!("parse json: {e}")))?;

        let mut count = 0;
        for entry in entries {
            self.store(entry).await?;
            count += 1;
        }

        Ok(count)
    }
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/providers/sqlite_provider.rs` | Add `migrate_from_json()` method |

---

### M2: Token Counting Uses 4:1 Heuristic Instead of tiktoken-rs

**对应 TASK**: 004-03
**对应 ADR**: ADR-004 (Token Budget Allocation)

#### Purpose

`char_count / 4 + 1` heuristic is inaccurate for non-English text and tool messages. This causes budget enforcement to be unreliable.

#### Implementation Plan

**Step 1**: Add `tiktoken-rs` dependency to `Cargo.toml`

**Step 2**: Replace `estimate_token_count_from_chars` with tiktoken-based estimation

```rust
use tiktoken_rs::cl100k_base;

pub fn estimate_token_count_from_chars(text: &str) -> usize {
    let bpe = cl100k_base().expect("tiktoken model unavailable");
    bpe.encode_with_special_tokens(text).len()
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `tiktoken-rs` dependency |
| `src-tauri/src/modules/runtime/compact.rs` | Replace heuristic with tiktoken |
| `src-tauri/src/modules/runtime/budget.rs` | Replace heuristic with tiktoken |

---

### M3: No YAML Budget Configuration Loading

**对应 TASK**: 004-08
**对应 ADR**: ADR-004 (Token Budget Allocation)

#### Purpose

ContextBudget uses hardcoded defaults (4000 tokens, 10/20/30/40%). Users cannot customize without code changes.

#### Implementation Plan

Add `BudgetConfig` struct with YAML deserialization:

```rust
#[derive(Deserialize)]
pub struct BudgetConfig {
    pub total_tokens: usize,
    pub system_pct: f32,
    pub episodic_pct: f32,
    pub semantic_pct: f32,
    pub working_pct: f32,
}

impl BudgetConfig {
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(Into::into)
    }
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/runtime/budget.rs` | Add `BudgetConfig` with YAML loading |

---

### M4: Session Schema Partially Incompatible with claw-cli

**对应 TASK**: 005-02
**对应 ADR**: ADR-005 (Upstream Claw-CLI Relationship)

#### Purpose

Session schema is missing `project_id`, `title`, `created_at`, `updated_at`, `token_count`, `pinned` fields required for claw-cli compatibility.

#### Implementation Plan

Add missing fields to the Session struct in [session.rs](../../../src-tauri/src/modules/session/mod.rs):

```rust
pub struct Session {
    // ... existing fields ...
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub token_count: usize,
    pub pinned: bool,
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/session/mod.rs` | Add missing schema fields |

---

### M5: ThreatScanner Module Doesn't Exist

**对应 TASK**: 006-05
**对应 ADR**: ADR-006 (Security Design)

#### Purpose

No regex-based scanning for secrets, API keys, or sensitive patterns in memory content before storage.

#### Implementation Plan

Create `security/scanner.rs`:

```rust
pub struct ThreatScanner {
    patterns: Vec<Regex>,
}

impl ThreatScanner {
    pub fn scan(&self, content: &str) -> Vec<Threat> {
        // Scan for API keys, passwords, private keys, etc.
    }
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/memory/security/scanner.rs` | **CREATE** — ThreatScanner |
| `src-tauri/src/modules/memory/security/mod.rs` | **CREATE** — module root |

---

### M6: FrozenSnapshot Verification Failure Has No Warning Log

**对应 TASK**: 006-06
**对应 ADR**: ADR-012 (Wiring Layer 2)

#### Purpose

`FrozenSnapshot::verify()` returns `bool` but the verify failure path at [agent.rs:987-991](../../../src-tauri/src/commands/agent.rs#L987-L991) only logs `warn!` without additional context about what changed.

#### Implementation Plan

Enhance `FrozenSnapshot::verify` to return `VerifyResult` with details:

```rust
pub struct VerifyResult {
    pub valid: bool,
    pub expected_hash: String,
    pub details: Option<String>,
}
```

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/src/modules/runtime/snapshot.rs` | Enhance verify to return detailed result |

---

### M7: Security Integration Tests Missing

**对应 TASK**: 006-07
**对应 ADR**: ADR-006 (Security Design)

#### Purpose

No integration tests verify the security layer (input validation, path validation, atomic writes, access control) works end-to-end.

#### Implementation Plan

Create `src-tauri/tests/security_integration.rs` with tests for:
- `test_xss_input_rejected`
- `test_path_traversal_blocked`
- `test_atomic_write_rollback`
- `test_access_context_enforcement`

#### Target Files

| File | Change |
|------|--------|
| `src-tauri/tests/security_integration.rs` | **CREATE** — security integration tests |

---

## Implementation Dependency Graph

```
Phase 1: Critical Wiring (Prerequisites for all other layers)
├── C2: AppState extension → enables C4 (ReflectionEngine wiring)
├── C1: WorkingMemory → independent
├── C5: WeibullDecay application → needs M3 (MemoryProvider trait extension)
└── C4: ReflectionEngine → depends on C2

Phase 2: High Priority Fixes
├── H4: dead_code cleanup → depends on C1-C5 (callers must exist first)
├── H6: Trajectory command export → independent
├── H3: Vector dual-write → independent
├── H1: SessionManager active retrieval → depends on C2 (ActiveRetrievalManager in AppState)
├── H2: IVF-PQ index → independent
└── H5: MockEmbedder → enables HRR test coverage

Phase 3: Phase 6E Harness Framework (7 slices, sequential)
├── 6e.1: EventBus → prerequisite for all 6E slices
├── 6e.2: TelemetryCollector → depends on 6e.1
├── 6e.3: AgentLoopIntegration → depends on 6e.1, 6e.2
├── 6e.4: SessionRecorder → depends on 6e.1
├── 6e.5: HarnessControl IPC → depends on 6e.2, 6e.4
├── 6e.6: harness-cli → depends on 6e.5
└── 6e.7: Integration Tests → depends on 6e.1-06

Phase 4: Quality Improvements
├── M1: JSON migration → independent
├── M2: tiktoken-rs → independent
├── M3: YAML config → independent
├── M4: Session schema → independent
├── M5: ThreatScanner → independent
├── M6: FrozenSnapshot → independent
└── M7: Security tests → depends on M5
```

---

## Recommended Execution Order

| Order | Gap | Phase | Rationale |
|-------|-----|-------|-----------|
| 1 | C2 (AppState) | 6BW | Unblocks C4, H1 — foundational wiring |
| 2 | C1 (WorkingMemory) | 6BW | Highest impact on API cost/performance |
| 3 | C4 (ReflectionEngine) | 6BW | Depends on C2, unlocks self-learning |
| 4 | C5 (WeibullDecay) | 6BW | Depends on M3 (trait extension) |
| 5 | H4 (dead_code) | 6BW | Cleanup after C1-C5 establish callers |
| 6 | H6 (Trajectory) | 6BW | Quick fix, 5 lines |
| 7 | Phase 6E (E1-E7) | 6E | New module, 7 sequential slices |
| 8 | H3 (Vector dual-write) | 6B | Data persistence fix |
| 9 | H1 (SessionManager retrieval) | 6BW | Depends on C2 |
| 10 | H2 (IVF-PQ) | 6B | Performance fix |
| 11 | H5 (MockEmbedder) | 6B | Test coverage |
| 12 | M1-M7 | 6B | Quality improvements |

---

## Rationale

### Why Not Fix All Gaps in One Phase?

1. **Blast radius**: C1-C5 touch core agent loop code — each must be independently testable
2. **Phase 6E is a greenfield module**: 1500+ lines of new code deserves its own Phase
3. **Medium gaps are non-blocking**: tiktoken-rs, YAML config, ThreatScanner are nice-to-have
4. **Rollback safety**: If C1 breaks something, we can revert it without touching C4

### Why Option<> for All New AppState Fields?

Per ADR-012 review checklist: "All wiring points should have graceful fallback." Using `Option<>` ensures:
- App starts even if TrajectoryManager path is unavailable
- LearningModule failure doesn't crash the app
- ActiveRetrievalManager absence falls back to passive memory invocation

### Why MockEmbedder Instead of FastEmbed for Tests?

FastEmbed requires model download (~100MB) which:
- Fails in CI without internet
- Slow to initialize (30s+ on cold start)
- Non-deterministic (model weights may change)

MockEmbedder provides deterministic, zero-download, instant test execution.

---

## Consequences

### Positive
- All Phase 6B modules become production-visible after C1-C5
- Phase 6E provides observability foundation for future debugging
- Self-learning actually learns after C4
- Memory importance scores reflect temporal decay after C5

### Negative
- `agent.rs` grows more complex with additional post-turn hooks
- AppState initialization time increases (TrajectoryManager, LearningModule)
- More failure modes to handle (each new component can fail independently)
- New `harness` module adds ~1500 lines to maintain

### Mitigations
- Each wiring point uses `tracing::warn!` on failure, not panic
- All new AppState fields are `Option<>` for graceful degradation
- Phase 6E EventBus is non-blocking — zero impact on agent loop latency
- Modular design: each gap can be reverted independently

---

## Review Checklist (All Gaps)

### Critical (C1-C5)
- [ ] WorkingMemory applied to LLM API call, not just post-turn logging
- [ ] AppState has 3 new `Option<>` fields with graceful fallback
- [ ] main.rs initializes all 3 new fields
- [ ] agent.rs uses AppState-level TrajectoryManager/LearningModule (not per-turn creation)
- [ ] ReflectionEngine triggered at configurable interval
- [ ] SelfModel::update_from_reflections processes reflection results
- [ ] MemoryProvider trait has apply_importance_decay() method
- [ ] WeibullDecay actually applied post-compaction, not just logged
- [ ] learning/mod.rs no longer has #![allow(dead_code)]

### High (H1-H6)
- [ ] SessionManager has optional active_retrieval field
- [ ] VectorMemoryProvider dual-writes to SQLite
- [ ] vector_provider.rs module-level #![allow(dead_code)] removed
- [ ] MockEmbedder exists and provides deterministic embeddings
- [ ] HRR integration tests no longer #[ignore]
- [ ] All trajectory Tauri commands exported in invoke_handler

### Phase 6E (E1-E7)
- [ ] EventBus is thread-safe singleton with non-blocking emit
- [ ] TelemetryCollector uses RwLock for concurrent access
- [ ] 8 critical paths in agent.rs emit events
- [ ] SessionRecorder uses separate SQLite DB
- [ ] harness_inject_pause_point is debug-only
- [ ] harness-cli compiles and runs independently
- [ ] All integration tests pass

### Medium (M1-M7)
- [ ] migrate_from_json() exists on SqliteMemoryProvider
- [ ] tiktoken-rs used for token counting (or documented deferral)
- [ ] BudgetConfig with YAML loading exists (or documented deferral)
- [ ] Session schema has all claw-cli compatible fields
- [ ] ThreatScanner exists with regex patterns
- [ ] FrozenSnapshot verify returns detailed result
- [ ] Security integration tests exist

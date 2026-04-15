# ADR-012: Phase 6B Wiring — Connecting Islands to Production Pipeline

**Status**: Proposed
**Date**: 2026-04-15
**Supersedes**: None
**Phase**: P0 (Critical wiring gaps)

---

## Context

Phase 6B (Memory Control Plane) completed implementation of **9 slices** and **9 individual modules**, with all 573 tests passing and clippy clean. However, a critical architectural gap was identified during post-completion review:

> **Only `SqliteMemoryProvider` is wired into the production pipeline.**
> All other modules exist as isolated "islands" — implemented but never called from `agent loop`, `session lifecycle`, or `app initialization`.

### Current State (Islands)

```
main.rs (app init)
  ├─→ create_memory_provider() → SqliteMemoryProvider (✅ wired)
  ├─→ register_builtin_tools(memory tools) (✅ wired)
  └─→ AppState (❌ no memory_provider, no learning, no trajectory)

ConversationRuntime.run_turn()  (agent loop)
  ├─→ Token budget check: simple max_token_budget (❌ not ContextBudget)
  ├─→ API call: no active retrieval (❌ not ActiveRetrievalManager)
  ├─→ Tool execution: no memory injection (❌ not VectorMemoryProvider)
  └─→ Session end: no trajectory, no reflection (❌ not LearningModule)

SessionManager (session lifecycle)
  ├─→ create_session: no trajectory record (❌)
  ├─→ save_session: no learning hooks (❌)
  └─→ delete_session: no cleanup (❌)

compact.rs (compaction)
  └─→ Compacts messages but never triggers WeibullDecay (❌)
```

### Dead Code Inventory

| Module | File(s) | Status | Reason |
|--------|---------|--------|--------|
| VectorMemoryProvider | `providers/vector_provider.rs` | `#![allow(dead_code)]` | No caller |
| LanceDBMemory | `providers/lancedb.rs` | `#![allow(dead_code)]` | No caller |
| FastEmbedProvider | `embedding/fastembed.rs` | `#![allow(dead_code)]` | No caller |
| HybridMemoryProvider | `hrr/integration.rs` | `#![allow(dead_code)]` | No caller |
| ActiveRetrievalManager | `retrieval.rs` | `#![allow(dead_code)]` | No caller |
| LearningModule | `learning/mod.rs` | No `init()` function | Never instantiated |
| TrajectoryManager | `learning/trajectory.rs` | Only Tauri commands | No `record()` caller |
| ReflectionEngine | `learning/reflection.rs` | No caller | No session analysis trigger |
| HolographicStore | `hrr/store.rs` | `#![allow(dead_code)]` | No caller |
| ContextBudget | `runtime/budget.rs` | Exists but not used | Agent uses simple `max_token_budget` |
| FrozenSnapshot | `runtime/snapshot.rs` | Exists but not used | Not captured on session init |
| WeibullDecay | `runtime/episodic_compaction.rs` | Exists but not used | Not triggered in compact.rs |
| WorkingMemory | `memory/working_memory.rs` | Exists but not used | Agent uses raw `Vec<ConversationMessage>` |

### Test Coverage Gap

| Test Category | Status | Issue |
|---------------|--------|-------|
| FastEmbed tests | `#[ignore]` | Requires model download at runtime |
| HRR integration tests | `#[ignore]` | Requires model download at runtime |
| VectorMemoryProvider tests | `#[ignore]` | Requires FastEmbed to work |
| Unit tests (bind/unbind, budget, etc.) | ✅ Pass | No integration coverage |

### Frontend Gap

Frontend `src/` directory search for "memory", "trajectory", "recall" → **zero matches**.
All Phase 6B memory functionality is backend-only with no user-facing interface.

---

## Decision

Create a **dedicated Wiring Phase** (Phase 6BW) that connects all Phase 6B modules into the production pipeline without re-implementing existing code. The wiring follows a four-layer approach:

### Layer 1: App Initialization Wiring (P0)

Wire memory providers and learning modules into `AppState` and `main.rs` startup.

#### 1.1 Extend AppState with Memory Infrastructure

**File: `src-tauri/src/commands/mod.rs`** (line 18)

```rust
// BEFORE:
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: Arc<Mutex<HashMap<String, Sender<PermissionPromptDecision>>>>,
    pub permission_overrides: Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    // ...
}

// AFTER:
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: Arc<Mutex<HashMap<String, Sender<PermissionPromptDecision>>>>,
    pub permission_overrides: Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub memory_provider: SharedMemoryProvider,       // NEW
    pub context_budget: ContextBudget,               // NEW
    pub trajectory_manager: Option<Arc<TrajectoryManager>>, // NEW
    pub learning_module: Option<Arc<Mutex<LearningModule>>>, // NEW
    // ...
}
```

#### 1.2 Wire into main.rs Initialization

**File: `src-tauri/src/main.rs`** (line 115-121)

```rust
// BEFORE:
let memory_provider = create_memory_provider();
modules::tools::register_builtin_tools(&tool_registry, memory_provider, scheduler_provider);
let project_manager = modules::projects::ProjectManager::new(projects_dir);
let app_state = AppState::new(session_manager, tool_registry, project_manager);

// AFTER:
let memory_provider = create_memory_provider();
modules::tools::register_builtin_tools(&tool_registry, memory_provider.clone(), scheduler_provider);
let project_manager = modules::projects::ProjectManager::new(projects_dir);

// Initialize ContextBudget
let context_budget = ContextBudget::default();

// Initialize TrajectoryManager (optional — graceful fallback if path unavailable)
let trajectory_manager = TrajectoryManager::new(
    if2ai_dir.join("trajectories")
).ok().map(|tm| Arc::new(tm));

// Initialize LearningModule (optional)
let learning_module = trajectory_manager.as_ref().map(|tm| {
    Arc::new(Mutex::new(LearningModule::new(
        memory_provider.clone(),
        tm.clone(),
    )))
});

let app_state = AppState::new(
    session_manager,
    tool_registry,
    project_manager,
    memory_provider,      // NEW
    context_budget,       // NEW
    trajectory_manager,   // NEW
    learning_module,      // NEW
);
```

#### 1.3 Remove dead_code Suppressions on Vector Provider

**File: `src-tauri/src/modules/memory/providers/vector_provider.rs`**

- Remove `#![allow(dead_code)]` from module level
- Replace with `#[allow(dead_code)]` on individual functions that remain unused
- This forces the compiler to demand callers for the public API

#### 1.4 Remove dead_code Suppressions on FastEmbed

**File: `src-tauri/src/modules/memory/embedding/fastembed.rs`**

- Same treatment as vector_provider.rs
- Tests marked `#[ignore]` should remain ignored until a mock embedding provider is added

---

### Layer 2: Agent Loop Wiring (P0)

Wire memory retrieval, budget enforcement, and context injection into `ConversationRuntime`.

#### 2.1 ContextBudget Integration

**File: `src-tauri/src/commands/agent.rs`** (lines 239-247, 772-778)

The current token budget check uses a simple `max_token_budget: Option<usize>` field on `ConversationRuntime`. Replace with `ContextBudget`:

```rust
// In ConversationRuntime struct:
// BEFORE:
max_token_budget: Option<usize>,

// AFTER:
context_budget: Option<ContextBudget>,  // None = no budget enforcement

// In run_turn() loop (line 239-247):
// BEFORE:
if let Some(budget) = self.max_token_budget {
    let estimated_tokens = estimate_session_tokens(&self.session);
    if estimated_tokens > budget { ... }
}

// AFTER:
if let Some(ref budget) = self.context_budget {
    // Check each slot: System (10%), Episodic (20%), Semantic (30%), Working (40%)
    if let Err(e) = budget.validate(&self.session) {
        // Trigger compaction before proceeding
        self.compact(CompactionConfig::default());
    }
}
```

#### 2.2 ActiveRetrievalManager Integration

**File: `src-tauri/src/commands/agent.rs`** (around line 772-786)

Before calling `runtime.run_turn()`, inject context from active retrieval:

```rust
// BEFORE:
let mut runtime = ConversationRuntime::new(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt,
);
let result = runtime.run_turn(user_message.clone(), None);

// AFTER:
// 1. Intent classification
let intent = QueryIntent::classify(&user_message);

// 2. Active retrieval
if let (Some(mem), Some(arm)) = (&state.memory_provider, state.active_retrieval_manager.as_ref()) {
    let context_entries = arm.pre_llm_call(mem, &user_message, intent).await?;
    // Inject context into system prompt
    if !context_entries.is_empty() {
        system_prompt.push(format_relevant_context(&context_entries));
    }
}

// 3. Create runtime and run
let mut runtime = ConversationRuntime::new(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt,
);
let result = runtime.run_turn(user_message.clone(), None);
```

#### 2.3 WorkingMemory Integration

**File: `src-tauri/src/modules/runtime/conversation.rs`** (lines 132-142, 220-227)

Replace the raw `Vec<ConversationMessage>` with a `WorkingMemory` wrapper:

```rust
// Option A: Add WorkingMemory as a field on ConversationRuntime
pub struct ConversationRuntime<C, T> {
    session: Session,
    working_memory: WorkingMemory,  // NEW
    // ...
}

// Option B: Use WorkingMemory in run_turn to limit context sent to LLM
// In run_turn() before building ApiRequest:
let context_messages = self.working_memory.get_context(&self.session.messages);
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: context_messages,  // NOT self.session.messages.clone()
    tools: Some(self.tool_executor.get_definitions()),
};
```

#### 2.4 FrozenSnapshot Integration

**File: `src-tauri/src/commands/agent.rs`** (line 772)

Capture a `FrozenSnapshot` of the system prompt at session start:

```rust
// Before creating runtime:
let snapshot = FrozenSnapshot::capture(&system_prompt);

// After turn completion:
if !snapshot.verify(&current_system_prompt) {
    tracing::warn!("[run_agent_turn] System prompt was modified since session start");
}
```

---

### Layer 3: Session Lifecycle Wiring (P1)

Wire trajectory recording, learning hooks, and WeibullDecay into session boundaries.

#### 3.1 TrajectoryManager Integration

**File: `src-tauri/src/commands/agent.rs`** (line 868-878)

After saving the session, record trajectory:

```rust
// After line 878 (state.session_manager.save_session(...)):
if let Some(ref tm) = state.trajectory_manager {
    if let Ok(trajectory) = Trajectory::from_session(
        &updated_app_session,
        &system_prompt.join("\n"),
        &model_id,
    ) {
        if let Err(e) = tm.record(&trajectory).await {
            tracing::warn!("[run_agent_turn] Failed to record trajectory: {e}");
        }
    }
}
```

#### 3.2 LearningModule Integration

**File: `src-tauri/src/commands/agent.rs`** (line 878+)

After saving trajectory, trigger reflection:

```rust
if let Some(ref lm) = state.learning_module {
    let mut learning = lm.lock().await;
    learning.record_turn(&updated_app_session, success).await;

    // Trigger reflection every N turns (configurable, default: every 5 turns)
    if learning.should_reflect() {
        if let Ok(reflections) = learning.reflect(&updated_app_session).await {
            learning.update_self_model(&reflections).await;
        }
    }
}
```

#### 3.3 WeibullDecay Integration

**File: `src-tauri/src/modules/runtime/compact.rs`**

After compaction removes messages, trigger WeibullDecay on the episodic memory:

```rust
// After compact_session() determines which messages to remove:
let removed_count = original_count - compacted_count;
if removed_count > 0 {
    let decay = WeibullDecay::default(); // lambda=7d, k=1.2
    let importance_adjustments = decay.calculate(removed_count);
    // Apply to SQLite provider
    memory_provider.apply_importance_decay(importance_adjustments).await?;
}
```

---

### Layer 4: Provider Upgrade (P1)

Upgrade the default memory provider from SQLite-only to Vector-backed.

#### 4.1 ProviderManager Memory Registration

**File: `src-tauri/src/modules/api/providers/manager.rs`**

Add memory provider support to ProviderManager:

```rust
// Add to ProviderManager struct:
memory_provider: SharedMemoryProvider,

// Add method:
pub fn with_memory_provider(mut self, memory: SharedMemoryProvider) -> Self {
    self.memory_provider = memory;
    self
}
```

#### 4.2 VectorMemoryProvider as Default

**File: `src-tauri/src/main.rs`** (`create_memory_provider` function, line 41-61)

Upgrade to use VectorMemoryProvider with SQLite fallback:

```rust
fn create_memory_provider() -> SharedMemoryProvider {
    // Try VectorMemoryProvider first (FastEmbed + LanceDB)
    match VectorMemoryProvider::new(default_vector_config()).await {
        Ok(vector) => Arc::new(vector),
        Err(e) => {
            tracing::warn!("[memory] VectorMemoryProvider failed: {e}, falling back to SQLite");
            create_sqlite_provider()
        }
    }
}
```

#### 4.3 HybridMemoryProvider (HRR + LanceDB)

**File: `src-tauri/src/modules/memory/hrr/integration.rs`**

Un-ignore HRR integration tests and wire HybridMemoryProvider as optional enhancement:

```rust
// Feature-gate behind `hrr_enabled` config flag
if config.hrr_enabled {
    let hybrid = HybridMemoryProvider::new(hybrid_config).await?;
    Arc::new(hybrid)
}
```

---

## Implementation Priority

### P0 (Critical — blocks all other layers)

| Priority | Task | File(s) | Description |
|----------|------|---------|-------------|
| 1 | AppState extension | `commands/mod.rs`, `main.rs` | Add memory_provider, context_budget, trajectory_manager, learning_module |
| 2 | ContextBudget check | `commands/agent.rs` (line 772+) | Replace simple budget with ContextBudget |
| 3 | ActiveRetrievalManager | `commands/agent.rs` (line 772+) | Pre-LLM-call context injection |
| 4 | TrajectoryManager record | `commands/agent.rs` (line 868+) | Post-turn trajectory recording |

### P1 (Important — unlocks vector search)

| Priority | Task | File(s) | Description |
|----------|------|---------|-------------|
| 5 | WorkingMemory | `conversation.rs` | Replace Vec with WorkingMemory |
| 6 | FrozenSnapshot | `commands/agent.rs` | Capture/verify system prompt |
| 7 | WeibullDecay | `compact.rs` | Trigger decay on compaction |
| 8 | LearningModule | `commands/agent.rs` | Reflection and self-model updates |

### P2 (Enhancement — algebraic reasoning)

| Priority | Task | File(s) | Description |
|----------|------|---------|-------------|
| 9 | VectorMemoryProvider default | `main.rs` | Upgrade default to vector-backed |
| 10 | HybridMemoryProvider | `main.rs`, `hrr/integration.rs` | HRR + LanceDB hybrid with config flag |
| 11 | HRR test un-ignore | `hrr/integration.rs` | Add mock embedding, remove `#[ignore]` |

### P3 (UX — frontend integration)

| Priority | Task | File(s) | Description |
|----------|------|---------|-------------|
| 12 | Memory Browser UI | `src/components/memory/` | View/search/delete memory entries |
| 13 | Settings: Token Budget | `src/components/settings/` | Configure ContextBudget values |
| 14 | Trajectory Export UI | `src/components/settings/` | Button to export ShareGPT JSONL |
| 15 | Trust Score Display | `src/components/memory/` | Show trust_score on memory cards |

---

## Rationale

### Why a Dedicated Wiring Phase?

1. **Code exists but is invisible**: 12+ modules are fully implemented but never called. Users cannot benefit from the investment.
2. **Tests pass but are misleading**: Unit tests verify individual modules in isolation. No integration test verifies the end-to-end flow from user message → active retrieval → memory injection → LLM call → trajectory recording.
3. **Dead code compounds**: Each new feature that depends on memory will face the same gap — "the module exists but how do I use it?"
4. **Incremental risk is low**: Wiring is additive — we're connecting existing modules, not rewriting them. Each wire can be independently tested and rolled back.

### Why Not Merge into an Existing Phase?

- Phase 6B is already marked `complete` with all slices done
- Wiring touches different files (`main.rs`, `commands/agent.rs`, `AppState`) than Phase 6B implementation files
- Clean separation makes review and rollback easier

### Why Gradual Provider Upgrade (SQLite → Vector → Hybrid)?

1. SQLite is the only production-proven backend right now
2. Vector search requires FastEmbed model download — could fail in offline environments
3. HRR is P2a by design — complement, not replacement
4. Each upgrade should be feature-gated and tested independently

---

## Consequences

### Positive
- All Phase 6B modules become production-visible
- Memory capabilities (semantic search, context budget, trajectory) become actual user-facing features
- Test coverage becomes meaningful (integration tests exercise the full pipeline)
- Future features can build on a wired infrastructure

### Negative
- `main.rs` and `commands/agent.rs` will grow in complexity
- Initialization time increases (FastEmbed model loading, LanceDB setup)
- More failure modes to handle (network timeout for model download, LanceDB corruption)

### Mitigations
- Use `Option<>` for all new AppState fields — graceful fallback if initialization fails
- Feature-gate VectorMemoryProvider and HRR behind config flags
- Add initialization timeout guards for FastEmbed model download
- Log every wiring point with `[memory]` and `[learning]` prefixes for observability

---

## Review Checklist

- [ ] AppState includes memory_provider, context_budget, trajectory_manager, learning_module
- [ ] main.rs initializes all four new fields with graceful fallback
- [ ] ContextBudget check replaces simple max_token_budget in agent loop
- [ ] ActiveRetrievalManager runs pre-LLM-call and injects context
- [ ] TrajectoryManager.record() runs post-turn completion
- [ ] LearningModule runs reflection on configurable interval
- [ ] WeibullDecay triggers on compaction
- [ ] WorkingMemory replaces raw Vec in context window
- [ ] FrozenSnapshot captures system prompt at session start
- [ ] All `#![allow(dead_code)]` on vector/HRR modules replaced with targeted `#[allow(dead_code)]`
- [ ] FastEmbed tests remain `#[ignore]` with documented reason
- [ ] New AppState fields are `Option<>` where graceful fallback is needed
- [ ] All wiring points logged with structured tracing
- [ ] No `unwrap()` in wiring code (use `?` or `match` with fallback)
- [ ] cargo fmt + clippy + test all pass

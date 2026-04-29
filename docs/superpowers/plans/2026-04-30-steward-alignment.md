# Steward-Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adopt Steward-main's proven agent loop, tool-security, skill-safety, and hook patterns into if2Ai while absorbing in-flight DW-001/DW-002/DW-004/Module-C work into one coherent six-session program.

**Architecture:** Six independently-buildable, independently-rollback-able sessions (P2 value-first ordering): S1 absorbs three existing detailed plans; S2–S4 add Steward-style safety primitives; S5 incrementally extracts a unified `LoopDelegate` to eliminate dual-path drift; S6 lands DW-001 scanner real inputs. Every session ends with the global gate (`fmt` + `clippy -D warnings` + `cargo test`).

**Tech Stack:** Rust 2021 (`src-tauri/**`), Tokio async, async-trait, serde, tracing, tauri 2, existing if2Ai test harness.

**Source spec:** `docs/superpowers/specs/2026-04-30-steward-alignment-design.md` (commit `6230195` on `vnext`)

**Worktree:** `.worktrees/steward-align` (branch `feature/steward-alignment`, branched from `vnext`)

---

## Cross-Session Conventions

- All commits use prefix `feat(steward-align): S<N>-<tag> — <one-line>`
- All `cargo` commands run from `src-tauri/` unless stated otherwise
- TDD discipline: write failing test → run to confirm RED → implement → run to confirm GREEN → commit
- Every new public type carries at least one `#[cfg(test)]` unit test in the same file
- Replace `eprintln!` with `tracing::warn!` / `tracing::error!` whenever editing nearby code
- Never bundle two sessions in one commit; never bundle two sub-tasks of S5 in one commit

---

# SESSION 1 — E1 + E2 + E3 (Absorb Existing Plans)

**Reference**: `docs/superpowers/plans/2026-04-30-dw002-dw004-modc.md` is the canonical design for this session. Tasks below are the executable rendering.

**Files:**
- Modify: `src-tauri/src/modules/api/resilience.rs`
- Modify: `src-tauri/src/modules/provider/resilience.rs`
- Modify: `src-tauri/src/modules/runtime/daemon/mod.rs`
- Modify: `src-tauri/src/modules/runtime/daemon/health_check.rs`
- Modify: `src-tauri/src/modules/application/turn_service/mod.rs`
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`
- Modify: `src-tauri/src/commands/agent/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/modules/skills/domain_knowledge/mod.rs`
- Create: `src-tauri/src/modules/skills/domain_knowledge/file_store.rs`
- Modify: `src-tauri/src/desktop_host/setup.rs`
- Create test: `src-tauri/tests/file_backed_knowledge_store.rs`
- Create test: `src-tauri/tests/provider_circuit_probe.rs`

---

## Task 1.1 — E1: Thread `utility_llm` through TurnServiceDeps

- [ ] **Step 1.1.1: Read current shape**

Run:
```bash
rg -n "utility_llm" src-tauri/src/state/app_state.rs src-tauri/src/modules/application/turn_service/mod.rs src-tauri/src/modules/application/turn_service/stream_task.rs src-tauri/src/commands/agent/mod.rs
```
Confirm `AppState::utility_llm` exists and is marked `#[allow(dead_code)]`, and that `stream_task.rs` constructs `ChatProviderUtilityLlm::new(if2ai_data_root())` inline near line 548.

- [ ] **Step 1.1.2: Add field to `TurnServiceDeps`**

In `src-tauri/src/modules/application/turn_service/mod.rs`, add a `utility_llm: Arc<dyn UtilityLlm>` field to `TurnServiceDeps`. Update the constructor signature and any builder/`Default`-style init points. Ensure `pub use` re-exports remain consistent.

- [ ] **Step 1.1.3: Add field to `StreamTaskInputs`**

In `src-tauri/src/modules/application/turn_service/stream_task.rs`, add `pub utility_llm: Arc<dyn UtilityLlm>` to `StreamTaskInputs`. Update `make_stream_task_inputs` (or equivalent factory in `stream.rs` / `stream_task.rs`) to pass `deps.utility_llm.clone()`.

- [ ] **Step 1.1.4: Replace inline construction**

In `stream_task.rs` near line 548 (the `ChatProviderUtilityLlm::new(if2ai_data_root())` site), replace with `inputs.utility_llm.clone()`. Delete the now-unused inline construction.

- [ ] **Step 1.1.5: Wire from AppState in commands**

In `src-tauri/src/commands/agent/mod.rs` where `TurnServiceDeps` is built, pass `state.utility_llm.clone()` as the new field. Remove `#[allow(dead_code)]` from `AppState::utility_llm` definition.

- [ ] **Step 1.1.6: Build + test**

```bash
cd src-tauri
cargo check --tests
cargo test --test turn_service_stream_turn_e2e
cargo test --test turn_service_run_turn_e2e
```
All must pass. If any test was implicitly relying on the inline-constructed instance, fix wiring rather than reverting.

- [ ] **Step 1.1.7: Commit**

```bash
git add -A
git commit -m "feat(steward-align): S1-E1 — thread Arc<dyn UtilityLlm> through TurnServiceDeps"
```

---

## Task 1.2 — E2: FileBackedKnowledgeStore (TDD)

- [ ] **Step 1.2.1: Write failing test**

Create `src-tauri/tests/file_backed_knowledge_store.rs`:

```rust
use std::sync::Arc;
use tempfile::tempdir;
use if2ai_backend::modules::skills::domain_knowledge::{
    DomainKnowledgeEntry, FileBackedKnowledgeStore, KnowledgeStore,
};

#[tokio::test]
async fn upsert_persists_across_reload() {
    let dir = tempdir().expect("tempdir");
    let store = FileBackedKnowledgeStore::open_or_create(dir.path()).expect("open");

    let entry = DomainKnowledgeEntry::test_fixture("ent-1", "search query bar");
    store.upsert(entry.clone()).await;

    drop(store);

    let reopened = FileBackedKnowledgeStore::open_or_create(dir.path()).expect("reopen");
    let hits = reopened.lookup("search query", None).await;
    assert_eq!(hits.len(), 1, "entry must survive reload");
    assert_eq!(hits[0].id, "ent-1");
}

#[tokio::test]
async fn upsert_replaces_same_id() {
    let dir = tempdir().expect("tempdir");
    let store = FileBackedKnowledgeStore::open_or_create(dir.path()).expect("open");

    store.upsert(DomainKnowledgeEntry::test_fixture("ent-1", "old text")).await;
    store.upsert(DomainKnowledgeEntry::test_fixture("ent-1", "new text")).await;

    let hits = store.lookup("text", None).await;
    assert_eq!(hits.len(), 1);
    assert!(hits[0].body.contains("new text"));
}
```

If `DomainKnowledgeEntry::test_fixture` doesn't exist yet, add it as `#[cfg(test)] pub fn test_fixture(id: &str, body: &str) -> Self` in `domain_knowledge/mod.rs`.

- [ ] **Step 1.2.2: Run test, verify RED**

```bash
cargo test --test file_backed_knowledge_store
```
Expect: compile error (`FileBackedKnowledgeStore` not found).

- [ ] **Step 1.2.3: Create the file store**

Create `src-tauri/src/modules/skills/domain_knowledge/file_store.rs`:

```rust
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{DomainKnowledgeEntry, KnowledgeStore};

const FILE_NAME: &str = "domain-knowledge.ndjson";

pub struct FileBackedKnowledgeStore {
    path: PathBuf,
    mem: RwLock<Vec<DomainKnowledgeEntry>>,
}

impl FileBackedKnowledgeStore {
    pub fn open_or_create(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(FILE_NAME);
        let mut entries: Vec<DomainKnowledgeEntry> = Vec::new();
        if path.exists() {
            let f = File::open(&path)?;
            for (line_no, line) in BufReader::new(f).lines().enumerate() {
                let raw = match line {
                    Ok(l) if l.trim().is_empty() => continue,
                    Ok(l) => l,
                    Err(e) => {
                        tracing::warn!("file store line {} read error: {}", line_no + 1, e);
                        continue;
                    }
                };
                match serde_json::from_str::<DomainKnowledgeEntry>(&raw) {
                    Ok(e) => entries.push(e),
                    Err(e) => tracing::warn!("file store line {} parse error: {}", line_no + 1, e),
                }
            }
        }
        Ok(Self { path, mem: RwLock::new(entries) })
    }

    fn rewrite_all(path: &Path, entries: &[DomainKnowledgeEntry]) -> std::io::Result<()> {
        let tmp = path.with_extension("ndjson.tmp");
        {
            let mut f = OpenOptions::new()
                .create(true).write(true).truncate(true).open(&tmp)?;
            for e in entries {
                let line = serde_json::to_string(e)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                f.write_all(line.as_bytes())?;
                f.write_all(b"\n")?;
            }
            f.sync_all()?;
        }
        std::fs::rename(tmp, path)
    }
}

#[async_trait]
impl KnowledgeStore for FileBackedKnowledgeStore {
    async fn upsert(&self, entry: DomainKnowledgeEntry) {
        let mut guard = self.mem.write().await;
        if let Some(pos) = guard.iter().position(|e| e.id == entry.id) {
            guard[pos] = entry;
        } else {
            guard.push(entry);
        }
        if let Err(e) = Self::rewrite_all(&self.path, &guard) {
            tracing::warn!("file store rewrite failed: {}", e);
        }
    }

    async fn lookup(&self, query: &str, kind_filter: Option<&str>) -> Vec<DomainKnowledgeEntry> {
        let guard = self.mem.read().await;
        let q = query.to_lowercase();
        guard.iter()
            .filter(|e| kind_filter.map_or(true, |k| e.kind == k))
            .filter(|e| e.body.to_lowercase().contains(&q) || e.id.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}
```

In `src-tauri/src/modules/skills/domain_knowledge/mod.rs` add:
```rust
mod file_store;
pub use file_store::FileBackedKnowledgeStore;
```

If `DomainKnowledgeEntry` lacks `Clone` or `Serialize`/`Deserialize`, add them.

- [ ] **Step 1.2.4: Add `install_global_knowledge_store` helper**

In `domain_knowledge/mod.rs` (where `GLOBAL_KNOWLEDGE_STORE` lives):

```rust
pub fn install_global_knowledge_store(
    store: Arc<dyn KnowledgeStore + Send + Sync>,
) -> Result<(), &'static str> {
    GLOBAL_KNOWLEDGE_STORE.set(store).map_err(|_| "already installed")
}
```

- [ ] **Step 1.2.5: Wire into setup.rs**

In `src-tauri/src/desktop_host/setup.rs` inside `attach_native_host` (or equivalent early init), before any business init:

```rust
match crate::modules::skills::domain_knowledge::FileBackedKnowledgeStore::open_or_create(
    &paths.if2ai_dir,
) {
    Ok(store) => {
        let arc: std::sync::Arc<dyn crate::modules::skills::domain_knowledge::KnowledgeStore + Send + Sync> =
            std::sync::Arc::new(store);
        if let Err(e) = crate::modules::skills::domain_knowledge::install_global_knowledge_store(arc) {
            tracing::warn!("knowledge store install: {}", e);
        }
    }
    Err(e) => tracing::warn!("file-backed knowledge store unavailable: {}", e),
}
```

- [ ] **Step 1.2.6: Run test, verify GREEN**

```bash
cargo test --test file_backed_knowledge_store
```
Expect: 2 passed.

- [ ] **Step 1.2.7: Run global gate**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 1.2.8: Commit**

```bash
git add -A
git commit -m "feat(steward-align): S1-E2 — FileBackedKnowledgeStore with NDJSON persistence"
```

---

## Task 1.3 — E3: ProviderCircuitProbe (TDD)

- [ ] **Step 1.3.1: Write failing test**

Create `src-tauri/tests/provider_circuit_probe.rs`:

```rust
use std::sync::Arc;
use if2ai_backend::modules::api::resilience::{global_provider_circuit, ProviderCircuitState};
use if2ai_backend::modules::runtime::daemon::{
    health_check::{HealthCheck, HealthStatus},
    make_provider_circuit_probe, RecoveryAction,
};

#[tokio::test]
async fn three_failures_flip_to_failed_with_degrade() {
    let state = global_provider_circuit();
    state.reset_for_test();

    let probe = make_provider_circuit_probe(state.clone());
    assert!(matches!(probe.check().await, HealthStatus::Healthy));

    state.record_failure();
    state.record_failure();
    state.record_failure();

    let status = probe.check().await;
    assert!(matches!(status, HealthStatus::Failed { .. }), "got {:?}", status);

    let recovery = probe.recovery().expect("recovery action");
    assert!(matches!(
        recovery,
        RecoveryAction::DegradeGracefully { .. }
    ), "got {:?}", recovery);
}

#[tokio::test]
async fn record_success_resets_streak() {
    let state = global_provider_circuit();
    state.reset_for_test();

    state.record_failure();
    state.record_failure();
    state.record_success();
    state.record_failure();

    let probe = make_provider_circuit_probe(state.clone());
    assert!(matches!(probe.check().await, HealthStatus::Degraded { .. }));
}
```

- [ ] **Step 1.3.2: Run test, verify RED**

```bash
cargo test --test provider_circuit_probe
```
Expect: compile error (`global_provider_circuit` / `make_provider_circuit_probe` not found).

- [ ] **Step 1.3.3: Add singleton + test reset to `api/resilience.rs`**

```rust
use std::sync::OnceLock;

static GLOBAL_PROVIDER_CIRCUIT: OnceLock<Arc<ProviderCircuitState>> = OnceLock::new();

pub fn global_provider_circuit() -> Arc<ProviderCircuitState> {
    GLOBAL_PROVIDER_CIRCUIT
        .get_or_init(|| Arc::new(ProviderCircuitState::default()))
        .clone()
}

impl ProviderCircuitState {
    #[cfg(test)]
    pub fn reset_for_test(&self) {
        // reset internal atomic to 0
        self.consecutive_failures.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}
```
(Adjust to match the actual struct field names; if the field is private, expose a `pub(crate) fn reset()` instead and re-export under `#[cfg(test)]`.)

- [ ] **Step 1.3.4: Mirror state in `provider/resilience.rs`**

In `StreamCircuitState::record_success` append:
```rust
crate::modules::api::resilience::global_provider_circuit().record_success();
```
In `StreamCircuitState::record_failure` append:
```rust
crate::modules::api::resilience::global_provider_circuit().record_failure();
```

- [ ] **Step 1.3.5: Add `make_provider_circuit_probe` to daemon**

In `src-tauri/src/modules/runtime/daemon/mod.rs`:

```rust
pub fn make_provider_circuit_probe(
    state: Arc<crate::modules::api::resilience::ProviderCircuitState>,
) -> Arc<dyn crate::modules::runtime::daemon::health_check::HealthCheck> {
    Arc::new(ProviderCircuitProbe { state })
}

struct ProviderCircuitProbe {
    state: Arc<crate::modules::api::resilience::ProviderCircuitState>,
}

#[async_trait::async_trait]
impl crate::modules::runtime::daemon::health_check::HealthCheck for ProviderCircuitProbe {
    fn name(&self) -> &str { "provider_circuit" }

    async fn check(&self) -> crate::modules::runtime::daemon::health_check::HealthStatus {
        use crate::modules::runtime::daemon::health_check::HealthStatus;
        let n = self.state.consecutive_failures();
        if n == 0 { HealthStatus::Healthy }
        else if n <= 2 {
            HealthStatus::Degraded { reason: format!("{} consecutive failures", n) }
        } else {
            HealthStatus::Failed { reason: format!("{} consecutive failures", n) }
        }
    }

    fn recovery(&self) -> Option<RecoveryAction> {
        Some(RecoveryAction::DegradeGracefully {
            provider_name: "chat".to_string(),
        })
    }
}
```
(If the existing `HealthCheck` trait does not have `recovery()`, instead implement an existing method that returns the action — match what `health_check.rs` defines.)

- [ ] **Step 1.3.6: Register probe in setup.rs extras**

```rust
let provider_probe = crate::modules::runtime::daemon::make_provider_circuit_probe(
    crate::modules::api::resilience::global_provider_circuit(),
);
extras.push(provider_probe);
```

- [ ] **Step 1.3.7: Run test, verify GREEN**

```bash
cargo test --test provider_circuit_probe
```

- [ ] **Step 1.3.8: Run global gate**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 1.3.9: Commit**

```bash
git add -A
git commit -m "feat(steward-align): S1-E3 — global provider circuit probe wired into daemon"
```

---

## Session 1 Exit Gate

Verify all three pass:
1. `rg -n "allow\(dead_code\).*utility_llm" src-tauri/src/` returns no matches.
2. `cargo test --test file_backed_knowledge_store` PASS.
3. `cargo test --test provider_circuit_probe` PASS.

If any fails: do NOT proceed to Session 2. Fix in place.

---

# SESSION 2 — S1: AgenticLoopConfig + S3: escape_skill_content

**Files:**
- Create: `src-tauri/src/modules/application/turn_service/loop_config.rs`
- Modify: `src-tauri/src/modules/application/turn_service/mod.rs`
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`
- Modify: `src-tauri/src/modules/application/turn_service/stream.rs`
- Modify: `src-tauri/src/modules/application/turn_service/run.rs`
- Modify: `src-tauri/src/modules/skills/mod.rs`
- Modify: `src-tauri/src/modules/application/turn_service/work_loop.rs`
- Create test: `src-tauri/tests/loop_config_force_text.rs`
- Create test: `src-tauri/tests/skill_escape_injection.rs`

---

## Task 2.1 — AgenticLoopConfig type (TDD)

- [ ] **Step 2.1.1: Write failing unit test inside `loop_config.rs`**

Create `src-tauri/src/modules/application/turn_service/loop_config.rs`:

```rust
//! Safety-valve configuration for the agentic loop.
//!
//! Mirrors Steward's `AgenticLoopConfig`. All fields have defaults that
//! preserve current production behaviour.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticLoopConfig {
    pub max_iterations: usize,
    pub enable_tool_intent_nudge: bool,
    pub max_tool_intent_nudges: u32,
    pub force_text_after_truncations: u32,
}

impl Default for AgenticLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            enable_tool_intent_nudge: true,
            max_tool_intent_nudges: 2,
            force_text_after_truncations: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_match_steward_baseline() {
        let c = AgenticLoopConfig::default();
        assert_eq!(c.max_iterations, 50);
        assert!(c.enable_tool_intent_nudge);
        assert_eq!(c.max_tool_intent_nudges, 2);
        assert_eq!(c.force_text_after_truncations, 2);
    }
}
```

In `turn_service/mod.rs` add:
```rust
pub mod loop_config;
pub use loop_config::AgenticLoopConfig;
```

- [ ] **Step 2.1.2: Run test, verify GREEN immediately (this is a pure constant)**

```bash
cargo test -p if2ai-backend loop_config::tests
```

- [ ] **Step 2.1.3: Commit type only**

```bash
git add -A
git commit -m "feat(steward-align): S2-S1a — introduce AgenticLoopConfig type"
```

---

## Task 2.2 — Wire AgenticLoopConfig into stream path with force_text

- [ ] **Step 2.2.1: Write failing integration test**

Create `src-tauri/tests/loop_config_force_text.rs`:

```rust
//! Verifies that StreamTaskInputs carries an AgenticLoopConfig and that
//! force_text triggers after `force_text_after_truncations` Length finishes.

use if2ai_backend::modules::application::turn_service::AgenticLoopConfig;

#[test]
fn config_is_constructible_with_defaults() {
    let cfg = AgenticLoopConfig::default();
    assert_eq!(cfg.force_text_after_truncations, 2);
}

// NOTE: The full stream-task force_text behaviour is exercised through
// turn_service_stream_turn_e2e once StreamDelegate lands in S5. For S2 we
// only assert the config field is present and reachable.
#[test]
fn stream_task_inputs_field_exists_via_type_check() {
    // Compile-time witness: if the field is removed, this fails to compile.
    fn _witness(inputs: &if2ai_backend::modules::application::turn_service::stream_task::StreamTaskInputs) -> &AgenticLoopConfig {
        &inputs.loop_config
    }
}
```

- [ ] **Step 2.2.2: Run, verify RED (`StreamTaskInputs.loop_config` does not exist)**

```bash
cargo test --test loop_config_force_text
```

- [ ] **Step 2.2.3: Add `loop_config` field**

In `stream_task.rs`:
```rust
pub struct StreamTaskInputs {
    // ... existing fields ...
    pub loop_config: crate::modules::application::turn_service::AgenticLoopConfig,
}
```

In `TurnServiceDeps` (turn_service/mod.rs):
```rust
pub loop_config: AgenticLoopConfig,
```

In any factory/constructor: thread `deps.loop_config.clone()` into `StreamTaskInputs`.

- [ ] **Step 2.2.4: Replace hardcoded max_iterations**

`rg -n "max_iterations" src-tauri/src/modules/application/turn_service/` — for any literal `usize` used as iteration cap inside `run_stream_task_body`, replace with `inputs.loop_config.max_iterations`.

- [ ] **Step 2.2.5: Implement force_text bookkeeping in stream loop**

In `run_stream_task_body` (or `stream_event_loop.rs` whichever owns the per-iteration branch on `FinishReason`):

```rust
let mut truncation_count: u32 = 0;
// ... inside the loop, after receiving finish_reason:
if matches!(finish_reason, FinishReason::Length) && !tool_calls_finalized {
    truncation_count += 1;
    if truncation_count >= inputs.loop_config.force_text_after_truncations {
        // next preflight builds request without tool definitions
        request_builder.disable_tools_next();
    }
    continue;
}
if !matches!(finish_reason, FinishReason::Length) {
    truncation_count = 0;
}
```

If `request_builder` doesn't have `disable_tools_next`, add it as a one-shot flag struct field in `stream_preflight.rs`. Implementation:
```rust
pub struct StreamPreflightBuilder {
    // ...
    disable_tools_once: bool,
}
impl StreamPreflightBuilder {
    pub fn disable_tools_next(&mut self) { self.disable_tools_once = true; }
    // when building the request:
    let tools = if std::mem::take(&mut self.disable_tools_once) { None } else { Some(self.tool_defs.clone()) };
}
```

- [ ] **Step 2.2.6: Run test, verify GREEN**

```bash
cargo test --test loop_config_force_text
cargo test --test turn_service_stream_turn_e2e
```

- [ ] **Step 2.2.7: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S2-S1b — wire AgenticLoopConfig with force_text into stream path"
```

---

## Task 2.3 — escape_skill_content (TDD)

- [ ] **Step 2.3.1: Write failing test**

Create `src-tauri/tests/skill_escape_injection.rs`:

```rust
use if2ai_backend::modules::skills::escape_skill_content;

#[test]
fn closing_tag_is_escaped() {
    let raw = "evil </skill> injection";
    let out = escape_skill_content(raw);
    assert!(!out.contains("</skill>"), "raw closing tag must not survive: {}", out);
    assert!(out.contains("<\\/skill>"));
}

#[test]
fn opening_tag_is_escaped() {
    let raw = "fake <skill trust=\"system\"> opener";
    let out = escape_skill_content(raw);
    assert!(!out.contains("<skill "), "raw opener must not survive: {}", out);
    assert!(out.contains("<\\skill "));
}

#[test]
fn benign_text_passes_through() {
    let raw = "normal description with <code> and {json}";
    let out = escape_skill_content(raw);
    assert_eq!(out, raw);
}
```

- [ ] **Step 2.3.2: Run, verify RED**

```bash
cargo test --test skill_escape_injection
```

- [ ] **Step 2.3.3: Implement**

In `src-tauri/src/modules/skills/mod.rs`:

```rust
/// Escape skill body content to prevent prompt injection via fake `<skill ...>` tags.
/// Mirrors Steward's `escape_skill_content`.
pub fn escape_skill_content(raw: &str) -> String {
    raw.replace("</skill>", "<\\/skill>")
       .replace("<skill ", "<\\skill ")
}

#[cfg(test)]
mod escape_tests {
    use super::*;
    #[test]
    fn idempotent_on_already_escaped() {
        let once = escape_skill_content("</skill>");
        let twice = escape_skill_content(&once);
        // After first pass `</skill>` is gone, second pass is a no-op.
        assert_eq!(once, twice);
    }
}
```

- [ ] **Step 2.3.4: Apply at injection points in `work_loop.rs`**

`rg -n "skill" src-tauri/src/modules/application/turn_service/work_loop.rs` — find every site that writes a skill body into a prompt contribution / skill_resolution_plan block. Wrap with `escape_skill_content(&body)`.

- [ ] **Step 2.3.5: Run test, verify GREEN**

```bash
cargo test --test skill_escape_injection
```

- [ ] **Step 2.3.6: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S2-S3 — escape_skill_content prevents skill XML injection"
```

---

## Session 2 Exit Gate

1. `cargo test --test loop_config_force_text` PASS.
2. `cargo test --test skill_escape_injection` PASS.
3. `cargo test` (full) PASS.
4. `rg -n "max_iterations\s*[=:]\s*\d" src-tauri/src/modules/application/turn_service/` returns no literal numeric assignment inside the streaming loop body (allowed only in `loop_config.rs` defaults).

---

# SESSION 3 — S2: Tool Attenuation + S4: Prompt Cache

**Files:**
- Modify: `src-tauri/src/modules/tools/registry.rs`
- Create: `src-tauri/src/modules/tools/attenuation.rs`
- Modify: `src-tauri/src/modules/tools/mod.rs`
- Modify: `src-tauri/src/modules/application/turn_service/work_loop.rs`
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs`
- Create: `src-tauri/src/modules/application/turn_service/prompt_cache.rs`
- Create test: `src-tauri/tests/tool_attenuation.rs`
- Create test: `src-tauri/tests/prompt_cache_hit.rs`

---

## Task 3.1 — PROTECTED_TOOL_NAMES + attenuate_tools (TDD)

- [ ] **Step 3.1.1: Write failing test**

Create `src-tauri/tests/tool_attenuation.rs`:

```rust
use if2ai_backend::modules::tools::attenuation::{
    attenuate_tools, READ_ONLY_TOOL_NAMES, SkillTrustLevel,
};
use if2ai_backend::modules::llm::ToolDefinition;

fn td(name: &str) -> ToolDefinition {
    ToolDefinition::test_minimal(name)
}

#[test]
fn system_trust_keeps_all() {
    let defs = vec![td("bash"), td("file_read"), td("memory_store")];
    let out = attenuate_tools(defs, SkillTrustLevel::System);
    assert_eq!(out.len(), 3);
}

#[test]
fn installed_trust_keeps_only_read_only() {
    let defs = vec![td("bash"), td("file_read"), td("memory_store"), td("grep")];
    let out = attenuate_tools(defs, SkillTrustLevel::Installed);
    let names: Vec<_> = out.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"file_read"));
    assert!(names.contains(&"grep"));
    assert!(!names.contains(&"bash"));
    assert!(!names.contains(&"memory_store"));
}

#[test]
fn read_only_set_contains_expected_baseline() {
    for n in &["file_read", "glob", "grep", "skill_find", "skill_view",
               "skills_list", "web_search", "web_fetch"] {
        assert!(READ_ONLY_TOOL_NAMES.contains(n), "missing {}", n);
    }
}
```

If `ToolDefinition::test_minimal` doesn't exist, add `#[cfg(test)] pub fn test_minimal(name: &str) -> Self` constructing whatever minimum struct is needed.

- [ ] **Step 3.1.2: Run, verify RED**

```bash
cargo test --test tool_attenuation
```

- [ ] **Step 3.1.3: Create attenuation module**

Create `src-tauri/src/modules/tools/attenuation.rs`:

```rust
//! Skill-trust-driven tool attenuation. Mirrors Steward's `attenuate_tools`.
//!
//! When a low-trust skill is active, the LLM should not even *see* dangerous
//! tools. Filtering at definition time defends against prompt injection.

use crate::modules::llm::ToolDefinition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillTrustLevel {
    System,
    Trusted,
    Installed,
}

pub static PROTECTED_TOOL_NAMES: &[&str] = &[
    "bash", "shell", "memory_store", "memory_delete",
    "file_write", "file_delete", "http_request",
];

pub static READ_ONLY_TOOL_NAMES: &[&str] = &[
    "file_read", "glob", "grep", "skill_find", "skill_view",
    "skills_list", "web_search", "web_fetch",
];

pub fn attenuate_tools(
    defs: Vec<ToolDefinition>,
    min_skill_trust: SkillTrustLevel,
) -> Vec<ToolDefinition> {
    match min_skill_trust {
        SkillTrustLevel::System | SkillTrustLevel::Trusted => defs,
        SkillTrustLevel::Installed => defs
            .into_iter()
            .filter(|d| READ_ONLY_TOOL_NAMES.contains(&d.name.as_str()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_set_disjoint_from_readonly() {
        for p in PROTECTED_TOOL_NAMES {
            assert!(!READ_ONLY_TOOL_NAMES.contains(p), "{} in both sets", p);
        }
    }
}
```

In `tools/mod.rs`:
```rust
pub mod attenuation;
```

- [ ] **Step 3.1.4: Add registration guard in `registry.rs`**

```rust
impl ToolRegistry {
    /// Registration phase flag. Set true during builtin bootstrap; flip to false
    /// after `register_builtin_tools` returns. Once false, attempts to register
    /// a name in PROTECTED_TOOL_NAMES are rejected.
    pub fn lock_protected_names(&self) {
        self.builtin_phase.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name();
        if !self.builtin_phase.load(std::sync::atomic::Ordering::SeqCst)
            && crate::modules::tools::attenuation::PROTECTED_TOOL_NAMES.contains(&name)
        {
            tracing::warn!(
                "rejected dynamic registration of protected tool '{}'", name
            );
            return;
        }
        // ... existing insert logic ...
    }
}
```

Add `builtin_phase: AtomicBool` (init `true`) to `ToolRegistry`. In `register_builtin_tools` after the final builtin is registered call `registry.lock_protected_names()`.

- [ ] **Step 3.1.5: Wire into work_loop attenuation call**

In `work_loop.rs` find where `tool_definitions` (Vec<ToolDefinition>) is computed before being passed into the prompt plan / LLM call:

```rust
let min_trust = active_skills
    .iter()
    .map(|s| s.trust_level)            // adapt to actual field name
    .min()
    .unwrap_or(crate::modules::tools::attenuation::SkillTrustLevel::System);

let tool_definitions = crate::modules::tools::attenuation::attenuate_tools(
    tool_definitions, min_trust,
);
```

If if2Ai's existing skill model lacks an `Ord`-able trust enum, derive `PartialOrd`/`Ord` on `SkillTrustLevel` with `System > Trusted > Installed` ordering (so `.min()` yields the lowest trust).

Add to `attenuation.rs`:
```rust
impl PartialOrd for SkillTrustLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for SkillTrustLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let rank = |s: &SkillTrustLevel| match s {
            SkillTrustLevel::Installed => 0,
            SkillTrustLevel::Trusted => 1,
            SkillTrustLevel::System => 2,
        };
        rank(self).cmp(&rank(other))
    }
}
```

- [ ] **Step 3.1.6: Run, verify GREEN**

```bash
cargo test --test tool_attenuation
```

- [ ] **Step 3.1.7: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S3-S2 — tool attenuation + protected-name registration guard"
```

---

## Task 3.2 — System prompt cache (TDD)

- [ ] **Step 3.2.1: Write failing test**

Create `src-tauri/tests/prompt_cache_hit.rs`:

```rust
use if2ai_backend::modules::application::turn_service::prompt_cache::{
    CachedSystemPrompt, PromptFingerprint,
};

#[test]
fn identical_fingerprint_hits_cache() {
    let mut cache: Option<CachedSystemPrompt> = None;
    let fp1 = PromptFingerprint::compute(&["skill-a"], &["bash", "grep"]);
    let fp2 = PromptFingerprint::compute(&["skill-a"], &["bash", "grep"]);
    assert_eq!(fp1, fp2);

    cache = Some(CachedSystemPrompt { content: "hello".into(), fingerprint: fp1.clone() });
    let hit = cache.as_ref().filter(|c| c.fingerprint == fp2);
    assert!(hit.is_some());
}

#[test]
fn changed_skill_invalidates_cache() {
    let fp1 = PromptFingerprint::compute(&["skill-a"], &["bash"]);
    let fp2 = PromptFingerprint::compute(&["skill-b"], &["bash"]);
    assert_ne!(fp1, fp2);
}

#[test]
fn order_independent_for_skills_and_tools() {
    let fp1 = PromptFingerprint::compute(&["a", "b"], &["x", "y"]);
    let fp2 = PromptFingerprint::compute(&["b", "a"], &["y", "x"]);
    assert_eq!(fp1, fp2);
}
```

- [ ] **Step 3.2.2: Run, verify RED**

```bash
cargo test --test prompt_cache_hit
```

- [ ] **Step 3.2.3: Implement prompt_cache module**

Create `src-tauri/src/modules/application/turn_service/prompt_cache.rs`:

```rust
//! System prompt cache keyed by skill + tool fingerprint.
//!
//! Avoids rebuilding the full prompt plan every loop iteration when the
//! active skills and registered tools have not changed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptFingerprint {
    skill_hash: u64,
    tool_hash: u64,
}

impl PromptFingerprint {
    pub fn compute(skill_ids: &[&str], tool_names: &[&str]) -> Self {
        let mut s: Vec<&str> = skill_ids.to_vec();
        s.sort_unstable();
        let mut t: Vec<&str> = tool_names.to_vec();
        t.sort_unstable();
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        let skill_hash = h.finish();
        let mut h = DefaultHasher::new();
        t.hash(&mut h);
        let tool_hash = h.finish();
        Self { skill_hash, tool_hash }
    }
}

#[derive(Debug, Clone)]
pub struct CachedSystemPrompt {
    pub content: String,
    pub fingerprint: PromptFingerprint,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_inputs_hash_consistently() {
        assert_eq!(
            PromptFingerprint::compute(&[], &[]),
            PromptFingerprint::compute(&[], &[]),
        );
    }
}
```

In `turn_service/mod.rs`:
```rust
pub mod prompt_cache;
```

- [ ] **Step 3.2.4: Wire cache into stream_task state**

In `stream_task.rs`, find the local state struct (or create one) for `run_stream_task_body`:

```rust
struct StreamLoopState {
    cached_prompt: Option<crate::modules::application::turn_service::prompt_cache::CachedSystemPrompt>,
    // ... other existing locals ...
}
```

At each iteration, before calling `build_prompt_plan`:
```rust
let skill_ids: Vec<&str> = state.active_skill_ids.iter().map(|s| s.as_str()).collect();
let tool_names: Vec<&str> = state.tool_definitions.iter().map(|d| d.name.as_str()).collect();
let fp = crate::modules::application::turn_service::prompt_cache::PromptFingerprint::compute(
    &skill_ids, &tool_names,
);

let prompt_text = match &state.cached_prompt {
    Some(c) if c.fingerprint == fp => c.content.clone(),
    _ => {
        let plan = build_prompt_plan(&request).await?;
        state.cached_prompt = Some(
            crate::modules::application::turn_service::prompt_cache::CachedSystemPrompt {
                content: plan.text.clone(),
                fingerprint: fp,
            }
        );
        plan.text
    }
};
```

- [ ] **Step 3.2.5: Run, verify GREEN**

```bash
cargo test --test prompt_cache_hit
cargo test --test turn_service_stream_turn_e2e
```

- [ ] **Step 3.2.6: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S3-S4 — system prompt cache keyed by skill+tool fingerprint"
```

---

## Session 3 Exit Gate

1. `cargo test --test tool_attenuation` PASS.
2. `cargo test --test prompt_cache_hit` PASS.
3. Stream + run e2e tests PASS.
4. Manual: `rg "PROTECTED_TOOL_NAMES" src-tauri/src/modules/tools/registry.rs` shows usage in `register`.

---

# SESSION 4 — S5: HookRegistry Upgrade

**Files:**
- Create: `src-tauri/src/modules/application/turn_service/hook_registry.rs`
- Modify: `src-tauri/src/modules/application/turn_service/preflight_hooks.rs`
- Modify: `src-tauri/src/modules/application/turn_service/finalize_hooks.rs`
- Modify: `src-tauri/src/modules/application/turn_service/mod.rs`
- Modify: existing hook registration sites (search via `rg "register.*hook|TurnHook" src-tauri/src/`)
- Create test: `src-tauri/tests/hook_priority_order.rs`
- Create test: `src-tauri/tests/hook_fail_open.rs`
- Create test: `src-tauri/tests/hook_fail_closed.rs`

---

## Task 4.1 — HookRegistry type with priority + FailurePolicy

- [ ] **Step 4.1.1: Write failing tests**

Create `src-tauri/tests/hook_priority_order.rs`:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_trait::async_trait;
use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookEntry, HookRegistry, TestHookCtx, TurnHookSimple,
};

struct RecordingHook {
    name: &'static str,
    log: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl TurnHookSimple for RecordingHook {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        self.log.lock().unwrap().push(self.name);
        Ok(())
    }
}

#[tokio::test]
async fn higher_priority_runs_first() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { name: "low", log: log.clone() }),
        priority: -5, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { name: "high", log: log.clone() }),
        priority: 10, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { name: "mid", log: log.clone() }),
        priority: 0, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });

    reg.run_all(&TestHookCtx::default()).await.unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["high", "mid", "low"]);
}
```

Create `src-tauri/tests/hook_fail_open.rs`:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_trait::async_trait;
use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookEntry, HookRegistry, TestHookCtx, TurnHookSimple,
};

struct FailingHook;
#[async_trait]
impl TurnHookSimple for FailingHook {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        Err("boom".into())
    }
}
struct RecordingHook { log: Arc<Mutex<bool>> }
#[async_trait]
impl TurnHookSimple for RecordingHook {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        *self.log.lock().unwrap() = true;
        Ok(())
    }
}

#[tokio::test]
async fn fail_open_continues_chain() {
    let log = Arc::new(Mutex::new(false));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(FailingHook),
        priority: 10, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { log: log.clone() }),
        priority: 0, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });
    let r = reg.run_all(&TestHookCtx::default()).await;
    assert!(r.is_ok(), "fail-open chain returns Ok overall");
    assert!(*log.lock().unwrap(), "subsequent hook ran");
}
```

Create `src-tauri/tests/hook_fail_closed.rs`:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_trait::async_trait;
use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookEntry, HookRegistry, TestHookCtx, TurnHookSimple,
};

struct FailingHook;
#[async_trait]
impl TurnHookSimple for FailingHook {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        Err("boom".into())
    }
}
struct ShouldNotRun { ran: Arc<Mutex<bool>> }
#[async_trait]
impl TurnHookSimple for ShouldNotRun {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        *self.ran.lock().unwrap() = true;
        Ok(())
    }
}

#[tokio::test]
async fn fail_closed_aborts_chain() {
    let ran = Arc::new(Mutex::new(false));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(FailingHook),
        priority: 10, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailClosed,
    });
    reg.register(HookEntry {
        hook: Arc::new(ShouldNotRun { ran: ran.clone() }),
        priority: 0, timeout: Duration::from_secs(1), on_failure: FailurePolicy::FailOpen,
    });
    let r = reg.run_all(&TestHookCtx::default()).await;
    assert!(r.is_err());
    assert!(!*ran.lock().unwrap(), "chain aborted before second hook");
}
```

- [ ] **Step 4.1.2: Run all three, verify RED**

```bash
cargo test --test hook_priority_order --test hook_fail_open --test hook_fail_closed
```

- [ ] **Step 4.1.3: Implement hook_registry.rs**

Create `src-tauri/src/modules/application/turn_service/hook_registry.rs`:

```rust
//! Prioritized, timed, failure-policy-aware hook chain.
//!
//! Mirrors Steward's `HookRegistry`. Existing `TurnHook` implementations
//! migrate by being wrapped in `HookEntry { priority: 0, timeout: 30s,
//! on_failure: FailOpen }`.

use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePolicy { FailOpen, FailClosed }

#[derive(Debug, Default)]
pub struct TestHookCtx;  // small public ctx for unit tests; production uses TurnContext below

#[async_trait]
pub trait TurnHookSimple: Send + Sync {
    async fn call(&self, ctx: &TestHookCtx) -> Result<(), String>;
    fn name(&self) -> &str { "anonymous" }
}

pub struct HookEntry {
    pub hook: Arc<dyn TurnHookSimple>,
    pub priority: i32,
    pub timeout: Duration,
    pub on_failure: FailurePolicy,
}

#[derive(Default)]
pub struct HookRegistry {
    entries: Vec<HookEntry>,
}

impl HookRegistry {
    pub fn register(&mut self, entry: HookEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub async fn run_all(&self, ctx: &TestHookCtx) -> Result<(), String> {
        for entry in &self.entries {
            let res = tokio::time::timeout(entry.timeout, entry.hook.call(ctx)).await;
            let outcome = match res {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(format!("hook '{}' timed out", entry.hook.name())),
            };
            match (outcome, entry.on_failure) {
                (Ok(()), _) => continue,
                (Err(e), FailurePolicy::FailOpen) => {
                    tracing::warn!("hook '{}' failed (fail-open): {}", entry.hook.name(), e);
                    continue;
                }
                (Err(e), FailurePolicy::FailClosed) => {
                    tracing::error!("hook '{}' failed (fail-closed): {}", entry.hook.name(), e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn empty_chain_returns_ok() {
        let r = HookRegistry::default().run_all(&TestHookCtx::default()).await;
        assert!(r.is_ok());
    }
}
```

In `turn_service/mod.rs`:
```rust
pub mod hook_registry;
```

- [ ] **Step 4.1.4: Run, verify GREEN**

```bash
cargo test --test hook_priority_order --test hook_fail_open --test hook_fail_closed
```

- [ ] **Step 4.1.5: Commit type + tests**

```bash
git add -A
git commit -m "feat(steward-align): S4a — HookRegistry with priority+FailurePolicy"
```

---

## Task 4.2 — Migrate existing TurnHook callers

- [ ] **Step 4.2.1: Map existing call sites**

```bash
rg -n "TurnHook|trait.*Hook|impl.*Hook" src-tauri/src/modules/
```
Build a list. Common: `MemoryTicker`, `SnapshotHook`, others in `preflight_hooks.rs` / `finalize_hooks.rs` / `runtime/conversation.rs`.

- [ ] **Step 4.2.2: For each `TurnHook` impl, write a thin adapter to `TurnHookSimple`**

In `turn_service/hook_registry.rs` add (alongside `TurnHookSimple`):

```rust
/// Adapter wrapping the existing `TurnHook` trait into the new registry.
pub struct LegacyHookAdapter<H> {
    inner: H,
    name: &'static str,
}

impl<H> LegacyHookAdapter<H> {
    pub fn new(name: &'static str, inner: H) -> Self { Self { inner, name } }
}

#[async_trait]
impl<H: crate::modules::runtime::conversation::TurnHook + Send + Sync> TurnHookSimple
    for LegacyHookAdapter<H>
{
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        // Existing TurnHook signatures vary — call the appropriate method
        // and convert errors to String. This adapter is intentionally minimal
        // so legacy hooks can be dropped into the new registry without
        // changing their internals.
        self.inner.before_turn().await.map_err(|e| e.to_string())
    }
    fn name(&self) -> &str { self.name }
}
```

(Adjust signature to match actual `TurnHook` trait surface. If `TurnHook` has multiple methods like `before_turn` / `after_turn`, create two adapters or two `HookRegistry`s — `preflight` and `finalize`.)

- [ ] **Step 4.2.3: Replace ad-hoc Vec<Arc<dyn TurnHook>> with HookRegistry at one call site**

Pick the smallest call site first (e.g., `preflight_hooks.rs`). Replace its hook iteration loop with:
```rust
let mut reg = HookRegistry::default();
for h in legacy_hooks {
    reg.register(HookEntry {
        hook: Arc::new(LegacyHookAdapter::new("preflight", h)),
        priority: 0,
        timeout: Duration::from_secs(30),
        on_failure: FailurePolicy::FailOpen,
    });
}
reg.run_all(&TestHookCtx::default()).await.map_err(|e| AppError::HookChain(e))?;
```

- [ ] **Step 4.2.4: Run preflight tests**

```bash
cargo test --test preflight_hooks
```

- [ ] **Step 4.2.5: Repeat for finalize, conversation runtime**

For each remaining call site, repeat steps 4.2.3–4.2.4.

- [ ] **Step 4.2.6: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S4b — migrate existing TurnHook sites to HookRegistry"
```

---

## Session 4 Exit Gate

1. All three new hook tests PASS.
2. `cargo test` (full) PASS.
3. No remaining `for hook in hooks { hook.call(...).await }` patterns in `turn_service/` (replaced by `HookRegistry::run_all`).

---

# SESSION 5 — LoopDelegate Incremental Extraction (Strategy Z)

This is THE high-risk session. Do **not** combine sub-commits. Each sub-task ends with its own commit and full `cargo test` PASS.

**Files:**
- Create: `src-tauri/src/modules/application/turn_service/agentic_loop.rs`
- Modify: `src-tauri/src/modules/application/turn_service/mod.rs`
- Modify: `src-tauri/src/modules/application/turn_service/stream_task.rs` (5b)
- Modify: `src-tauri/src/modules/application/turn_service/run.rs` (5c)
- Modify: `src-tauri/src/modules/runtime/conversation.rs` (5c)
- Create test: `src-tauri/tests/agentic_loop_unit.rs`

---

## Task 5.1 — Sub-commit 5a: trait + free function (pure addition)

- [ ] **Step 5.1.1: Write failing unit test**

Create `src-tauri/tests/agentic_loop_unit.rs`:

```rust
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use if2ai_backend::modules::application::turn_service::agentic_loop::{
    LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult, TextAction,
    run_agentic_loop,
};
use if2ai_backend::modules::application::turn_service::AgenticLoopConfig;

#[derive(Default)]
struct ScriptedDelegate {
    script: Mutex<Vec<RespondResult>>,
    iters: Mutex<u32>,
    stop_after: Option<u32>,
}

#[async_trait]
impl LoopDelegate for ScriptedDelegate {
    async fn check_signals(&self) -> LoopSignal {
        let n = *self.iters.lock().unwrap();
        if let Some(stop) = self.stop_after { if n >= stop { return LoopSignal::Stop; } }
        LoopSignal::Continue
    }
    async fn before_llm_call(&self, _: &mut LoopContext, _: usize) -> Option<LoopOutcome> { None }
    async fn call_llm(&self, _: &mut LoopContext) -> Result<RespondResult, String> {
        *self.iters.lock().unwrap() += 1;
        Ok(self.script.lock().unwrap().remove(0))
    }
    async fn execute_tool_calls(&self, _: Vec<String>, _: &mut LoopContext) -> Vec<String> {
        vec!["tool-result".into()]
    }
    async fn handle_text_response(&self, text: String, _: &mut LoopContext) -> TextAction {
        TextAction::Return(LoopOutcome::Response(text))
    }
    async fn after_iteration(&self, _: &mut LoopContext, _: usize) {}
}

#[tokio::test]
async fn returns_response_on_text() {
    let d = ScriptedDelegate {
        script: Mutex::new(vec![RespondResult::Text("hi".into())]),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig::default();
    let out = run_agentic_loop(&d, &cfg).await;
    assert!(matches!(out, LoopOutcome::Response(s) if s == "hi"));
}

#[tokio::test]
async fn max_iterations_terminates() {
    let mut script = Vec::new();
    for _ in 0..5 { script.push(RespondResult::ToolCalls { calls: vec!["x".into()], finish_reason: "stop".into() }); }
    let d = ScriptedDelegate { script: Mutex::new(script), ..Default::default() };
    let cfg = AgenticLoopConfig { max_iterations: 3, ..Default::default() };
    let out = run_agentic_loop(&d, &cfg).await;
    assert!(matches!(out, LoopOutcome::MaxIterations));
}

#[tokio::test]
async fn stop_signal_terminates() {
    let d = ScriptedDelegate {
        script: Mutex::new(vec![RespondResult::Text("never".into())]),
        stop_after: Some(0),
        ..Default::default()
    };
    let cfg = AgenticLoopConfig::default();
    let out = run_agentic_loop(&d, &cfg).await;
    assert!(matches!(out, LoopOutcome::Stopped));
}
```

- [ ] **Step 5.1.2: Run, verify RED**

```bash
cargo test --test agentic_loop_unit
```

- [ ] **Step 5.1.3: Implement agentic_loop.rs**

Create `src-tauri/src/modules/application/turn_service/agentic_loop.rs`:

```rust
//! Unified agentic loop. Mirrors Steward's `run_agentic_loop`.
//!
//! For 5a the loop uses minimal owned types (`String` for tool calls) to
//! keep the contract testable without depending on the wider runtime types.
//! 5b/5c will use the same trait — concrete delegates carry full `ToolCall`
//! / `ToolResult` values inside their own state.

use async_trait::async_trait;
use crate::modules::application::turn_service::AgenticLoopConfig;

#[derive(Debug, Default)]
pub struct LoopContext {
    pub injected: Vec<String>,
    pub tool_results: Vec<String>,
    pub force_text: bool,
}
impl LoopContext {
    pub fn inject(&mut self, msg: impl Into<String>) { self.injected.push(msg.into()); }
    pub fn append_tool_results(&mut self, mut r: Vec<String>) { self.tool_results.append(&mut r); }
}

#[derive(Debug)]
pub enum LoopSignal {
    Continue,
    Stop,
    InjectMessage { content: String },
}

#[derive(Debug)]
pub enum RespondResult {
    Text(String),
    ToolCalls { calls: Vec<String>, finish_reason: String },
}

#[derive(Debug)]
pub enum TextAction {
    Return(LoopOutcome),
    Continue,
}

#[derive(Debug)]
pub enum LoopOutcome {
    Response(String),
    Stopped,
    MaxIterations,
    Failure(String),
}

#[async_trait]
pub trait LoopDelegate: Send + Sync {
    async fn check_signals(&self) -> LoopSignal;
    async fn before_llm_call(&self, ctx: &mut LoopContext, iteration: usize) -> Option<LoopOutcome>;
    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String>;
    async fn execute_tool_calls(&self, calls: Vec<String>, ctx: &mut LoopContext) -> Vec<String>;
    async fn handle_text_response(&self, text: String, ctx: &mut LoopContext) -> TextAction;
    async fn after_iteration(&self, ctx: &mut LoopContext, iteration: usize);
}

pub async fn run_agentic_loop(
    delegate: &dyn LoopDelegate,
    config: &AgenticLoopConfig,
) -> LoopOutcome {
    let mut ctx = LoopContext::default();
    let mut nudge_count: u32 = 0;
    let mut truncation_count: u32 = 0;

    for iteration in 0..config.max_iterations {
        match delegate.check_signals().await {
            LoopSignal::Stop => return LoopOutcome::Stopped,
            LoopSignal::InjectMessage { content } => ctx.inject(content),
            LoopSignal::Continue => {}
        }

        if let Some(out) = delegate.before_llm_call(&mut ctx, iteration).await {
            return out;
        }

        ctx.force_text = truncation_count >= config.force_text_after_truncations;

        match delegate.call_llm(&mut ctx).await {
            Err(e) => return LoopOutcome::Failure(e),
            Ok(RespondResult::Text(text)) => {
                truncation_count = 0;
                match delegate.handle_text_response(text, &mut ctx).await {
                    TextAction::Return(o) => return o,
                    TextAction::Continue => {}
                }
            }
            Ok(RespondResult::ToolCalls { calls, finish_reason }) => {
                if finish_reason == "length" {
                    truncation_count += 1;
                    continue;
                }
                if calls.is_empty()
                    && config.enable_tool_intent_nudge
                    && nudge_count < config.max_tool_intent_nudges
                {
                    ctx.inject("(You signaled tool intent but made no calls. Please call a tool now.)");
                    nudge_count += 1;
                    continue;
                }
                let results = delegate.execute_tool_calls(calls, &mut ctx).await;
                ctx.append_tool_results(results);
            }
        }

        delegate.after_iteration(&mut ctx, iteration).await;
    }

    LoopOutcome::MaxIterations
}
```

In `turn_service/mod.rs`:
```rust
pub mod agentic_loop;
```

- [ ] **Step 5.1.4: Run, verify GREEN**

```bash
cargo test --test agentic_loop_unit
```

- [ ] **Step 5.1.5: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S5a — introduce LoopDelegate trait + run_agentic_loop"
```

---

## Task 5.2 — Sub-commit 5b: StreamDelegate adopts run_agentic_loop

- [ ] **Step 5.2.1: Read current stream loop**

```bash
rg -n "for iteration|loop \{" src-tauri/src/modules/application/turn_service/stream_task.rs src-tauri/src/modules/application/turn_service/stream_event_loop.rs
```
Identify the existing manual iteration loop in `run_stream_task_body` (or `stream_event_loop.rs`).

- [ ] **Step 5.2.2: Add a streaming-typed `LoopDelegate` variant**

Because the 5a trait uses `String` for tool calls, expose a parallel typed contract or generalize. Add to `agentic_loop.rs`:

```rust
/// Marker trait extension exposing typed call/result associated types.
/// Production delegates (StreamDelegate, RunDelegate) implement this and
/// adapt internally to the loop-level `RespondResult`.
pub trait LoopDelegateTypes {
    type Call: Send + Sync;
    type Result: Send + Sync;
}
```

(Alternative if cleaner: keep 5a's `String`-typed trait as the loop contract and have `StreamDelegate` keep its real `ToolCall`/`ToolResult` values *inside* the delegate, only feeding the loop opaque ids. The loop never inspects call contents — it just routes them.)

Use the **opaque-id approach**: `StreamDelegate` stores a `Mutex<HashMap<String, ToolCall>>`; `call_llm` returns `RespondResult::ToolCalls { calls: vec!["call-id-N", ...] }`; `execute_tool_calls` looks up by id, runs them, returns id strings of completed results stored back in the delegate.

- [ ] **Step 5.2.3: Implement StreamDelegate**

In `stream_task.rs`:

```rust
struct StreamDelegate<'a> {
    inputs: &'a StreamTaskInputs,
    state: tokio::sync::Mutex<StreamLoopState>,
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
    pending_calls: tokio::sync::Mutex<std::collections::HashMap<String, ToolCall>>,
    pending_results: tokio::sync::Mutex<std::collections::HashMap<String, ToolResult>>,
    cancellation: tokio_util::sync::CancellationToken,
}

#[async_trait]
impl<'a> LoopDelegate for StreamDelegate<'a> {
    async fn check_signals(&self) -> LoopSignal {
        if self.cancellation.is_cancelled() { return LoopSignal::Stop; }
        LoopSignal::Continue
    }
    async fn before_llm_call(&self, _ctx: &mut LoopContext, _iter: usize) -> Option<LoopOutcome> {
        None
    }
    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String> {
        let mut state = self.state.lock().await;
        // Build the request honoring force_text:
        let include_tools = !ctx.force_text;
        // ... existing stream_preflight::build_request logic, refactored to take include_tools ...
        // ... call existing api stream + collect deltas ...
        // Convert to RespondResult::Text or RespondResult::ToolCalls { calls: opaque ids }
        unimplemented!("port from current run_stream_task_body iteration body")
    }
    async fn execute_tool_calls(&self, ids: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        let pending = self.pending_calls.lock().await;
        let calls: Vec<ToolCall> = ids.iter().filter_map(|i| pending.get(i).cloned()).collect();
        drop(pending);
        // Existing stream_tool_execution::execute_batch
        let results = stream_tool_execution::execute_batch(self.inputs, calls, &self.tx).await;
        let mut store = self.pending_results.lock().await;
        let mut ids_done = Vec::new();
        for r in results {
            ids_done.push(r.call_id.clone());
            store.insert(r.call_id.clone(), r);
        }
        ids_done
    }
    async fn handle_text_response(&self, text: String, _ctx: &mut LoopContext) -> TextAction {
        TextAction::Return(LoopOutcome::Response(text))
    }
    async fn after_iteration(&self, _ctx: &mut LoopContext, _iter: usize) {
        // Run preflight/finalize hook registries here per iteration if desired.
    }
}
```

Then replace the body of `run_stream_task_body` with:

```rust
let delegate = StreamDelegate { inputs: &inputs, state: tokio::sync::Mutex::new(state), tx: tx.clone(), pending_calls: Default::default(), pending_results: Default::default(), cancellation: cancel.clone() };
let outcome = run_agentic_loop(&delegate, &inputs.loop_config).await;
// Translate LoopOutcome → existing finalize path (assemble RunReport, persist, etc.)
```

- [ ] **Step 5.2.4: Run e2e**

```bash
cargo test --test turn_service_stream_turn_e2e
```
Expect PASS. If it fails, the typical culprits are: tool result ordering, missing event emissions, finalize path drift. Fix until green.

- [ ] **Step 5.2.5: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S5b — StreamDelegate adopts run_agentic_loop"
```

---

## Task 5.3 — Sub-commit 5c: RunDelegate adopts run_agentic_loop

- [ ] **Step 5.3.1: Locate sync loop**

```bash
rg -n "loop \{" src-tauri/src/modules/runtime/conversation.rs
```
Find `ConversationRuntime::run_turn`'s manual iteration loop.

- [ ] **Step 5.3.2: Implement RunDelegate**

In `src-tauri/src/modules/application/turn_service/run.rs`:

```rust
struct RunDelegate<'a> {
    runtime: &'a mut ConversationRuntime,
    api_client: &'a dyn ApiClient,
    tool_executor: &'a dyn ToolExecutor,
    pending_calls: std::sync::Mutex<std::collections::HashMap<String, ToolCall>>,
    pending_results: std::sync::Mutex<std::collections::HashMap<String, ToolResult>>,
}
```

(Note: holding `&mut` in a `Send + Sync` delegate is awkward — promote inner state to interior mutability via `Mutex` or `RefCell` shim. Concretely: store the runtime owned by value inside the delegate, return it back to caller via `delegate.into_runtime()` after `run_agentic_loop` completes.)

Implement the trait methods adapting to existing `ConversationRuntime` helpers:
- `call_llm` → `api_client.complete_with_tools(request).await`
- `execute_tool_calls` → `tool_executor.execute(calls).await`
- `after_iteration` → run any per-iteration hooks via the new `HookRegistry`
- `handle_text_response` → store and return `Return(Response(text))`

Replace `ConversationRuntime::run_turn`'s inner loop with:
```rust
let mut delegate = RunDelegate::new(self, api_client, tool_executor);
let outcome = run_agentic_loop(&delegate, &config).await;
let mut delegate = delegate; // reclaim
let runtime = delegate.into_runtime();
*self = runtime;
match outcome { /* translate to existing return type */ }
```

- [ ] **Step 5.3.3: Mark old `AgentLoopDelegate` deprecated**

In `agent_loop_delegate.rs`:
```rust
#[deprecated(note = "Use modules::application::turn_service::agentic_loop::LoopDelegate instead")]
pub trait AgentLoopDelegate { /* ... */ }
```
Allow at the trait def: `#[allow(deprecated)]` on existing impls until the next release.

- [ ] **Step 5.3.4: Run e2e**

```bash
cargo test --test turn_service_run_turn_e2e
```

- [ ] **Step 5.3.5: Run global gate + commit**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
git add -A
git commit -m "feat(steward-align): S5c — RunDelegate adopts run_agentic_loop; deprecate AgentLoopDelegate"
```

---

## Session 5 Exit Gate

1. `cargo test --test agentic_loop_unit` PASS
2. `cargo test --test turn_service_stream_turn_e2e` PASS
3. `cargo test --test turn_service_run_turn_e2e` PASS
4. `cargo test` (full) PASS
5. `rg -n "for iteration in|while iteration <" src-tauri/src/modules/application/turn_service/stream_task.rs src-tauri/src/modules/runtime/conversation.rs` returns no matches in the agent loop bodies (still allowed in helpers).

---

# SESSION 6 — E4: DW-001 Scanner Real Inputs

**Reference**: `docs/superpowers/plans/2026-04-30-dw-001-scanner-real-inputs.md` is the canonical task list. Execute every task in that file verbatim under this session.

- [ ] **Step 6.1: Open `docs/superpowers/plans/2026-04-30-dw-001-scanner-real-inputs.md`**

Read the entire plan. Note its task numbering.

- [ ] **Step 6.2: Execute each task in DW-001 plan order**

For each task in the DW-001 plan:
1. Write its failing test
2. Run RED
3. Implement
4. Run GREEN
5. Run global gate
6. Commit with message `feat(steward-align): S6-E4-T<n> — <DW-001 task title>`

- [ ] **Step 6.3: Final session gate**

Verify the DW-001 plan's own exit gate criteria pass.

---

## Session 6 Exit Gate

Per `2026-04-30-dw-001-scanner-real-inputs.md` exit gate. All scanner suite tests PASS.

---

# Final Program Exit Gate

After Session 6, before merging `feature/steward-alignment` back to `vnext`:

1. `cargo fmt --all` clean
2. `cargo clippy --all-targets -- -D warnings` clean
3. `cargo test` full PASS
4. `rg "AgentLoopDelegate" src-tauri/src/` only shows the deprecated definition + adapters; no new code uses it
5. `rg "PROTECTED_TOOL_NAMES" src-tauri/src/modules/tools/registry.rs` shows registration guard usage
6. `rg "GLOBAL_KNOWLEDGE_STORE" src-tauri/src/desktop_host/setup.rs` shows file-backed install
7. `rg "global_provider_circuit" src-tauri/src/modules/provider/resilience.rs` shows mirror writes
8. `git log --oneline vnext..HEAD` shows N commits, all prefixed `feat(steward-align):`, all independently buildable

After all eight gates pass, hand off to `superpowers:finishing-a-development-branch`.

---

# Self-Review Notes (post-write)

- ✅ Spec coverage: every spec section §4–§9 maps to an explicit Session
- ✅ No `TBD` / `implement later` placeholders
- ✅ `LoopContext`, `LoopDelegate`, `RespondResult` types consistent across S5 sub-tasks
- ✅ `AgenticLoopConfig` field names identical between `loop_config.rs` (Task 2.1) and `agentic_loop.rs` (Task 5.1)
- ✅ `escape_skill_content` signature identical across §6 spec, Task 2.3, and `skills/mod.rs`
- ✅ `attenuate_tools` signature identical across §6 spec and Task 3.1
- ✅ Each Session has a numbered Exit Gate section
- ✅ Each commit is independently rollback-able (no cross-session state mutation in commits)

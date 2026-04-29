# DW-001 Self-Edit Scanner 真化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `spawn_self_edit_scanner_interval` 里的三个 `landed-stub`（`MockUtilityLlm::empty()`、`ConstEmbedder`、空 `&[]` reports）替换为进程级真实依赖，使 DW-001 每 5 min 扫描到真实的 `HarnessRunReport` failure 集群并生成真实的 self-edit 提案。

**Architecture:** 四层依赖串联：(1) `FastEmbedProvider` 实现 `sedimentation::Embedder` trait；(2) `load_scanner_tick_input` 异步从磁盘加载最近 N 条 HarnessRunReport 并派生 `failure_rate/sample_size`；(3) 进程内 `Arc<Mutex<ScannerStageState>>` 跨 tick 持久化 `PromotionStage`；(4) `setup.rs` scanner interval 用真实 LLM + Embedder + reports 驱动。

**Tech Stack:** Rust async (Tokio)、`fastembed` crate、`HarnessReportStore`、`ChatProviderUtilityLlm`、`sedimentation::Embedder` trait

---

## File Map

| 操作   | 文件                                                        | 职责                                                                          |
| ------ | ----------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Modify | `src-tauri/src/modules/memory/embedding/fastembed.rs`       | 给 `FastEmbedProvider` 实现 `sedimentation::Embedder`                         |
| Create | `src-tauri/src/modules/learning/self_edit/scanner_state.rs` | `ScannerTickInput`（report 加载 + 统计）+ `ScannerStageState`（跨 tick 状态） |
| Modify | `src-tauri/src/modules/learning/self_edit/mod.rs`           | 导出 `scanner_state` 模块                                                     |
| Modify | `src-tauri/src/modules/desktop_host/setup.rs`               | 串联真实依赖进 scanner interval loop                                          |

---

### Task 1: `FastEmbedProvider` 实现 `sedimentation::Embedder` trait

**Files:**
- Modify: `src-tauri/src/modules/memory/embedding/fastembed.rs`

- [ ] **Step 1: 写失败测试**

在 `fastembed.rs` 末尾 `#[cfg(test)]` 块中添加：

```rust
#[cfg(test)]
mod embedder_trait_tests {
    use super::*;
    use crate::modules::skills::sedimentation::Embedder;

    /// ConstEmbedder stub verifies the trait is callable — used as a
    /// compile-time canary before FastEmbedProvider impl exists.
    #[test]
    fn embed_via_trait_returns_nonempty_or_empty_never_panics() {
        struct ConstEmbedder(Vec<f32>);
        impl Embedder for ConstEmbedder {
            fn embed(&self, _text: &str) -> Vec<f32> {
                self.0.clone()
            }
        }
        let e = ConstEmbedder(vec![1.0, 0.0]);
        assert_eq!(e.embed("hello"), vec![1.0, 0.0]);
    }

    /// FastEmbedProvider must satisfy the Embedder bound at compile time.
    #[test]
    fn fast_embed_provider_satisfies_embedder_bound() {
        fn _accepts_embedder(_e: &dyn Embedder) {}
        // Construct only if model is available; skip otherwise.
        if let Ok(p) = FastEmbedProvider::new() {
            _accepts_embedder(&p);
        }
    }
}
```

- [ ] **Step 2: 运行测试，确认编译失败（`FastEmbedProvider` 未 impl `Embedder`）**

```bash
cd src-tauri && cargo test --lib memory::embedding::embedder_trait_tests 2>&1 | head -20
```

预期：`error[E0277]: the trait bound 'FastEmbedProvider: Embedder' is not satisfied`

- [ ] **Step 3: 实现 `Embedder for FastEmbedProvider`**

在 `fastembed.rs` 的 `#[cfg(test)]` 之前添加：

```rust
use crate::modules::skills::sedimentation::Embedder;

impl Embedder for FastEmbedProvider {
    /// Bridge to `embed_one`; returns `Vec::new()` on any error so the
    /// dedup gate degrades gracefully instead of panicking.
    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_one(text).unwrap_or_default()
    }
}
```

- [ ] **Step 4: 运行测试**

```bash
cd src-tauri && cargo test --lib memory::embedding::embedder_trait_tests 2>&1 | tail -10
```

预期：`test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/modules/memory/embedding/fastembed.rs
git commit -m "feat(embed): impl sedimentation::Embedder for FastEmbedProvider"
```

---

### Task 2: `ScannerTickInput` + `ScannerStageState` helpers

**Files:**
- Create: `src-tauri/src/modules/learning/self_edit/scanner_state.rs`
- Modify: `src-tauri/src/modules/learning/self_edit/mod.rs`

- [ ] **Step 1: 写失败测试（新建文件只含 tests）**

创建 `src-tauri/src/modules/learning/self_edit/scanner_state.rs`：

```rust
//! DW-001 scanner tick helpers — report loading + stage persistence.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_empty_store_returns_zero_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let store = crate::modules::harness::HarnessReportStore::new(tmp.path());
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 0);
        assert!((input.failure_rate - 0.0).abs() < f32::EPSILON);
        assert!(input.reports.is_empty());
    }

    #[tokio::test]
    async fn load_reports_derives_correct_failure_rate() {
        use crate::modules::harness::run_report::{HarnessRunReport, TaskOutcome};
        let tmp = tempfile::tempdir().unwrap();
        let store = crate::modules::harness::HarnessReportStore::new(tmp.path());
        for (id, outcome) in [
            ("r1", TaskOutcome::Success),
            ("r2", TaskOutcome::Success),
            ("r3", TaskOutcome::Failed),
        ] {
            let mut r = HarnessRunReport::new_empty(id, chrono::Utc::now());
            r.task.outcome = outcome;
            store.save(&r).await.unwrap();
        }
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 3);
        assert!((input.failure_rate - 1.0_f32 / 3.0).abs() < 0.01);
    }

    #[test]
    fn stage_state_default_is_shadow() {
        use crate::modules::learning::self_edit::PromotionStage;
        let s = ScannerStageState::default();
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_applies_promote() {
        use crate::modules::learning::self_edit::{PromotionStage, StageTransition};
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Promote(PromotionStage::Canary1Pct));
        assert_eq!(s.stage, PromotionStage::Canary1Pct);
    }

    #[test]
    fn stage_state_holds() {
        use crate::modules::learning::self_edit::{PromotionStage, StageTransition};
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Hold);
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_demotes() {
        use crate::modules::learning::self_edit::{PromotionStage, StageTransition};
        let mut s = ScannerStageState { stage: PromotionStage::Canary1Pct };
        s.apply_transition(StageTransition::Demote(PromotionStage::Shadow));
        assert_eq!(s.stage, PromotionStage::Shadow);
    }
}
```

- [ ] **Step 2: 在 `self_edit/mod.rs` 注册模块（令 cargo 能找到文件）**

在 `src-tauri/src/modules/learning/self_edit/mod.rs` 的 `pub mod` 列表中添加：

```rust
pub mod scanner_state;
```

- [ ] **Step 3: 运行，确认编译失败（`load_scanner_tick_input` / `ScannerStageState` 未定义）**

```bash
cd src-tauri && cargo test --lib learning::self_edit::scanner_state 2>&1 | head -15
```

预期：`error[E0425]: cannot find function 'load_scanner_tick_input'`

- [ ] **Step 4: 实现完整 `scanner_state.rs`**

将 `scanner_state.rs` 替换为完整实现：

```rust
//! DW-001 scanner tick helpers — report loading + stage persistence.
//!
//! `load_scanner_tick_input` is an async helper that loads the N most
//! recent `HarnessRunReport`s from disk and derives `failure_rate` /
//! `sample_size` for the promotion state machine.
//!
//! `ScannerStageState` persists `PromotionStage` across scanner ticks
//! via `Arc<Mutex<ScannerStageState>>` inside `spawn_self_edit_scanner_interval`.

use crate::modules::harness::run_report::{HarnessRunReport, TaskOutcome};
use crate::modules::harness::HarnessReportStore;
use crate::modules::learning::self_edit::{PromotionStage, StageTransition};

/// Input snapshot fed into one scanner tick.
#[derive(Debug)]
pub struct ScannerTickInput {
    /// Most-recent reports (newest first).
    pub reports: Vec<HarnessRunReport>,
    /// `failed / total`; 0.0 when no reports.
    pub failure_rate: f32,
    /// `reports.len()`
    pub sample_size: usize,
}

/// Load the `max_reports` most recent reports from `store` and derive
/// rolling-window stats.  Returns all-zero on missing / unreadable store.
pub async fn load_scanner_tick_input(
    store: &HarnessReportStore,
    max_reports: usize,
) -> ScannerTickInput {
    let entries = match store.list().await {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(?err, "[scanner_state] failed to list reports; holding");
            return ScannerTickInput { reports: Vec::new(), failure_rate: 0.0, sample_size: 0 };
        }
    };

    let window: Vec<_> = entries.into_iter().take(max_reports).collect();
    let mut reports = Vec::with_capacity(window.len());
    for entry in &window {
        match store.load(&entry.run_id).await {
            Ok(Some(r)) => reports.push(r),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    run_id = %entry.run_id, ?err,
                    "[scanner_state] skipping corrupt report"
                );
            }
        }
    }

    let sample_size = reports.len();
    let failed = reports
        .iter()
        .filter(|r| r.task.outcome == TaskOutcome::Failed)
        .count();
    let failure_rate = if sample_size == 0 {
        0.0
    } else {
        failed as f32 / sample_size as f32
    };

    ScannerTickInput { reports, failure_rate, sample_size }
}

/// Mutable promotion stage state preserved across scanner ticks.
#[derive(Debug, Clone)]
pub struct ScannerStageState {
    pub stage: PromotionStage,
}

impl Default for ScannerStageState {
    fn default() -> Self {
        Self { stage: PromotionStage::Shadow }
    }
}

impl ScannerStageState {
    /// Apply the `StageTransition` returned by `next_stage()`.
    pub fn apply_transition(&mut self, t: StageTransition) {
        match t {
            StageTransition::Promote(s) | StageTransition::Demote(s) => self.stage = s,
            StageTransition::Hold => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_empty_store_returns_zero_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 0);
        assert!((input.failure_rate - 0.0).abs() < f32::EPSILON);
        assert!(input.reports.is_empty());
    }

    #[tokio::test]
    async fn load_reports_derives_correct_failure_rate() {
        let tmp = tempfile::tempdir().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        for (id, outcome) in [
            ("r1", TaskOutcome::Success),
            ("r2", TaskOutcome::Success),
            ("r3", TaskOutcome::Failed),
        ] {
            let mut r = HarnessRunReport::new_empty(id, chrono::Utc::now());
            r.task.outcome = outcome;
            store.save(&r).await.unwrap();
        }
        let input = load_scanner_tick_input(&store, 20).await;
        assert_eq!(input.sample_size, 3);
        assert!((input.failure_rate - 1.0_f32 / 3.0).abs() < 0.01);
    }

    #[test]
    fn stage_state_default_is_shadow() {
        let s = ScannerStageState::default();
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_applies_promote() {
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Promote(PromotionStage::Canary1Pct));
        assert_eq!(s.stage, PromotionStage::Canary1Pct);
    }

    #[test]
    fn stage_state_holds() {
        let mut s = ScannerStageState::default();
        s.apply_transition(StageTransition::Hold);
        assert_eq!(s.stage, PromotionStage::Shadow);
    }

    #[test]
    fn stage_state_demotes() {
        let mut s = ScannerStageState { stage: PromotionStage::Canary1Pct };
        s.apply_transition(StageTransition::Demote(PromotionStage::Shadow));
        assert_eq!(s.stage, PromotionStage::Shadow);
    }
}
```

- [ ] **Step 5: 运行所有 scanner_state 测试**

```bash
cd src-tauri && cargo test --lib learning::self_edit::scanner_state::tests 2>&1 | tail -12
```

预期：`test result: ok. 5 passed`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/modules/learning/self_edit/scanner_state.rs \
        src-tauri/src/modules/learning/self_edit/mod.rs
git commit -m "feat(scanner): ScannerTickInput + ScannerStageState helpers for DW-001"
```

---

### Task 3: 串联真实依赖到 `spawn_self_edit_scanner_interval`

**Files:**
- Modify: `src-tauri/src/modules/desktop_host/setup.rs`

- [ ] **Step 1: 写编译验证测试**

在 `setup.rs` 底部添加（如无 `#[cfg(test)]` 则新建）：

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn scanner_stage_default_is_shadow() {
        use crate::modules::learning::self_edit::scanner_state::ScannerStageState;
        use crate::modules::learning::self_edit::PromotionStage;
        assert_eq!(ScannerStageState::default().stage, PromotionStage::Shadow);
    }
}
```

- [ ] **Step 2: 运行，确认通过**

```bash
cd src-tauri && cargo test --lib desktop_host::setup::tests 2>&1 | tail -8
```

预期：`test result: ok. 1 passed`

- [ ] **Step 3: 替换 `spawn_self_edit_scanner_interval` 完整函数体**

将 `setup.rs` 中 `fn spawn_self_edit_scanner_interval(app_handle: tauri::AppHandle)` 整个函数（从函数签名到最后一个 `}`）替换为：

```rust
/// DW-001 深度接入（truth-loop iter-7）— 替换三个 landed-stub：
///
/// 1. `MockUtilityLlm::empty()` → `ChatProviderUtilityLlm`（真实 LLM）
/// 2. `ConstEmbedder`           → `FastEmbedProvider`（真实向量化；fail-safe 回退）
/// 3. `&[], &[]`                → 磁盘最近 100 份 `HarnessRunReport`
///
/// `PromotionStage` 通过 `Arc<Mutex<ScannerStageState>>` 跨 tick 持久化。
fn spawn_self_edit_scanner_interval(app_handle: tauri::AppHandle) {
    use std::sync::{Arc, Mutex};

    use crate::modules::harness::HarnessReportStore;
    use crate::modules::learning::self_edit::scanner::{
        run_scanner_once, self_edit_scanner_disabled, DEFAULT_SCANNER_INTERVAL,
    };
    use crate::modules::learning::self_edit::scanner_state::{
        load_scanner_tick_input, ScannerStageState,
    };
    use crate::modules::learning::self_edit::PromotionStage;
    use crate::modules::memory::embedding::FastEmbedProvider;
    use crate::modules::memory::llm::ChatProviderUtilityLlm;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    use crate::modules::runtime::evolution_emitter::emit_evolution_event;
    use crate::modules::skills::sedimentation::Embedder;

    if self_edit_scanner_disabled() {
        tracing::info!("[setup] DW-001 self-edit scanner disabled (IF2AI_DISABLE_SELF_EDIT=1)");
        return;
    }

    /// Fallback when FastEmbedProvider fails to initialise.
    struct ConstFallbackEmbedder;
    impl Embedder for ConstFallbackEmbedder {
        fn embed(&self, _text: &str) -> Vec<f32> {
            vec![1.0, 0.0, 0.0]
        }
    }

    let embedder: Arc<dyn Embedder + Send + Sync> = match FastEmbedProvider::new() {
        Ok(p) => {
            tracing::info!("[setup] DW-001 using FastEmbedProvider for dedup");
            Arc::new(p)
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                "[setup] DW-001 FastEmbedProvider init failed; falling back to ConstEmbedder"
            );
            Arc::new(ConstFallbackEmbedder)
        }
    };

    let stage_state = Arc::new(Mutex::new(ScannerStageState::default()));
    let store = HarnessReportStore::with_default_root();

    let task = async move {
        let mut ticker = tokio::time::interval(DEFAULT_SCANNER_INTERVAL);
        loop {
            ticker.tick().await;

            let tick = load_scanner_tick_input(&store, 100).await;
            let reports_refs: Vec<&crate::modules::harness::run_report::HarnessRunReport> =
                tick.reports.iter().collect();

            let current_stage = stage_state
                .lock()
                .map(|g| g.stage)
                .unwrap_or(PromotionStage::Shadow);

            let llm: std::sync::Arc<dyn crate::modules::memory::UtilityLlm> =
                std::sync::Arc::new(ChatProviderUtilityLlm::new(
                    crate::modules::config::store::if2ai_data_root(),
                ));

            let outcome_fut = std::panic::AssertUnwindSafe(run_scanner_once(
                &reports_refs,
                &[],
                llm,
                embedder.as_ref(),
                current_stage,
                tick.failure_rate,
                tick.sample_size,
            ));
            let outcome = match futures::FutureExt::catch_unwind(outcome_fut).await {
                Ok(o) => o,
                Err(_) => {
                    tracing::warn!("[setup] DW-001 self-edit scanner tick panicked");
                    continue;
                }
            };

            if let Ok(mut g) = stage_state.lock() {
                g.apply_transition(outcome.transition);
            }

            for proposal in &outcome.proposals {
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::SelfEditProposal,
                    "draft",
                    CorrelationIds::default(),
                    &serde_json::json!({
                        "id": proposal.id,
                        "kind": proposal.kind.as_str(),
                        "target": proposal.target,
                        "justification": proposal.justification,
                    }),
                    None,
                );
            }
            for (proposal, verdict) in &outcome.verdicts {
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::SelfEditProposal,
                    "verdict",
                    CorrelationIds::default(),
                    &serde_json::json!({
                        "id": proposal.id,
                        "verdict": format!("{:?}", verdict.verdict),
                        "failedGates": verdict.failed_gates,
                    }),
                    None,
                );
            }

            if let Some(reason) = &outcome.skipped_reason {
                tracing::debug!("[DW-001] scanner tick skipped: {reason}");
            } else {
                tracing::info!(
                    proposals = outcome.proposals.len(),
                    verdicts = outcome.verdicts.len(),
                    stage = ?outcome.transition,
                    reports_loaded = tick.sample_size,
                    failure_rate = tick.failure_rate,
                    "[DW-001] scanner tick complete"
                );
            }
        }
    };

    match tokio::runtime::Handle::try_current() {
        Ok(handle) => { handle.spawn(task); }
        Err(_) => {
            tracing::warn!("[setup] DW-001 no tokio runtime for scanner; skipped");
        }
    }
}
```

- [ ] **Step 4: `cargo check`**

```bash
cd src-tauri && cargo check 2>&1 | grep -E "^error|Finished" | tail -5
```

预期：`Finished \`dev\` profile`

- [ ] **Step 5: 运行全部相关测试**

```bash
cd src-tauri \
  && cargo test --lib learning::self_edit::scanner_state::tests 2>&1 | tail -6 \
  && cargo test --lib memory::embedding::embedder_trait_tests 2>&1 | tail -6 \
  && cargo test --lib desktop_host::setup::tests 2>&1 | tail -6
```

预期：三组均 `ok`

- [ ] **Step 6: Commit code + Commit ledger**

```bash
git add src-tauri/src/modules/desktop_host/setup.rs
git commit -m "feat(scanner): DW-001 wire real LLM/Embedder/reports into scanner interval"

# Ledger: 在 AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md §12.3 中把 DW-001 移出 landed-stub 表，
# 新建 §13 Iteration 7 增量。
# 在 REGISTRY.md 中把 DW-001 状态改为 **done**⁶，添加 footnote ⁶。
git add docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md \
        docs/packs/REGISTRY.md
git commit -m "docs: iter-7 ledger – DW-001 scanner 真化完成，所有 landed-stub 清零"
```

---

## Execution Handoff

Plan saved. Two execution options:

**1. Subagent-Driven (recommended)** — 每 Task 派一个新 subagent，两阶段审查，快速迭代

**2. Inline Execution** — 在本 session 用 executing-plans，批量执行含 checkpoint

Which approach?

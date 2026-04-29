# WU-005: Self-Edit 后台扫描器（AE-001~003 Wire-up）

## Status
- State: active

## Goal
新建 `learning/self_edit/scanner.rs`：一个 tokio 周期性任务（默认间隔 5 分钟），在后台依次调用 `cluster_failures` → `generate_proposals` → `verify_proposals`（AE-002 gate）→ `next_stage`（AE-003 状态机），将通过 gate 的提案持久化，并通过 `emit_evolution_event` 广播 `SelfEditProposal` / `VerificationDecision`。在 `desktop_host/setup.rs` 处 spawn 此任务，支持 `IF2AI_DISABLE_SELF_EDIT=1` 完全跳过。

## Spec (verifiable)
- scanner 在 disable 标志下不 spawn（不调用 cluster_failures）→ `tests::self_edit_scanner::scanner_not_spawned_when_disabled`
- `cluster_failures` 返回空集合时 scanner 不调用 generate_proposals → `tests::self_edit_scanner::empty_cluster_skips_generation`
- generate_proposals 成功后广播 `SelfEditProposal` envelope → `tests::self_edit_scanner::proposal_emits_self_edit_event`
- verify_proposals gate REJECT 后广播 `VerificationDecision`（verdict=rejected）→ `tests::self_edit_scanner::rejected_proposal_emits_verification_decision`
- scanner panic 不影响 daemon / setup 主流程 → `tests::self_edit_scanner::scanner_panic_does_not_crash_setup`

## Files (scope — write list)
- `src-tauri/src/modules/learning/self_edit/scanner.rs`    (new — 周期性扫描任务)
- `src-tauri/src/modules/learning/self_edit/mod.rs`        (modify — pub mod scanner)
- `src-tauri/src/modules/desktop_host/setup.rs`            (modify — spawn scanner task)
- `src-tauri/tests/self_edit_scanner.rs`                   (new — 5 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/learning/failure_clustering.rs`           (cluster_failures)
- `src-tauri/src/modules/learning/self_edit/proposal.rs`           (generate_proposals)
- `src-tauri/src/modules/learning/self_edit/verification.rs`       (verify_proposals / VerificationVerdict)
- `src-tauri/src/modules/learning/self_edit/promotion.rs`          (next_stage / PromotionStage)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`             (emit_evolution_event — WU-001)

## Contract (review must check)
- scanner 必须在独立 `tokio::spawn` task 内运行，不阻塞 setup.rs（I7）
- `IF2AI_DISABLE_SELF_EDIT=1` 完全跳过 spawn（不仅跳过执行）
- 任意步骤 panic / Err 只记 tracing::warn！不 propagate
- 不改 AE-001~003 任何 pub API 签名（I1）

## Out of Scope
- ❌ 不实现 proposal 持久化存储（使用现有 StrategyRegistryStore 或内存 Vec）
- ❌ 不实现 promotion 回写 skill（仅状态机 next_stage，不做文件写入）
- ❌ 不暴露新 IPC 命令
- ❌ 不实现 UI 展示

## Depends on
- WU-001（emit_evolution_event）
- FEAT-AE-001/002/003（实现 done）

## Verify
- `./scripts/pack run WU-005`
- `cargo test --test self_edit_scanner`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done

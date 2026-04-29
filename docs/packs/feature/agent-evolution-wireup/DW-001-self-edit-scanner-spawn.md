# DW-001: Self-Edit Scanner Spawn（setup.rs → tokio interval task）

## Status
- State: active

## Goal
在 `desktop_host/setup.rs::register_evolution_probes_for_app` 内 spawn 一个 tokio interval task，每 60 s 调用 `run_scanner_once(reports, history, llm, embedder, stage, failure_rate, sample_size)`；成功时广播 `SelfEditProposal` + `VerificationDecision` evolution events；任何失败只 `tracing::warn` 不中断进程。

## Spec (verifiable)
- interval task 启动后第一次 tick 调用 `run_scanner_once` → `tests::self_edit_scanner::scanner_spawned_and_called_on_tick`
- scanner 返回 `ScannerOutcome::Proposals(drafts)` 时广播 `SelfEditProposal` envelope → `tests::self_edit_scanner::scanner_outcome_emits_proposal_event`
- `run_scanner_once` panic 不影响 daemon 进程继续运行 → `tests::self_edit_scanner::scanner_panic_does_not_kill_daemon`
- `IF2AI_DISABLE_SELF_EDIT=1` 时 interval task 不 spawn → `tests::self_edit_scanner::env_flag_skips_scanner_spawn`
- scanner 失败时记录 `tracing::warn` 而非 `tracing::error` → `tests::self_edit_scanner::scanner_failure_logs_warn`

## Files (scope — write list)
- `src-tauri/src/modules/desktop_host/setup.rs`          (modify — spawn scanner interval task)
- `src-tauri/tests/self_edit_scanner.rs`                 (new — 5 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/learning/self_edit/scanner.rs`  (run_scanner_once 签名 + ScannerOutcome)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`   (emit_evolution_event — WU-001)
- `src-tauri/src/modules/runtime/daemon/mod.rs`          (register_evolution_probes_for_app stub)

## Contract (review must check)
- 不改 `register_evolution_probes_for_app` 对外签名（I1）
- 不新增 IPC 命令名；仅广播现有 `runtime_event` 频道（I3）
- `IF2AI_DISABLE_SELF_EDIT=1` 必须完全阻止 spawn（env::var 检查在 spawn 前）
- failure isolation：`tokio::spawn` 内任何 Err/panic 均 catch，不传播（I7 clippy pass）

## Out of Scope
- ❌ 不修改 `run_scanner_once` 内部算法（scanner.rs 只读）
- ❌ 不修改 promotion 状态机（FEAT-AE-003）
- ❌ 不实现 UI 展示组件
- ❌ 不改变已有 browser probe 的注册逻辑（WU-002 done）

## Depends on
- WU-001（emit_evolution_event）
- WU-005（self-edit background scanner helper functions done）

## Verify
- `./scripts/pack run DW-001`
- `cargo test --test self_edit_scanner`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done

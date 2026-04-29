# DW-005: Browser Click Strategy Wire（runtime.rs → decide_browser_click_strategy 生产接入）

## Status
- State: active

## Goal
在 `smart_browser/runtime.rs` 的 Click 命令派发路径内，调用已实现的 `decide_browser_click_strategy(target, screenshot, selectors)` 决定交互方式（`CoordinateClick` / `LabelReference` / `CssSelector`），将决策写入 `tracing::debug` 并广播 `BrowserHealth` evolution event；任何失败降级到原有 CssSelector 路径，记 `tracing::warn`。

## Spec (verifiable)
- Click 命令路径调用 `decide_browser_click_strategy` 并取回 `InteractionDecision` → `tests::browser_refinement::click_strategy_called_in_dispatch`
- 返回 `CoordinateClick` 时 tracing span 带 `strategy=coordinate` 字段 → `tests::browser_refinement::coordinate_strategy_traced`
- strategy 决策后广播 `BrowserHealth` envelope（family="click_strategy"）→ `tests::browser_refinement::click_strategy_emits_browser_health_event`
- `decide_browser_click_strategy` 返回 `Err` 时降级 CssSelector，不 panic → `tests::browser_refinement::strategy_error_falls_back_to_css`
- `IF2AI_DISABLE_BROWSER_STRATEGY=1` 时跳过策略决策，直接使用 CssSelector → `tests::browser_refinement::env_flag_skips_strategy`

## Files (scope — write list)
- `src-tauri/src/modules/smart_browser/runtime.rs`     (modify — Click 派发路径接入 strategy)
- `src-tauri/tests/browser_refinement.rs`              (modify — 补 5 个 strategy tests)

## Reads (read-only)
- `src-tauri/src/modules/smart_browser/coordinate_strategy.rs`  (decide_browser_click_strategy + InteractionDecision)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`          (emit_evolution_event — WU-001)
- `src-tauri/src/modules/smart_browser/session_health.rs`       (BrowserHealthPayload 参考字段)

## Contract (review must check)
- 不改 `execute_browser_use_mcp_action` 对外签名（I1）
- 不改任何 IPC 事件名（I3）
- failure isolation：strategy Err → 降级 CssSelector（原有路径），不抛出
- `IF2AI_DISABLE_BROWSER_STRATEGY=1` 覆盖检查在 strategy 调用前

## Out of Scope
- ❌ 不修改 `decide_browser_click_strategy` 内部降级链逻辑（coordinate_strategy.rs 只读）
- ❌ 不修改 content simplifier 接入（WU-006 已处理）
- ❌ 不改 browser session health check 周期（FEAT-SH-003 / WU-002 已处理）
- ❌ 不实现 UI 展示组件

## Depends on
- WU-001（emit_evolution_event）
- WU-006（coordinate_strategy.rs helper done）

## Verify
- `./scripts/pack run DW-005`
- `cargo test --test browser_refinement`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done

# WU-006: Browser 工具简化 + 坐标策略（BR-001/002 Wire-up）

## Status
- State: active

## Goal
在 `smart_browser/runtime.rs::execute_browser_use_mcp_action` 成功返回后，调用 `adaptive_simplify(raw_html, available_tokens)` 将原始 HTML 结果压缩到 token 预算内，广播 `ContentSimplified` event。对 Click 类命令在派发前调用 `decide_interaction(context)` 选取最优坐标策略（CoordinateClick → LabelReference → CssSelector 降级链），结果记录在 tracing，广播 `BrowserHealth` event。两处均有独立失败降级：失败则原始结果透传，不中断浏览器工具链。

## Spec (verifiable)
- `execute_browser_use_mcp_action` 返回 HTML 时被 `adaptive_simplify` 处理 → `tests::browser_wireup::html_result_is_simplified`
- `adaptive_simplify` 失败时原始内容透传 → `tests::browser_wireup::simplify_failure_falls_back`
- Click 命令派发前 `decide_interaction` 被调用 → `tests::browser_wireup::click_calls_decide_interaction`
- 简化成功后广播 `ContentSimplified` envelope → `tests::browser_wireup::simplify_emits_content_simplified_event`
- `IF2AI_DISABLE_BROWSER_SIMPLIFY=1` 时跳过 adaptive_simplify → `tests::browser_wireup::env_flag_disables_simplify`

## Files (scope — write list)
- `src-tauri/src/modules/smart_browser/runtime.rs`    (modify — 调用 adaptive_simplify + decide_interaction)
- `src-tauri/tests/browser_wireup.rs`                 (new — 5 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/smart_browser/content_simplifier.rs`     (adaptive_simplify / SimplifiedContent)
- `src-tauri/src/modules/smart_browser/coordinate_strategy.rs`    (decide_interaction / InteractionStrategy)
- `src-tauri/src/modules/runtime/budget.rs`                       (token budget accessor)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`            (emit_evolution_event — WU-001)

## Contract (review must check)
- 不改 `execute_browser_use_mcp_action` 对外签名（I1）
- `adaptive_simplify` 只在返回 HTML 类型结果时调用（非 JSON/错误不处理）
- `decide_interaction` 调用纯函数，不持有状态，失败静默降级
- `IF2AI_DISABLE_BROWSER_SIMPLIFY=1` 可完全跳过 adaptive_simplify（不影响 decide_interaction）

## Out of Scope
- ❌ 不修改 BR-001/002 内部算法
- ❌ 不改浏览器会话生命周期（SH-003 作用域）
- ❌ 不实现 BR-003 alias 重定向（由 WU-007 完成）
- ❌ 不实现 UI 展示

## Depends on
- WU-001（emit_evolution_event）
- FEAT-BR-001/002（实现 done）

## Verify
- `./scripts/pack run WU-006`
- `cargo test --test browser_wireup`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done

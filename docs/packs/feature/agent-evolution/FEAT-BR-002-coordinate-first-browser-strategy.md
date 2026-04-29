# FEAT-BR-002: 坐标优先浏览器策略

## Status
- State: active

## Goal
在 `smart_browser/coordinate_strategy.rs` 实现纯函数交互策略降级链：CoordinateClick → LabelReference →
CssSelector。`decide_interaction` 通过闭包注入坐标点击函数，不依赖 BrowserSession，保证测试零浏览器开销。
`verify_click_effect` 用字节比较 + 简单 dHash 距离验证点击效果。

## Spec (verifiable)
- screenshot 含目标 element（identified_elements 非空，label 匹配）→ 返回 CoordinateClick 策略 → `tests::browser_refinement::test_coordinate_click_preferred_with_screenshot`
- 无 screenshot 但 available_selectors 非空 → fallback 到 CssSelector → `tests::browser_refinement::test_css_selector_fallback_no_screenshot`
- fallback_chain 顺序为 [CoordinateClick, LabelReference, CssSelector]（按优先级） → `tests::browser_refinement::test_fallback_chain_order`
- 空目标 + 无 screenshot + 无 selector → fallback_chain 至少含一项，不 panic → `tests::browser_refinement::test_empty_input_non_empty_fallback`
- verify_click_effect(同一字节序列, 同一字节序列) → changed = false, similarity = 1.0 → `tests::browser_refinement::test_verify_click_effect_identical`

## Files (scope — write list)
- src-tauri/src/modules/smart_browser/coordinate_strategy.rs  (new)
- src-tauri/src/modules/smart_browser/mod.rs                  (modify — add `pub mod coordinate_strategy;`)
- src-tauri/tests/browser_refinement.rs                       (modify — add 5 BR-002 tests)

## Reads (read-only)
- src-tauri/src/modules/smart_browser/contract.rs             (SmartBrowserCommandKind variants)
- src-tauri/src/modules/smart_browser/session_health.rs       (SessionHeartbeat — SH-003)

## Contract (review must check)
- 不新增 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- decide_interaction 签名为纯函数（无 &mut self，无 async）
- 不调用 CDP Input.dispatchMouseEvent（执行留 wire-up Pack）

## Out of Scope
- ❌ 不实际调用 CDP Input.dispatchMouseEvent
- ❌ 不改 BrowserSession（browser/session.rs）
- ❌ 不接入 runtime.rs 命令执行链
- ❌ 不接入 vision / multimodal model
- ❌ 不改前端 contracts.ts

## Depends on
- FEAT-SH-003 (SessionHeartbeat / BrowserSessionLivenessCheck)

## Verify
- cargo build --package if2ai-tauri
- cargo test --test browser_refinement -- test_coordinate_click_preferred_with_screenshot test_css_selector_fallback_no_screenshot test_fallback_chain_order test_empty_input_non_empty_fallback test_verify_click_effect_identical
- cargo clippy --package if2ai-tauri -D warnings
- ./scripts/pack run FEAT-BR-002

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done

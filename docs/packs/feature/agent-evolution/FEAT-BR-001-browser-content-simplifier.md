# FEAT-BR-001: 浏览器内容简化器

## Status
- State: active

## Goal
在 `smart_browser/content_simplifier.rs` 实现纯函数 HTML 简化器：过滤 script/style/nav/aside/footer，保留
main/article/form/input；35 000 字符硬限制；`adaptive_simplify` 根据剩余 token 预算动态推算字符上限并调用
simplify_html。全部为无副作用纯函数，不依赖任何 chromium 实例。

## Spec (verifiable)
- filter_tags 含 script/style 的输出中不含 `<script` / `<style` → `tests::browser_refinement::test_filter_script_style_removed`
- preserve_tags 含 main/form 的输出中保留对应标签 → `tests::browser_refinement::test_preserve_main_and_form`
- 超过 35 000 chars 的原始 HTML 经 simplify_html 后 simplified_chars ≤ 35 000 → `tests::browser_refinement::test_35k_hard_limit`
- 空字符串输入不 panic，返回空 SimplifiedContent（compression_ratio = 1.0）→ `tests::browser_refinement::test_empty_html_no_panic`
- adaptive_simplify(raw_html, 2000) 返回 token_estimate ≤ 2000 → `tests::browser_refinement::test_adaptive_simplify_token_budget`

## Files (scope — write list)
- src-tauri/src/modules/smart_browser/content_simplifier.rs   (new)
- src-tauri/src/modules/smart_browser/mod.rs                  (modify — add `pub mod content_simplifier;`)
- src-tauri/tests/browser_refinement.rs                       (new — integration tests for BR-001/002/003)

## Reads (read-only)
- src-tauri/src/modules/runtime/budget.rs                     (estimate_tokens signature)
- src-tauri/Cargo.toml                                        (confirm scraper crate present)

## Contract (review must check)
- 不新增 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency（scraper 已在 Cargo.toml）
- `cargo clippy -D warnings` 通过 (I7)
- SimplifierConfig 实现 Default；DEFAULT_MAX_OUTPUT_CHARS = 35_000 为 pub const
- adaptive_simplify 内不使用 unwrap/expect/todo!/unimplemented!

## Out of Scope
- ❌ 不接入 runtime.rs 的 web_scan / execute_browser_use_mcp_action（wire-up 留后续 Pack）
- ❌ 不调真实 chromium / CDP
- ❌ 不改前端 contracts.ts
- ❌ 不改 browser/session.rs 或 browser/snapshot.rs
- ❌ 不实现 LLM 摘要（文本截断即可）

## Depends on
- FEAT-TE-001 (estimate_tokens)

## Verify
- cargo build --package if2ai-tauri
- cargo test --test browser_refinement -- test_filter_script_style_removed test_preserve_main_and_form test_35k_hard_limit test_empty_html_no_panic test_adaptive_simplify_token_budget
- cargo clippy --package if2ai-tauri -D warnings
- ./scripts/pack run FEAT-BR-001

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done

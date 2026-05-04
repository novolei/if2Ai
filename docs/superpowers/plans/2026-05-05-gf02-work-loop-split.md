# GF-02 — Split `work_loop.rs` into Cohesive Sub-modules

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §4 GF-02 + §7.4 覆盖矩阵
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)
> **预计**：2 周，5 PR
> **依赖**：DR-01 完成（部分调用涉及 `SupervisorOps` 收口；建议 work_loop 拆分启动前 supervisor facade 已稳定，避免 PR 期间 import path 抖动）
> **基线 LOC**：3,232（measured 2026-05-05；roadmap 写 3,226，差异为 commit 噪音，本 plan 锁定 3,232 为基线）

---

## 1. 问题

`src-tauri/src/modules/application/turn_service/work_loop.rs` 是 turn_service 内最大单文件（3,232 LOC）。承担至少 10 类彼此独立的职责：

1. 路由上下文构建（route_context / augment_from_run_log）
2. work-loop 决策路由（route_work_loop / route_work_loop_with_context）
3. Skill 解析与 auto-load（resolve_skill_plan / auto_load_trusted_skill_context / SkillRuntimeMetadata 解析 / DK lookup advisory）
4. Canonical tool pool 构建与 enforce 策略（build_canonical_tool_pool / enforce_tool_definitions_for_loop）
5. Prompt contribution 生成（memory_recall / tool_required / single_shell_command / continuation_context）
6. 意图识别（is_tool_required_work_intent / is_direct_shell_command_intent / is_memory_recall_intent / is_continuation_intent / context_requires_tool_execution / looks_like_direct_answer，含大量中英 needle 列表 ~244 LOC）
7. 助手文本信号（assistant_claims_tool_execution_without_tool / assistant_signals_tool_intent / detect_repetitive_model_output / tool_intent_nudge_message）
8. 文本工具调用解析（normalize_dsml_delimiters / detect_textual_tool_call_markup / extract_textual_tool_calls / DSML & XML invoke 解析）
9. Final report 构建（build_final_run_report / loop_outcome_for / next_steps_for）
10. 测试（mod tests，~1,160 LOC，约占文件 1/3）

后果：任何调整都会触发整个文件 cargo check 重编译；needle 列表混在路由决策中难以独立审计；DSML/XML 解析与 work-loop 调度毫无概念耦合却共享文件；测试已混合 4 个域无法局部 re-run；`pub(super)` 导出 23 个符号被 sibling 按 `super::work_loop::*` 引用，split 后必须保持该路径不变。

## 2. 目标

- 拆为子目录 `work_loop/` 下多个 ≤ 1,000 LOC 子文件；每文件单一概念。
- `work_loop/mod.rs` 作 orchestrator + facade：仅 `mod` 声明 + `pub(super) use` 重新导出现有 23 个符号 + 公共常量；目标 ≤ 400 LOC。
- **公共 API 完全不变**：sibling 仍按 `super::work_loop::route_work_loop(…)` 调用，零 import 修改。
- 测试拆分到 `work_loop/tests.rs`（或按子模块分散），保持 `cargo test --lib turn_service` 全绿不变。
- 每 PR 通过 cargo fmt / clippy -D warnings / 相关 cargo test / npm test / npm run build:web。

## 3. 非目标

- **不修改任何行为**（routing / detection / skill autoload / tool pool 输出必须 byte-for-byte 一致）
- **不引入新 trait / 新公共 contract**（这是机械拆分，不是 redesign）
- 不动 `WorkLoopDecision` / `SkillResolutionPlan` / `FinalRunReport` 三个 contract（住 `runtime/contracts/agent_loop.rs`）
- 不动 sibling 的 import 语句（保持 `super::work_loop::xxx`）
- 不重写 needle 列表（中英意图词目前以硬编码常量 array 存在；优化是后续 P2 议题）
- 不合并 / 修改 GF-05（stream_finalize.rs）或 stream_task.rs 的拆分

## 4. 设计

### 4.1 LOC 簇映射（基于源码行号实测）

| 行段 | LOC | 概念簇 | 目标子文件 |
|------|-----|--------|------------|
| 1-89 | 89 | imports + 常量（AUTO_SKILL_TOOLS / MEMORY_*_TOOLS / READ_ONLY_PLANNING_TOOLS / MUTATING_TOOL_NAMES / 各 reason code 字符串） | `mod.rs`（常量） |
| 90-218 | 129 | WorkLoopRouteContext + route_context_from_messages + augment_route_context_from_run_log | `routing.rs` |
| 220-308 | 89 | route_work_loop / route_work_loop_with_context | `routing.rs` |
| 310-442 | 133 | resolve_skill_plan + spawn_dk_lookup_advisory | `skill_resolution.rs` |
| 444-639 | 196 | excerpt_skill_body + auto_load_trusted_skill_context | `skill_resolution.rs` |
| 641-709 | 69 | SkillRuntimeMetadata + parse_skill_runtime_metadata + parse_frontmatter_list + skill_metadata_prompt_block | `skill_resolution.rs` |
| 711-826 | 116 | CanonicalToolPool + build_canonical_tool_pool + stable_tool_schema_hash + tool_pool_policy_label + enforce_tool_definitions_for_loop | `tool_pool.rs` |
| 828-951 | 124 | requires_*_evidence + memory/tool_required/single_shell/continuation prompt contributions | `prompt_contributions.rs` |
| 953-1003 | 51 | is_incomplete_memory_lookup_response + should_force_final_after_memory_recall + mutation_block_reason | `guards.rs` |
| 1005-1109 | 105 | build_final_run_report | `final_report.rs` |
| 1111-1144 | 34 | skill_candidate + source_family_can_auto_load + push_unique | `skill_resolution.rs` + `mod.rs` 私有 helper |
| 1146-1390 | 245 | looks_like_direct_answer + is_tool_required_work_intent + is_direct_shell_command_intent | `intent_detection.rs` |
| 1392-1432 | 41 | context_requires_tool_execution | `intent_detection.rs` |
| 1434-1528 | 95 | assistant_claims_tool_execution_without_tool + assistant_signals_tool_intent | `assistant_signals.rs` |
| 1530-1744 | 215 | normalize_dsml_delimiters + detect_textual_tool_call_markup + extract_textual_tool_calls + DSML/XML 解析 + helpers | `textual_tool_calls.rs` |
| 1746-1789 | 44 | tool_intent_nudge_message + detect_repetitive_model_output | `assistant_signals.rs` |
| 1791-1878 | 88 | is_tool_required_terminal_reason + is_continuation_intent + is_memory_recall_intent | `intent_detection.rs` |
| 1880-1940 | 61 | message_text + recent_tool_required_without_tool + assistant_claimed_tool_execution_without_tool_message + block_is_mutating_tool_evidence | `message_helpers.rs`（crate 私有） |
| 1941-1979 | 39 | asks_for_skill_discovery + query_tokens + score_skill_candidate | `skill_resolution.rs` |
| 1980-2012 | 33 | is_read_only_planning_tool + is_plan_write_tool + is_memory_read_tool + tool_likely_mutates + input_json_contains_mutating_method | `tool_classification.rs` |
| 2014-2071 | 58 | loop_outcome_for + next_steps_for | `final_report.rs` |
| 2073-3232 | 1,160 | `mod tests` | `tests.rs` |

### 4.2 子模块清单与 LOC 估算

```
src-tauri/src/modules/application/turn_service/work_loop/
├── mod.rs                       (~250 LOC)  orchestrator: imports + 全部 pub(super) use re-exports + 跨模块共享常量 + push_unique
├── routing.rs                   (~340 LOC)  WorkLoopRouteContext + route_context_from_messages + augment_route_context_from_run_log + route_work_loop[_with_context] + context_requires_tool_execution
├── skill_resolution.rs          (~580 LOC)  resolve_skill_plan + spawn_dk_lookup_advisory + auto_load + excerpt + SkillRuntimeMetadata + skill_candidate + scoring helpers
├── tool_pool.rs                 (~170 LOC)  CanonicalToolPool + build/enforce/policy_label/schema_hash
├── tool_classification.rs       (~80 LOC)   is_read_only_planning_tool + is_plan_write_tool + is_memory_read_tool + tool_likely_mutates + input_json_contains_mutating_method
├── prompt_contributions.rs      (~180 LOC)  requires_*_evidence + 4 个 prompt contribution 构造
├── guards.rs                    (~80 LOC)   is_incomplete_memory_lookup_response + should_force_final_after_memory_recall + mutation_block_reason
├── intent_detection.rs          (~430 LOC)  looks_like_direct_answer + is_tool_required_work_intent + is_direct_shell_command_intent + is_continuation_intent + is_memory_recall_intent + is_tool_required_terminal_reason
├── assistant_signals.rs         (~170 LOC)  assistant_claims_tool_execution_without_tool + assistant_signals_tool_intent + tool_intent_nudge_message + detect_repetitive_model_output
├── textual_tool_calls.rs        (~250 LOC)  normalize_dsml_delimiters + detect_textual_tool_call_markup + extract_textual_tool_calls + DSML/XML 解析 + helpers
├── final_report.rs              (~190 LOC)  build_final_run_report + loop_outcome_for + next_steps_for
├── message_helpers.rs           (~80 LOC)   message_text + recent_tool_required_without_tool + assistant_claimed_tool_execution_without_tool_message + block_is_mutating_tool_evidence
└── tests.rs                     (~1,160 LOC) 现有 tests 全部迁入；末次 PR 内可按 sub-mod 二次拆分
```

每文件 ≤ 1,000 LOC。tests.rs 1,160 单文件超阈值，但属测试代码，不计 production god-file 阈值；可在末次 PR 顺手二次拆分。

### 4.3 `mod.rs` 骨架

```rust
//! Work-loop routing and skill-resolution helpers for `TurnService`.
//!
//! Split per GF-02 plan (2026-05-05-gf02-work-loop-split.md). All public
//! symbols continue to be reachable as `super::work_loop::xxx` from
//! turn_service siblings.

mod assistant_signals;
mod final_report;
mod guards;
mod intent_detection;
mod message_helpers;
mod prompt_contributions;
mod routing;
mod skill_resolution;
mod textual_tool_calls;
mod tool_classification;
mod tool_pool;

#[cfg(test)]
mod tests;

// Cross-module reason-code constants live here so any sub-module can
// `use super::REASON_X;` without circular imports.
pub(super) const AUTO_SKILL_TOOLS: &[&str] = &[…];
pub(super) const MEMORY_RECALL_INTENT_REASON: &str = "memory_recall_intent";
// … (the 6 reason codes + MEMORY_TOOLS / READ_ONLY_PLANNING_TOOLS / MUTATING_TOOL_NAMES / MEMORY_READ_TOOLS)

// Re-export the entire pub(super) surface so sibling import paths
// (super::work_loop::xxx) keep working byte-for-byte.
pub(super) use assistant_signals::{
    assistant_claims_tool_execution_without_tool, assistant_signals_tool_intent,
    detect_repetitive_model_output, tool_intent_nudge_message,
};
pub(super) use final_report::build_final_run_report;
pub(super) use guards::{
    is_incomplete_memory_lookup_response, mutation_block_reason,
    should_force_final_after_memory_recall,
};
pub(super) use prompt_contributions::{
    continuation_context_prompt_contribution, memory_recall_prompt_contribution,
    requires_memory_recall_evidence, requires_single_shell_command_evidence,
    requires_tool_execution_evidence, single_shell_command_prompt_contribution,
    tool_required_prompt_contribution,
};
pub(super) use routing::{
    augment_route_context_from_run_log, route_context_from_messages, route_work_loop,
    route_work_loop_with_context, WorkLoopRouteContext,
};
pub(super) use skill_resolution::{auto_load_trusted_skill_context, resolve_skill_plan};
pub(super) use textual_tool_calls::{
    detect_textual_tool_call_markup, extract_textual_tool_calls, normalize_dsml_delimiters,
};
pub(super) use tool_pool::{build_canonical_tool_pool, CanonicalToolPool};

pub(crate) fn push_unique(values: &mut Vec<String>, value: String) { … }
```

### 4.4 跨子模块依赖图

```
              ┌──────────────────┐
              │ tool_classification │ ← used by tool_pool, guards
              └──────────────────┘
                         │
        ┌────────────────┴────────────────────┐
        ▼                                     ▼
┌──────────────┐                       ┌──────────────┐
│  tool_pool   │                       │   guards     │
└──────────────┘                       └──────────────┘

┌─────────────────┐    ┌────────────────────┐
│ intent_detection │ ← │ message_helpers     │
└─────────────────┘    └────────────────────┘
        │                        │
        ▼                        ▼
┌──────────────┐          ┌──────────────────┐
│   routing    │ ──────→  │ prompt_contributions │
└──────────────┘          └──────────────────┘

┌──────────────────┐         ┌────────────────────┐    ┌──────────────────┐
│ assistant_signals │         │  textual_tool_calls │   │  final_report    │
└──────────────────┘         └────────────────────┘    └──────────────────┘

┌────────────────────┐
│ skill_resolution   │ ← uses runtime/prompt + skills + DK store
└────────────────────┘
```

无循环依赖。

### 4.5 关键设计决策

1. **保留 `pub(super)`**：split 后通过 `pub(super) use` 在 `mod.rs` 重新导出。turn_service 之外无人能 import，符合 ARCHITECTURE §4.2 边界。
2. **常量住 `mod.rs`**：reason code 字符串与 needle 数组常量是跨子模块共用的「事实定义」；放 `mod.rs` 而不是单独 `constants.rs`，保持 facade 自包含。
3. **`push_unique` 公共化为 `pub(crate)`**：3 个子模块需要它；私有化到 mod.rs 简化复用。
4. **`tests.rs` 不立刻细拆**：先一次性整体迁入 `work_loop/tests.rs`，所有 `use super::*;` 改为 `use super::super::*;`；末次 PR 体量舒适时再二次拆分。
5. **不增加 wrapper**：`mod.rs` 只 re-export，没有 trampoline——避免 LOC 浪费 + 调用栈层级噪音。

## 5. PR 拆分（5 PR 渐进）

### PR-GF02-A —「叶子簇」抽取 + git mv 切换到子目录布局
抽 `tool_classification.rs` + `textual_tool_calls.rs` + `assistant_signals.rs` + `message_helpers.rs`。
- 第一步即把 `work_loop.rs` 重命名为 `work_loop/mod.rs`（`git mv`），然后增量抽出 4 个子文件。后续 PR 全部基于子目录布局。
- LOC：work_loop/mod.rs 从 3,232 → 约 2,650（-580）

### PR-GF02-B — `tool_pool.rs` + `guards.rs` + `final_report.rs`
- LOC：~2,650 → ~2,250

### PR-GF02-C — `intent_detection.rs`（needle 列表大户）
抽 6 个 intent 函数 + needle 列表常量（move 而非 re-export）。
- LOC：~2,250 → ~1,800

### PR-GF02-D — `prompt_contributions.rs` + `routing.rs`
依赖前面已抽出的子模块，必须最后做。
- LOC：~1,800 → ~1,400（仅剩 skill_resolution + tests + facade）

### PR-GF02-E — `skill_resolution.rs` + `tests.rs`
抽 skill_resolution 整片。把 `mod tests` 整体迁出到 `work_loop/tests.rs`。`work_loop/mod.rs` 收敛为纯 facade（≤ 250 LOC）。
- LOC：~1,400 → ~250；新增 tests.rs ~1,160

每 PR 必须保持：
- `cargo check` 通过
- `cargo test turn_service --lib` 全绿
- `git diff --stat` 显示行为零变更

## 6. TDD 策略

**核心 invariant**：「机械拆分」期间所有现有测试**逐字节通过**。

### 6.1 Pre-flight（PR-A 启动前）
1. `cargo test turn_service --lib --no-fail-fast 2>&1 | tee /tmp/work_loop_baseline.txt`
2. 提取通过测试名单作为 baseline；任何 PR 内若有测试名消失或新增 `#[ignore]`，PR 必须 fail review。
3. `cargo expand` diff 只允许模块路径变化。

### 6.2 Per-PR 抽出 fn 的边界测试
每 PR 抽出子模块**之前**，先在 `work_loop` 现有 `mod tests` 中**补一组针对要抽出 fn 的纯函数测试**（如果当前覆盖弱）。重点补：

- PR-A：`tool_likely_mutates("bash", "{}")`、`is_plan_write_tool("write_file")`、`is_memory_read_tool("memory_recall")`、`normalize_dsml_delimiters` 输出含 fullwidth `｜`、`extract_textual_tool_calls(deepseek_dsml_sample)`、`extract_textual_tool_calls(xml_invoke_sample)`、`assistant_signals_tool_intent("Let me check the file")`、`detect_repetitive_model_output(stutter_sample)`、`recent_tool_required_without_tool(messages_with_recent_tool_use)` → None
- PR-B：`enforce_tool_definitions_for_loop` 4 种 loop_kind 各 1 测试；`mutation_block_reason` allowlist vs 写工具
- PR-C：`is_tool_required_work_intent("帮我做一个完整网页")` → true；`is_direct_shell_command_intent("ls -la")` vs `"请帮我看看 ls 是什么"` → true / false
- PR-D：`route_work_loop_with_context` 在 inherited_tool_required path 下 reason_codes 包含 `inherited_tool_required_work_intent`；`continuation_context_prompt_contribution` 在 facts 为空时返回 None
- PR-E：`resolve_skill_plan` 当 active_skill_ids 含未知 id → 该 id 进 candidates 且 blocked_reason 非空；`auto_load_trusted_skill_context` 对 untrusted source candidate 设 auto_load_allowed=false

### 6.3 Sibling 调用 invariant
全部 sibling 调用点（`grep "super::work_loop::"` 共 30+ 行）**不允许**在本 plan 任何 PR 中修改 import 文本。CI grep gate（`grep -rn "use super::work_loop\|super::work_loop::" src-tauri/src` 行数与 baseline 比对）保证此点。

## 7. 文件清单（按 PR）

### PR-GF02-A
| 文件 | 操作 |
|------|------|
| `…/turn_service/work_loop.rs` | `git mv` 到 `work_loop/mod.rs` + 删除被抽走的 fn |
| `…/work_loop/tool_classification.rs` | NEW |
| `…/work_loop/textual_tool_calls.rs` | NEW |
| `…/work_loop/assistant_signals.rs` | NEW |
| `…/work_loop/message_helpers.rs` | NEW |
| `…/work_loop/mod.rs` | EDIT：增 4 个 `mod` + 4 组 `pub(super) use` |

### PR-GF02-B
| 文件 | 操作 |
|------|------|
| `…/work_loop/tool_pool.rs` | NEW |
| `…/work_loop/guards.rs` | NEW |
| `…/work_loop/final_report.rs` | NEW |
| `…/work_loop/mod.rs` | EDIT |

### PR-GF02-C
| 文件 | 操作 |
|------|------|
| `…/work_loop/intent_detection.rs` | NEW（含中英 needle 列表常量） |
| `…/work_loop/mod.rs` | EDIT |

### PR-GF02-D
| 文件 | 操作 |
|------|------|
| `…/work_loop/prompt_contributions.rs` | NEW |
| `…/work_loop/routing.rs` | NEW |
| `…/work_loop/mod.rs` | EDIT |

### PR-GF02-E
| 文件 | 操作 |
|------|------|
| `…/work_loop/skill_resolution.rs` | NEW |
| `…/work_loop/tests.rs` | NEW（迁入原 mod tests） |
| `…/work_loop/mod.rs` | EDIT：收敛为 facade（~250 LOC） |

## 8. 验证（每 PR 必跑）

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml turn_service --lib --no-fail-fast
cargo test --manifest-path src-tauri/Cargo.toml work_loop --lib --no-fail-fast
npm test
npm run build:web

# Import-path invariant gate
grep -rn "super::work_loop::\|use super::work_loop" src-tauri/src --include="*.rs" | wc -l
```

末次 PR 额外：
- `diff <(grep "test result" baseline) <(grep "test result" after)` 应仅 0 行差异
- 手工 smoke：start chat turn → 触发权限 → 完成 turn → reload → 读 history

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 抽出后 needle 列表行为改变 | 抽出前 boundary tests 覆盖每个 intent fn 的核心 needle；diff `cargo expand` 输出确认常量数组逐字保留 |
| `pub(super) use` 重新导出遗漏导致 sibling 编译失败 | 每 PR 抽出后立即 `cargo check`；grep gate 验证 import 路径数量；CI 必跑 |
| 测试 `use super::*;` 路径在拆分后失效 | PR-E 拆 tests 时统一加 `use super::super::*;` 或按子模块拆 tests 内嵌 |
| 末次 PR 一次性 mv tests 体量过大 | 提供「先 git mv tests block，再删除原文件中的 #[cfg(test)] 块」的两-commit 拆法 |
| DR-01 supervisor facade 未稳定 | 本 plan 不直接 import supervisor；但 stream_finalize / stream_task 内的 supervisor 调用与 work_loop 同 PR 域。建议 GF-02 严格在 DR-01 PR-A merge 后再开 PR-A |
| 中英文 needle 列表 commit diff 噪音大 | PR 描述显式贴 `git diff --stat` + reviewer checklist「逐数组验证移动而非修改」 |
| `#[path]` 同目录共存导致 Rust 编译歧义 | 直接采用 `git mv work_loop.rs work_loop/mod.rs` 一次到位 |

## 10. PR 描述模板（PR-A 示例）

```markdown
## 改善 ID
GF-02 PR-A — work_loop split (leaf clusters extraction)
(docs/IMPROVEMENTS-2026-05-05.md §4)

## 变更摘要
将 turn_service/work_loop.rs (3,232 LOC) 重命名为 work_loop/mod.rs，
并抽出 4 个叶子子模块：tool_classification / textual_tool_calls /
assistant_signals / message_helpers。所有 pub(super) 符号通过 mod.rs
重新导出，sibling import 路径不变。零行为变化。

## Before / After
- LOC（work_loop/mod.rs）: 3,232 → ~2,650
- 公共 API 符号数：23 → 23（不变）
- sibling import 行数：30+ → 30+（不变；grep gate 已校验）
- 新增 boundary tests：8 个

## 验证
- [x] cargo fmt --check
- [x] cargo clippy -D warnings
- [x] cargo test turn_service --lib（baseline ↔ after diff = 0）
- [x] npm test + build:web
- [x] grep "super::work_loop::" 行数 ≥ baseline

## 关联
- Plan: docs/superpowers/plans/2026-05-05-gf02-work-loop-split.md
- Wave: docs/superpowers/plans/2026-05-05-improvements-wave1-overview.md
- Depends on: DR-01 PR-A merged（supervisor facade 稳定）
- Followups: PR-B, PR-C, PR-D, PR-E
```

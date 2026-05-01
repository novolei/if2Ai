# ARCHITECTURE + evolution spec 真值同步

> **For agentic workers:** 按 `CLAUDE.md` Superpowers 流程；本计划为 **write-only / 文档优先**，后续代码主线另开计划。

**Goal:** 消除 `ARCHITECTURE.md` 与 `.qoder/specs/if2ai-agent-evolution-report.md` 相对 `codebase` 的二次真相，并锁定下一阶段代码收敛入口。

**Architecture:** 文档与代码对齐 → 再选单主线（预算 30K **或** Stream/projection 收口）避免并行大改。

**Tech Stack:** Markdown only（本迭代）；后续 Rust/TS。

---

## Phase A — 文档勘误（本迭代 ✅）

- **A1** `ARCHITECTURE.md` §7：supervisor 叙述与 `runtime/supervisor.rs`（MIG-020）对齐。
- **A2** `ARCHITECTURE.md` §7 P2：memory UI 多读面 — 与 `MemoryBrowser` / `MemoryDebugTab` 走 `src/api/memory` 的现状对齐。
- **A3** `.qoder/specs/if2ai-agent-evolution-report.md` Part 1.3「关键结论」：去掉对 GAP 的依赖句，与文首「文档治理」一致。
- **A4** `ARCHITECTURE.md` §5.3：permission 路径与已实现基础设施语气对齐（仍承认 live channel 风险若存在）。

## Phase B — 代码主线

**B1 执行记录（2026-05-01）**（`N/A` 写法须符合 `CLAUDE.md`「N/A 书写标准」）

- **Brainstorming: N/A** — Skill `brainstorming`（`SKILL.md` 文前 `description`）针对 *creative work / creating features / modifying behavior*；本轮无新增产品或交互方案，仅落实本文 **Phase B1** 与真值 [`.qoder/specs/if2ai-agent-evolution-report.md`](../../.qoder/specs/if2ai-agent-evolution-report.md) **Module A**（默认请求字符上限 ~30K）及 **Part 6** 矩阵 #1 已描述之目标。
- **§5 systematic-debugging: N/A** — Skill `systematic-debugging`（`SKILL.md` §「When to Use」）要求遇 *bug / test failure / unexpected behavior / build failure* 等再进入；本轮为按上条真值之正向实现，无待查根因。
- **§8 TDD: N/A** — Skill `test-driven-development`（`SKILL.md` §「Exceptions」）可与人类约定验收形态；本仓库对 `budget.rs` env 解析类改动采用 **同模块单元测试**（与 `streaming_tier_budget` 测试同锁策略）+ 本计划所列 `cargo test` 为证；真值锚点同 **Module A** 表行「默认请求字符上限」。
- **B1** `MAX_REQUEST_CHAR_BUDGET` 与 30K 产品目标 + 观测闭环（对齐 evolution Module A / Part 1 #1）。
- **B2** `StreamTokenPayload` / `App.tsx` 热路径 vs projection-only UI（对齐 ARCH §3.3 / §7）。
- **B3** harness `final_run_report` vs canonical event log 派生（ARCH §6 / MIG-023 方向）。

## Phase C — spec 全文扫尾（可选）

- 将 evolution 正文其余「GAP §x」脚注改为「以仓库代码为准」或删除（Module E/H 等段）。

## Phase D — Superpowers 执行纪律（用户指令：任何代码改动 100% 流程）

- [x] **D1** 收窄豁免并写明 `N/A` 规则：`CLAUDE.md`、`AGENTS.md`、`.cursor/rules/superpowers-workflow.mdc`。
- [x] **D2** 收紧计划内 `N/A` 书写标准：`CLAUDE.md` 新增「N/A 书写标准」、`AGENTS.md` / `superpowers-workflow.mdc` 摘要；本文件 **B1 执行记录**三条 `N/A` 按新标准重写。

**Verification（D1）:** 通读上述三处「唯一豁免 / 硬顺序」段落一致；`rg "单文件纯文案" CLAUDE.md AGENTS.md` 无残留（已删除小修豁免）。

**Verification（D2）:** `rg "书写标准（收紧）" CLAUDE.md` 命中；本文件 B1 执行记录每条 `N/A` 均含 skill 名 + `SKILL.md` 位置指称 + evolution「Module A」或「Part 6」锚点。

---

**Verification（本迭代）:** 人工通读三处修改 + `rg 'GAP §' .qoder/specs/if2ai-agent-evolution-report.md` 计数下降（不要求为零）。
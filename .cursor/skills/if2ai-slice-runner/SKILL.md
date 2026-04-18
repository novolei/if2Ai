---
name: if2ai-slice-runner
description: Executes one If2Ai exec-plan slice end-to-end following the 17-step SOP from CLAUDE.md (READ → GAP analysis → IMPL → LINT → GATE → REVIEW → DIFF-GATE → COMMIT → UPDATE). Use when the user asks to "执行 slice", "实现下一个 slice", "继续 exec-plan", "run slice X.Y", "implement next pending slice", or any request that involves picking up work from docs/exec-plans/active/*.yaml. Also use when an executor session starts and needs to find the current Phase. Do NOT use for ad-hoc one-off code edits unrelated to a slice.
---

# If2Ai Slice Runner

Executes ONE pending slice from the active exec-plan, following the strict 17-step SOP defined in `CLAUDE.md`. This skill is a thin orchestrator — the source of truth is always `CLAUDE.md` + the slice's `design_ref`.

## Hard Prerequisites (read these FIRST, in order)

```
1. CLAUDE.md                                          # 项目 executor 行为规范（17 步 SOP）
2. docs/references/coding-style-and-lint-contract.md  # Rust 编码 + lint 合约（全局约束）
3. 调用 find_current_slice.py 一键拿到 Phase + slice  # 见 Step 1 / Discovery
4. slice.design_ref                                   # 必读，禁止跳过
5. slice.impl_targets 中现有文件                       # 理解当前状态
```

> 如果跳过 1-2，后续 lint / review 一定挂。如果跳过 3-5，会改错文件或写错接口。

## 上下文加载（省 token）

| 场景 | 必读 | 勿读 |
|------|------|------|
| **执行 slice**（本 skill） | `CLAUDE.md` → `coding-style-and-lint-contract.md` → `find_current_slice.py` 输出中的 `design_ref` + `impl_targets` 对应文件 | 勿 `Glob` / 批量 `Read` 整个 `docs/design-docs/`；勿在未点名时通读 `docs/product-specs/` |
| **ad-hoc 改 Rust**（非 slice） | `.cursor/rules/rust.mdc` + `coding-style-and-lint-contract.md` | 不必整份 `CLAUDE.md`（无 harness 门禁时） |
| **ad-hoc 改前端** | `.cursor/rules/frontend.mdc` | — |

**设计文档**：只读当前 slice 的 `design_ref` 路径（单文件或明确章节）。需要第二份设计时再 `@` 具体路径；禁止「先扫一遍 `docs/design-docs/`」。

**harness / CI 日志**：粘贴或摘要时只保留失败用例与关键 JSON 字段，禁止整段无筛选 log。

## Quick Workflow Checklist

复制并跟踪每个 slice 的状态：

```
Slice <id> Progress:
- [ ] 1. READ: 跑 find_current_slice.py 拿到 slice 元数据 → 读 design_ref + impl_targets 现状
- [ ] 2. GAP analysis（必须输出非空 TO-DO LIST）
- [ ] 3. IMPL（按 TO-DO LIST 严格实现 design_ref 接口）
- [ ] 4. LINT: cargo fmt + clippy -D warnings + cargo test 全绿
- [ ] 5. GATE: python -m harness.runner run --slice <id> --workspace .
- [ ] 6. REVIEW: 调用 code-reviewer subagent → REVIEW_PASS
- [ ] 7. DIFF-GATE: python -m harness.runner diff-gate --workspace .
- [ ] 8. COMMIT (按 commit message 模板)
- [ ] 9. UPDATE exec-plan: slice.status=done + dashboard
- [ ] 10. REPORT: 追加 docs/generated/QUALITY_SCORE.md
- [ ] 11. NEXT: 自动开始下一个 pending slice（不询问）
```

完整 17 步细节见 `CLAUDE.md` "每次执行一个 Slice 的完整流程" 段，不要重复读。

## Step Detail Cheatsheet

### Step 1: 一键定位当前 slice

不要再手动 grep `**[当前]**` 和翻 YAML。直接跑：

```bash
.venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py
```

输出 Markdown，包含：当前 Phase YAML 路径、第一个 `status: pending` 的 slice id、`design_ref`、**`depends_on`（若有）**、**`meta.design_docs` 全量索引（Phase 6H 等多文档 Phase）**、`current_state`、`must_implement`、`impl_targets`、`acceptance`、`review_checklist`，并在前一个 slice 命中 `meta.human_checkpoints.after_slice` 时给出 `HUMAN_CHECKPOINT` 警告。Agent 应先把 **`design_ref` 整文件读入**；bib 列表仅作交叉引用地图，禁止未读 `design_ref` 就先通读 bib 中所有文件。

要程序化解析时用 JSON 模式：

```bash
.venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py --json
```

退出码语义：

| exit | 含义 |
|---|---|
| 0 | 找到 pending slice 或 PHASE_COMPLETE（看 stdout `status` 字段） |
| 2 | index.md / YAML 结构错误 |
| 3 | 没找到 `**[当前]**` 行，或找到多行非 strikethrough 标记 |

输出 `PHASE_COMPLETE` → 停止，输出 `HUMAN_CHECKPOINT_REACHED: Phase X 全部完成`，等人工 review。**禁止自行 promote**，除非用户明确授权全自动模式（见 CLAUDE.md "全自动模式"）。

### Step 2: GAP Analysis（最容易跳的一步）

**绝对不能跳。** 必须输出格式：

```
## Slice <id> Gap Analysis
### current_state 检查
- ✅ <file>: 已存在
- ❌ <file>: 不存在 / 接口缺失 <fn_name>

### must_implement 比对（逐条）
- [ ] <signature 1> — 现状: 缺失 / 签名不一致 / 已实现
- [ ] <signature 2> — ...

### TO-DO LIST（必须非空）
1. 在 <file> 创建 <fn>
2. 修改 <file> 的 <fn> 签名以匹配 design_ref §X
3. ...
```

**如果 TO-DO LIST 为空** → 不要进入 IMPL。说明 `current_state` 字段过期或 `design_ref` 需要细化，停下来报告问题，让人类决定。

### Step 3: IMPL 关键约束

逐条对照 `docs/references/coding-style-and-lint-contract.md` 与规则：`src-tauri/**` 用 `.cursor/rules/rust.mdc`；`src/**` 用 `.cursor/rules/frontend.mdc`。Rust 异步/并发疑难时再读 `rust-async-patterns`（正文精简；长示例见 `references/patterns-full.md`）与按需章节的 `rust-best-practices`。

红线（违反则 review 必挂）：
- 非测试代码出现 `unwrap()` / `expect()` / `todo!()` / `unimplemented!()`
- 跨模块直接 `use crate::xxx`，未走 `crate::modules::*`
- async 路径出现 `std::thread::sleep` / 锁跨 await
- pub 函数无 `///` doc / 新加 `#[allow(...)]` 无说明
- impl_targets 中的文件「一行没改」就尝试 mark done

### Step 4: LINT（命令固定）

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

任何一条非 0 → 回到 Step 3，禁止带 warning 进 REVIEW。

### Step 5: GATE

```bash
# 默认
python -m harness.runner run --slice <id> --workspace .

# 带 behavior suite
python -m harness.runner run --slice <id> --workspace . --suite harness/suites/<suite>.yaml

# 写报告（可选）
python -m harness.runner run --slice <id> --workspace . --report-out .harness-reports/slice-<id>.json
```

通过标准：stdout JSON `"passed": true` 且 exit code 0。

### Step 6: REVIEW

**首选**：直接调用 sub-agent（同 turn 内，作为新一条 message）：

```
Use the code-reviewer subagent to review slice <id>
```

**备用**（无 sub-agent 时）：

```bash
python -m harness.runner review --slice <id> --workspace .
```

输出 `REVIEW_PASS` → Step 7。`REVIEW_FAIL` → 按报告修复，回到 Step 3。

### Step 7: DIFF-GATE（防止 docs-only 蒙混）

```bash
python -m harness.runner diff-gate --workspace .
```

`DIFF_GATE FAIL` 几乎总是表示 impl_targets 中某文件未被实际修改，不要 commit，回到 Step 2 重做 gap。

### Step 8: COMMIT 模板

```
<type>(slice-<id>): <简短描述>

<可选详细说明>

Slice: <id>
Design-ref: <design_ref 文件名>
Gate: PASS (compile ✅ test ✅ behavior ✅/⏭️)
Review: PASS
```

`type` ∈ `feat | fix | refactor | test | chore`。一次 commit 只能涵盖一个 slice。

### Step 9-10: UPDATE & REPORT

- exec-plan YAML：把当前 slice 的 `status: pending` → `done`
- exec-plan YAML `dashboard`：`last_updated`、`completed_slices` 追加 id、`current_slice` 指向下一个 pending
- 追加 `docs/generated/QUALITY_SCORE.md` 一行变更记录

### Step 11: NEXT — 自动继续，不要询问

完成后立即回到 Step 1 处理下一个 pending slice。**唯一允许停下来等人**：
- 遇到 `meta.human_checkpoints.after_slice` 命中的 slice → 输出 `HUMAN_CHECKPOINT_REACHED: Phase X 全部完成，请人工 review 后继续`
- 同一 gate 修复超过 3 次仍失败 → 在 `dashboard.blocked` 记录后停止

设计文档歧义 / 缺依赖（如某个 ProviderManager 未实现）→ 选保守方案（trait + mock），在 commit message 说明，**不要询问**。

## Discovery Helpers

```bash
# 推荐：一键拿当前 slice 全量元数据（Markdown）
.venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py

# 程序化解析
.venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py --json

# 兜底（脚本异常时）：手动找当前 Phase
grep -B1 "\*\*\[当前\]\*\*" docs/exec-plans/index.md | grep -v "~~"

# 当前 Phase YAML 中所有 pending
grep -B2 "status: pending" docs/exec-plans/active/<phase>.yaml | head -40

# 验证新写的 Phase YAML 格式
python -m harness.runner check-slice --file docs/exec-plans/active/<phase>.yaml

# Phase 切换（人工确认后）
python -m harness.runner promote --workspace . --dry-run
python -m harness.runner promote --workspace .
```

## Anti-Patterns（被发现立刻回退）

| 反模式 | 正确做法 |
|---|---|
| 跳过 design_ref 凭印象写 | 必须读 design_ref 全文 |
| 批量 Read / Glob `docs/design-docs/` | 只读当前 `design_ref` + `impl_targets` |
| 默认并行读完 rust-best-practices 全部 chapter | 按任务读 1～2 章（见该 skill 内表格） |
| 跳过 GAP analysis 直接改 | 必须输出 TO-DO LIST |
| 旧测试通过 = 接口正确 | 必须逐行对照 design_ref 验证签名 |
| 一次 commit 多个 slice | 一个 commit 一个 slice |
| LINT 没通过就 REVIEW | 强制顺序，无例外 |
| DIFF_GATE FAIL 仍 mark done | 必须有真实代码改动 |
| 完成一个 slice 停下来询问"继续吗？" | 自动继续，除非命中 checkpoint |
| 修改了 design_ref 文档 | 禁止；只读不写 |

## When NOT to Use This Skill

- 用户问的是"读一下这段代码"、"这个 bug 怎么改" 这种 ad-hoc 任务 → 直接做，不要套 SOP
- 用户在做 design / planning 阶段（写 design-doc、改 product-spec） → 用 system-architect subagent
- 用户在 review 单个 PR / 单个文件 → 用 code-reviewer subagent
- Phase 已全部完成等待人工 → 输出 `HUMAN_CHECKPOINT_REACHED` 并停止，不要 promote

## References

- `CLAUDE.md` — 完整 17 步 SOP、约束、提交规范、Human Checkpoint 处理
- `docs/references/coding-style-and-lint-contract.md` — Rust 编码 + lint 合约
- `docs/exec-plans/index.md` — 当前 Phase 标记
- `.claude/agents/code-reviewer.md` — review subagent 定义
- `.claude/agents/system-architect.md` — 规划 / 审计 subagent 定义
- `.cursor/skills/rust-async-patterns/SKILL.md` — Rust async 速查；长示例见 `references/patterns-full.md`
- `.cursor/skills/rust-best-practices/SKILL.md` — Rust 通用最佳实践（按需读 chapter）
- `.cursor/rules/rust.mdc` — Tauri 2 + Rust（`src-tauri/`）
- `.cursor/rules/frontend.mdc` — React + TS + Tailwind v4（`src/`）
- `.cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py` — Step 1 自动化：定位当前 Phase + 第一个 pending slice，输出全量元数据

# CLAUDE.md — If2Ai Executor 行为规范

> 本文件是 claude code（executor）的行为约束文档。  
> executor 每次启动时**必须按顺序读取以下文件**，然后再读当前 slice 的 design_ref：
>
> 1. 本文件（CLAUDE.md）
> 2. `docs/references/coding-style-and-lint-contract.md` — **Rust 编码规范和 lint 合约，全局约束每一步代码实现**
> 3. 当前 slice 的 `design_ref` 文档

---

## 角色定义

你是 **If2Ai 自动开发 executor**。  
你的工作是：按照 `docs/exec-plans/active/` 目录下当前 Phase 的 YAML 文件中的 slices，  
逐一实现代码，直到该 Phase 所有 slice 状态变为 `done`。

**如何确定当前 Phase 文件**：读取 `docs/exec-plans/index.md`，找到标记为「当前」的计划文件路径。

---

## 每次执行一个 Slice 的完整流程

```
1. READ   → 读取 exec-plan，找到第一个 status: pending 的 slice
2. READ   → 读取该 slice 的 design_ref 文档（必须，不能跳过）
3. READ   → 读取 impl_targets 中的现有文件（理解当前状态）
4. GAP    → **先做 gap analysis，再动笔**：
           a. 读取 slice 的 `current_state` 字段，确认哪些文件存在、哪些对应接口缺失
           b. 读取 slice 的 `must_implement` 字段，逐条确认每个签名是否在文件中已存在
           c. 如果 impl_targets 中的文件已存在，**逐段对照 design_ref 验证接口一致性**
           d. 列出 "TO-DO LIST"：缺失的函数/struct/文件，**此列表必须非空才能继续**
           ❌ 如果 gap analysis 发现所有接口都已存在 → 说明 current_state 有误或 design_ref
              定义需要细化，必须停下来检查，而不是直接跳到步骤 5
5. IMPL   → 按 gap analysis 的 TO-DO LIST 实现代码，严格遵守 design_ref 中的接口定义；
           遵守 coding-style-and-lint-contract.md 的 Code Shape / Modularization 规则
6. LINT   → 强制运行 lint 合约（见下方 Lint 合约）：cargo fmt + clippy + test 全部通过方可继续
7. GATE   → 运行 harness gate：python -m harness.runner run --slice <id> --workspace .
8. FIX    → 如果有失败的 gate 或 lint，修复代码，回到步骤 5
9. REVIEW → 使用 code-reviewer sub-agent 审查代码（`.claude/agents/code-reviewer.md`）：
           直接说「Use the code-reviewer subagent to review slice <id>」
           或运行备用命令：python -m harness.runner review --slice <id> --workspace .
           sub-agent/命令输出 REVIEW_PASS 才能继续；REVIEW_FAIL 则回到步骤 5
10. FIX      → 如果 review 有 FAIL 条目，修复，回到步骤 5
11. DIFF-GATE → 运行 python -m harness.runner diff-gate --workspace .
             输出 DIFF_GATE PASS 才能继续；
             DIFF_GATE FAIL 意味着没有写代码，必须回到步骤 4 实现代码。
             ❌ 严禁：仅凭迁移来的旧代码已通过测试就标记 slice done
12. COMMIT → git commit（见下方提交规范）
13. UPDATE → 更新 exec-plan YAML 中该 slice 的 status 为 done
14. UPDATE → 更新 exec-plan YAML 的 dashboard 区域
15. REPORT → 更新 docs/generated/QUALITY_SCORE.md（追加变更记录一行）
16. STATUS → 运行 python -m harness.runner status --workspace . 并将输出写入执行日志
17. NEXT   → 回到步骤 1，处理下一个 slice
```

**停止条件**：所有 slice 都是 `status: done`，或遇到 `human_checkpoint`。

> **⚠️ 代码实现原则（最高优先级）**：  
> 每个 slice 的 `impl_targets` 列出了需要创建或修改的文件。  
> **impl_targets 中的每个文件都必须被本 slice 的实现修改过**，否则不能进入步骤 11 DIFF-GATE。  
> 如果 impl_targets 中的文件已经存在（迁移来的旧代码），你**必须逐行对照 design_ref 验证接口定义**，修复不一致之处，并补充缺失的接口。  
> 「测试已通过」**不等于**「接口正确实现」——旧代码可能存在错误的接口，测试只测了已有函数。

> **🔁 自动继续原则（重要）**：  
> 完成一个 slice 的步骤 17 NEXT 后，**立即自动开始下一个 pending slice，不要暂停、不要询问用户是否继续**。  
> 唯一允许停下来等待人工的场合是：遇到 `human_checkpoint`，或 gate 失败超过 3 次进入 blocked 状态。  
> 对于设计文档中的歧义或依赖缺失（如 ProviderManager 未实现），选择保守方案（用 trait + mock）自行决策并在 commit message 中说明，不要停下来询问。

---

## 必须遵守的约束

> **全局参考**：`docs/references/coding-style-and-lint-contract.md` 是所有 Rust 代码的完整规范。下表是从中提炼的强制检查项。

### 代码约束

| 规则                                                | 来源             | 检查方式                   |
| --------------------------------------------------- | ---------------- | -------------------------- |
| 无 `unwrap()`（测试除外）                           | lint-contract    | gate compile_gate / review |
| 无硬编码 API key / 路径                             | lint-contract    | review                     |
| 跨模块引用必须用 `crate::modules::*`                | lint-contract    | gate compile_gate          |
| 所有公开函数必须有 Rust doc 注释                    | lint-contract    | review                     |
| 异步代码用 tokio，不用 std::thread                  | lint-contract    | review                     |
| `Result<T, E>` 错误传播用 `?`，不用 `match` 嵌套    | lint-contract    | review                     |
| 默认用 `pub(crate)`，`pub` 仅在边界真实存在时       | Code Shape Rules | review                     |
| 一个文件只拥有一个有界职责                          | Code Shape Rules | review                     |
| `main.rs` / bin 只做编排，不含业务逻辑              | Code Shape Rules | review                     |
| 按有界上下文分模块，不按文件大小                    | Modularization   | review                     |
| 新增 `allow(...)` 必须在代码或 slice 记录中说明理由 | Lint Contract    | review                     |

### 强制 Lint 合约（步骤 6 LINT）

每个 slice 在提交前 **必须全部通过**：

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- 遵守 `rust/Cargo.toml` 中的 workspace lint 设置
- clippy warning 视为重构或拆分模块的信号，不是可忽略的噪音
- lint 失败时回到步骤 5，不允许带警告提交

### 禁止行为

- ❌ 不允许在 acceptance 没有通过的情况下执行 git commit
- ❌ 不允许修改 design_ref 文档（只能读，不能改）
- ❌ 不允许在不理解 design_ref 的情况下开始实现
- ❌ 不允许一次提交多个 slice 的代码
- ❌ 不允许跳过 slice（除非 status: skip，否则必须顺序执行）
- ❌ 不允许使用 `todo!()` 或 `unimplemented!()` 留给以后
- ❌ 不允许在 lint/fmt/clippy 未全部通过的情况下调用 REVIEW
- ❌ 不允许在文档落后于代码时扩大 slice 范围（先更新文档）
- ❌ **不允许在没有修改 impl_targets 中任何文件的情况下标记 slice done**
- ❌ **不允许认为「迁移来的旧代码已通过测试 = 接口已实现」**；必须对照 design_ref 验证并更新代码
- ❌ 不允许在 DIFF_GATE FAIL 的情况下进行 git commit 或更新 slice 状态为 done

---

## Harness Gate 调用方式

```bash
# 基础调用（Layer 1 + Layer 2）
python -m harness.runner run --slice 1.2 --workspace .

# 带 behavior suite（Layer 3）
python -m harness.runner run \
  --slice 1.5 \
  --workspace . \
  --suite harness/suites/tool_registry_basic.yaml

# 仅运行特定测试
python -m harness.runner run \
  --slice 1.3 \
  --workspace . \
  --test-filter "runtime::conversation"

# 结果写入文件（用于 CI / dashboard）
python -m harness.runner run \
  --slice 1.3 \
  --workspace . \
  --report-out .harness-reports/slice-1.3.json
```

**gate 通过标准**：stdout 中 JSON 的 `"passed": true`，且 exit code 为 0。

---

## 自动 Code Review 规范

Review 步骤使用项目内置的 **code-reviewer sub-agent**（`.claude/agents/code-reviewer.md`）。

### 首选方式：调用 sub-agent

```
Use the code-reviewer subagent to review slice <id>
```

Claude Code 会自动委托给 code-reviewer sub-agent，它会：读取 git diff → 检查 coding-style 规范 → 验证 review_checklist → 输出 `REVIEW_PASS` 或 `REVIEW_FAIL`。

### 备用方式：命令行（无 sub-agent 时）

```bash
python -m harness.runner review --slice <id> --workspace .
```

该命令自动检查：

| 检查项                        | 说明                              |
| ----------------------------- | --------------------------------- |
| `cargo fmt --check`           | 格式是否符合规范                  |
| `cargo clippy -D warnings`    | 无 lint 警告                      |
| `no unwrap()/expect()`        | 非测试代码中无不安全调用          |
| `no todo!()/unimplemented!()` | 无未完成占位符                    |
| `no hardcoded secrets`        | 无硬编码 API key                  |
| `pub fn has /// doc comment`  | 公开函数有文档注释                |
| slice review_checklist        | 打印当前 slice 的检查项供人工参考 |

输出 `REVIEW_PASS` → 继续步骤 9  
输出 `REVIEW_FAIL` → 修复对应条目，回到步骤 5

---

## Git 提交规范

提交信息格式：

```
<type>(slice-<id>): <简短描述>

<可选的详细说明>

Slice: <id>
Design-ref: <design_ref 文件名>
Gate: PASS (compile ✅ test ✅ behavior ✅/⏭️)
Review: PASS
```

type 使用：`feat` / `fix` / `refactor` / `test` / `chore`

示例：

```
feat(slice-1.3): implement ConversationRuntime core loop

- Implement run() state machine per agent-loop.md
- Add token budget enforcement before each LLM call
- Use ProviderManager trait for LLM abstraction

Slice: 1.3
Design-ref: docs/design-docs/agent-loop.md
Gate: PASS (compile ✅ test ✅ behavior ⏭️)
Review: PASS
```

---

## 遇到阻塞时的处理

| 情况                             | 行动                                                      |
| -------------------------------- | --------------------------------------------------------- |
| gate 失败，修复超过 3 次仍不通过 | 在 exec-plan dashboard.blocked 中记录，停止并等待人工介入 |
| design_ref 有歧义或矛盾          | 选择更保守的解释，在 commit message 中说明                |
| 依赖的 slice 未完成              | 检查前序 slice，如果是 pending 先完成它                   |
| 编译错误超出当前 slice 范围      | 只修复与当前 slice 相关的错误，其他的记录到 blocked       |

---

## Human Checkpoint 处理

当完成 `meta.human_checkpoints` 中 `after_slice` 指定的 slice 后：

1. 确保所有之前的 slice 都是 `status: done`
2. 在 exec-plan 的 `dashboard.notes` 中写明等待人工 review
3. 停止执行，**不要自动开始下一个 Phase**
4. 输出：`HUMAN_CHECKPOINT_REACHED: Phase X 全部完成，请人工 review 后继续`

> **全自动模式（仅在人类明确授权后启用）**：  
> 如果人类在启动前声明"全自动执行，无需 checkpoint 停止"，则在步骤 4 之后额外执行：  
> `python -m harness.runner promote --workspace .`  
> 该命令自动将 index.md 中下一 Phase 标记为 `[当前]`，并将对应 YAML 的 `phase_status` 改为 `active`，  
> 然后继续回到步骤 1 执行下一 Phase。  
> **默认行为是停止等待人工**，不要自行切换到全自动模式。

---

## Phase 规划流程（下一个 Phase 的 YAML 由谁创建）

**规则：executor 只执行已批准的 slice。规划权归人类。**

流程如下：

```
人类收到 HUMAN_CHECKPOINT_REACHED 消息
    ↓
人类 review 已完成的代码
    ↓
人类触发："请根据 docs/design-docs/ 生成 Phase N+1 的 exec-plan YAML"
    ↓
AI（Copilot / Claude）读取相关 design-docs，生成 phase-N+1-xxx.yaml
    ↓
人类 review 并确认 slice 定义质量
    ↓
人类将文件放入 docs/exec-plans/active/ 并更新 docs/exec-plans/index.md
    ↓
Executor 继续执行新 Phase
```

### AI 生成新 Phase YAML 时的参考资料

生成下一个 Phase 的 exec-plan 时，AI 应读取：

| 来源                                      | 目的                                                             |
| ----------------------------------------- | ---------------------------------------------------------------- |
| `docs/design-docs/README.md`              | 了解整体架构层和各 Phase 对应的模块                              |
| `docs/design-docs/<相关模块>.md`          | 获取接口定义和约束，作为 `design_ref` 和 `review_checklist` 来源 |
| `docs/product-specs/index.md`             | 了解产品功能需求，确定 slice 的业务目标                          |
| `docs/exec-plans/active/<当前Phase>.yaml` | 了解已完成的模块，避免重复，确保依赖顺序                         |

### 生成的 YAML 必须满足

- 每个 slice 有明确的 `impl_targets`（具体文件路径）
- `acceptance` 条目可被 `harness/runner.py` 直接执行
- `review_checklist` 条目直接引用 `design_ref` 中的接口规范
- slice 粒度：单人 1-2 天内可完成
- 生成后运行验证：`python -m harness.runner check-slice --file <新文件路径>`

---

## Dashboard 更新格式

每次完成一个 slice 后，更新 exec-plan YAML 的 dashboard 区域：

```yaml
dashboard:
  last_updated: 'YYYY-MM-DD'
  completed_slices: ['1.1', '1.2', ...] # 追加刚完成的 slice id
  current_slice: '1.3' # 更新为下一个 pending slice
  blocked: [] # 如有阻塞，在此记录
  notes: '简短描述当前状态'
```

---

## 快速参考

```bash
# 启动时必读（按顺序）
cat CLAUDE.md
cat docs/references/coding-style-and-lint-contract.md

# 激活 harness 虚拟环境（首次需要：python3 -m venv .venv && .venv/bin/pip install pyyaml）
source .venv/bin/activate

# 步骤 6 LINT — 每个 slice 必须全部通过
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 步骤 9 REVIEW — 自动静态审查（替代 sub-agent）
python -m harness.runner review --slice <id> --workspace .

# 步骤 11 DIFF-GATE — 验证有实际代码变更（严禁 docs-only 完成 slice）
python -m harness.runner diff-gate --workspace .

# 验证 exec-plan 格式
python -m harness.runner check-slice \
  --file docs/exec-plans/active/phase-1-foundation.yaml

# 运行单个 slice 的全部 gates
python -m harness.runner run --slice <id> --workspace .

# 查看当前 Phase 文件
cat docs/exec-plans/index.md

# 查看当前 pending slices（替换为实际的 Phase 文件名）
grep -B1 "status: pending" docs/exec-plans/active/<current-phase>.yaml

# 验证新生成的 Phase YAML 格式
python -m harness.runner check-slice --file docs/exec-plans/active/<new-phase>.yaml

# Phase 切换：预览 + 执行（人工 review 后运行，或全自动模式下由 executor 调用）
python -m harness.runner promote --workspace . --dry-run   # 预览，不修改文件
python -m harness.runner promote --workspace .             # 正式切换到下一 Phase

# 生成 git diff 用于 review
git diff --staged
```

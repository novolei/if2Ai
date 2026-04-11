# CLAUDE.md — If2Ai Executor 行为规范

> 本文件是 claude code（executor）的行为约束文档。  
> executor 每次启动时**必须先读本文件**，然后再读当前 slice 的 design_ref。

---

## 角色定义

你是 **If2Ai 自动开发 executor**。  
你的工作是：按照 `docs/exec-plans/active/phase-1-foundation.yaml` 中的 slices，  
逐一实现代码，直到该 Phase 所有 slice 状态变为 `done`。

---

## 每次执行一个 Slice 的完整流程

```
1. READ   → 读取 exec-plan，找到第一个 status: pending 的 slice
2. READ   → 读取该 slice 的 design_ref 文档（必须，不能跳过）
3. READ   → 读取 impl_targets 中的现有文件（理解当前状态）
4. IMPL   → 实现代码，严格遵守 design_ref 中的接口定义
5. GATE   → 运行 harness gate：python -m harness.runner run --slice <id> --workspace .
6. FIX    → 如果有失败的 gate，修复代码，回到步骤 5
7. REVIEW → 调用 sub-agent 做 code review（见下方 Review 规范）
8. FIX    → 如果 review 有 FAIL 条目，修复，回到步骤 5
9. COMMIT → git commit（见下方提交规范）
10. UPDATE → 更新 exec-plan YAML 中该 slice 的 status 为 done
11. UPDATE → 更新 exec-plan YAML 的 dashboard 区域
12. NEXT  → 回到步骤 1，处理下一个 slice
```

**停止条件**：所有 slice 都是 `status: done`，或遇到 `human_checkpoint`。

---

## 必须遵守的约束

### 代码约束

| 规则 | 检查方式 |
|------|---------|
| 无 `unwrap()`（测试除外） | gate compile_gate / review |
| 无硬编码 API key / 路径 | review |
| 跨模块引用必须用 `crate::modules::*` | gate compile_gate |
| 所有公开函数必须有 Rust doc 注释 | review |
| 异步代码用 tokio，不用 std::thread | review |
| `Result<T, E>` 错误传播用 `?`，不用 `match` 嵌套 | review |

### 禁止行为

- ❌ 不允许在 acceptance 没有通过的情况下执行 git commit
- ❌ 不允许修改 design_ref 文档（只能读，不能改）
- ❌ 不允许在不理解 design_ref 的情况下开始实现
- ❌ 不允许一次提交多个 slice 的代码
- ❌ 不允许跳过 slice（除非 status: skip，否则必须顺序执行）
- ❌ 不允许使用 `todo!()` 或 `unimplemented!()` 留给以后

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

## Sub-agent Code Review 规范

调用 sub-agent 时，你的 prompt 必须包含：

```
你是 If2Ai 代码审查员。请审查以下代码变更是否满足 review_checklist。

【设计文档片段】
{slice.design_ref 中相关的 interface 定义部分}

【review_checklist】
{slice.review_checklist 的所有条目}

【代码变更（git diff）】
{实际的 diff 内容}

【输出格式】
对每个 checklist 条目，输出：
  PASS: <条目内容>
  或
  FAIL: <条目内容>
       原因：<具体文件:行号和原因>

最后输出整体结论：REVIEW_PASS 或 REVIEW_FAIL
```

只有 sub-agent 输出 `REVIEW_PASS` 时才能继续步骤 9。

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

| 情况 | 行动 |
|------|------|
| gate 失败，修复超过 3 次仍不通过 | 在 exec-plan dashboard.blocked 中记录，停止并等待人工介入 |
| design_ref 有歧义或矛盾 | 选择更保守的解释，在 commit message 中说明 |
| 依赖的 slice 未完成 | 检查前序 slice，如果是 pending 先完成它 |
| 编译错误超出当前 slice 范围 | 只修复与当前 slice 相关的错误，其他的记录到 blocked |

---

## Human Checkpoint 处理

当完成 `meta.human_checkpoints` 中 `after_slice` 指定的 slice 后：

1. 确保所有之前的 slice 都是 `status: done`
2. 在 exec-plan 的 `dashboard.notes` 中写明等待人工 review
3. 停止执行，**不要自动开始下一个 Phase**
4. 输出：`HUMAN_CHECKPOINT_REACHED: Phase X 全部完成，请人工 review 后继续`

---

## Dashboard 更新格式

每次完成一个 slice 后，更新 exec-plan YAML 的 dashboard 区域：

```yaml
dashboard:
  last_updated: "YYYY-MM-DD"
  completed_slices: ["1.1", "1.2", ...]  # 追加刚完成的 slice id
  current_slice: "1.3"                   # 更新为下一个 pending slice
  blocked: []                             # 如有阻塞，在此记录
  notes: "简短描述当前状态"
```

---

## 快速参考

```bash
# 激活 harness 虚拟环境（首次需要：python3 -m venv .venv && .venv/bin/pip install pyyaml）
source .venv/bin/activate

# 验证 exec-plan 格式
python -m harness.runner check-slice \
  --file docs/exec-plans/active/phase-1-foundation.yaml

# 运行单个 slice 的全部 gates
python -m harness.runner run --slice <id> --workspace .

# 查看当前 pending slices
grep -B1 "status: pending" docs/exec-plans/active/phase-1-foundation.yaml

# 生成 git diff 用于 review
git diff --staged
```

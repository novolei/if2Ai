# vNext Runtime Migration Execution Guide

> Agent 执行 vNext 迁移任务的完整操作手册。
> 配套文档：[spec.md](./spec.md) | [design.md](./design.md) | [task.md](./task.md) | [Pack 模板](./pack-template-vnext.md)
>
> 最后更新：2026-04-24

---

## 0. 总览

Agent 执行 vNext 迁移的原子单位是 **Pack**（不是 Task）。三份参考文档（spec/design/task）是蓝图，Pack 是可执行单元。本手册定义从 Task 到完成 commit 的完整闭环。

```
Task (task.md T-XXX)
  → 翻译为 Pack (VNEXT-XXX)
  → Pre-Check (contract snapshot)
  → Build (编码，3 条硬约束)
  → Verify (6 道门：3 base + 3 vNext)
  → Review (逐条核对 Spec)
  → Regression (前序 Phase 测试集)
  → Commit
```

---

## 1. Task → Pack 翻译

### 1.1 自动生成

```bash
./scripts/vnext-task-to-pack T-003          # 生成 Pack stub
./scripts/vnext-task-to-pack T-003 --dry-run # 只打印不写文件
```

输出：`docs/packs/feature/migration-core/VNEXT-T003-<slug>.md`

### 1.2 手工补全

自动生成的 Pack 包含框架和引用，但以下字段需要人工填写：

| 字段 | 来源 | 填写规则 |
|------|------|---------|
| **Goal** | task.md "输出"字段 | 浓缩为 2-3 行行为描述 |
| **Spec** | task.md "验收"字段 | 逐条转为 test name，格式：`<行为> → 测试 <test_name>` |
| **Files** | task.md "关键代码路径" | 列出每个文件并标注 (new/modify) |
| **Reads** | 自动生成 + 补充 | 自动已填 spec/design 引用；需补充只读依赖文件 |
| **Contract 特有约束** | spec.md 对应章节 | 提取本 Task 特有的不变量 |
| **Out of Scope** | task.md + 判断 | 与 task.md 对齐，加上通用排除 |

**关键规则**：Pack 的 `Reads` 必须显式点名 spec.md 和 design.md 的具体章节号，不允许笼统引用。

---

## 2. Pre-Build Contract Check

Agent 开始编码**之前**，必须执行以下只读检查：

### 2.1 依赖 Task 完成确认

```bash
# 检查依赖的 Pack 是否已在 REGISTRY 中标记为 done
grep "VNEXT-T002" docs/packs/REGISTRY.md
```

如果依赖的 Task 未完成 → **STOP**，报告原因，不自行继续。

### 2.2 Contract Snapshot 生成

读取 Pack `Reads` 列出的所有文件，记录关键类型签名：

```
Contract Snapshot:
- CorrelationIds 字段: session_id, project_id, run_id, stream_id, turn_index
- StreamTokenPayload event_type 值集: <列出所有>
- CanonicalRuntimeEvent kind 值集: <列出所有>
- RuntimeProjectionSnapshot 字段: runs, approvals, memory, activation, executionMode
```

### 2.3 兼容性判断

如果 spec 说"需要新增 `attempt_id` 到 `CorrelationIds`"但当前代码中 `CorrelationIds` 没有 `attempt_id` → 这是预期的"要新增"。

如果 spec 说"需要改造 `stream_emitter`"但 `stream_emitter` 的当前接口与 spec 描述完全不同 → 可能前序 Task 未完成或 spec 过时 → **STOP** 并报告。

---

## 3. Build 阶段 3 条硬约束

### 约束 1：Only Touch Pack Files

- 只修改 Pack `## Files` 列出的文件
- 如果发现需要修改列表外的文件 → **STOP** 并报告，不自行扩大范围
- 这防止 scope creep

### 约束 2：Contract 先写后用

如果 Task 涉及新增 event type 或修改数据模型：

1. **先**修改 contract 定义文件（`contracts/common.rs`、`types.ts`）
2. **再**写消费代码（reducer、emitter、UI 组件）

这确保 contract 是"源头"而非"事后补丁"。

### 约束 3：No New Raw Consumer

如果 Task 涉及前端代码：

- 新增的 UI 组件不允许直接订阅 raw Tauri event
- 必须通过 `runtime-projection-bridge` 消费
- 验证方式：`grep -r "listen.*agent-token" src/modules/` 不应新增结果

---

## 4. Verify 阶段 6 道门

### 4.1 3 道 Base 门（已有 `./scripts/pack run`）

| 门 | 检查内容 |
|----|---------|
| B1 | `cargo fmt --check` + `cargo clippy -D warnings` |
| B2 | `cargo test` + `npm test` |
| B3 | `npm run build` |

### 4.2 3 道 vNext 专项门

| 门 | 适用 Task | 检查内容 |
|----|---------|---------|
| V1: Contract Drift | 涉及 contract 变更（T-001/T-010/T-013/T-018） | Rust event type 集合 ⊆ TS translator 映射集 |
| V2: Single Truth | 涉及前端 truth 迁移（T-002~T-005/T-019） | 无新 raw agent-token 消费者；projection store 是唯一来源 |
| V3: Event Log Integrity | 涉及后端 runtime（T-001/T-004/T-006/T-008/T-010/T-011/T-013） | seq 单调；terminal event 正确关闭 run |

vNext 专项门的执行命令：

```bash
# V1: Contract Drift Test
npm test -- --testPathPattern="contract-drift"

# V2: Single Truth Test
npm test -- --testPathPattern="single-truth"
# 或手动检查：
grep -r "listen.*agent-token" src/modules/ | grep -v node_modules | grep -v ".test."

# V3: Event Log Integrity
cargo test --manifest-path src-tauri/Cargo.toml -- event_log
```

### 4.3 门与 Task 的适用矩阵

| Task | B1-B3 | V1 | V2 | V3 |
|------|-------|----|----|-----|
| T-001 Contract 统一 | ✅ | ✅ | — | ✅ |
| T-002 Projection Truth | ✅ | — | ✅ | — |
| T-003 Chat Cutover | ✅ | — | ✅ | — |
| T-004 session.json 拆分 | ✅ | — | — | ✅ |
| T-005 Single Truth | ✅ | — | ✅ | — |
| T-006 Supervisor | ✅ | — | — | ✅ |
| T-007 Supervisor 前端 | ✅ | — | — | — |
| T-008 stream_task 分解 | ✅ | — | — | ✅ |
| T-009 Command Thinning | ✅ | — | — | — |
| T-010 Tool Contract | ✅ | ✅ | — | ✅ |
| T-011 Resume Contract | ✅ | — | — | ✅ |
| T-012 Resume 前端 | ✅ | — | — | — |
| T-013 Attempt Ledger | ✅ | ✅ | — | ✅ |
| T-014 Attempt 前端 | ✅ | — | — | — |
| T-015 Run Report | ✅ | — | — | ✅ |
| T-016 Harness Cutover | ✅ | — | — | — |
| T-017 Harness Eval | ✅ | — | — | — |
| T-018 Drift Guardrails | ✅ | ✅ | — | — |
| T-019 Memory UI | ✅ | — | ✅ | — |
| T-020 Checkpoint 冷启动 | ✅ | — | ✅ | — |

---

## 5. Review 阶段

Code-reviewer 逐条核对：

1. Pack `## Spec` 的每个 test name 是否有对应测试实现
2. spec 定义的接口签名与代码实现是否一致
3. design 的状态机/迁移步骤与代码路径是否一致
4. Pack `## Contract` 的每条约束是否被遵守
5. `## Out of Scope` 列出的文件是否确实未被修改

---

## 6. Regression 阶段

每个 Task 完成后，必须跑**当前 Phase 的回归套件**：

```bash
# Phase A (T-001~T-005 完成后)
./scripts/pack suite harness/suites/vnext_phase_a_regression.yaml

# Phase B (T-006~T-009 完成后)
./scripts/pack suite harness/suites/vnext_phase_b_regression.yaml

# Phase C (T-010~T-014 完成后)
./scripts/pack suite harness/suites/vnext_phase_c_regression.yaml

# Phase D (T-015~T-020 完成后)
./scripts/pack suite harness/suites/vnext_phase_d_regression.yaml
```

每个 Phase 的回归套件包含**前序 Phase 的全部回归**（递进式），确保不退化。

---

## 7. Commit 规范

```
<type>(VNEXT-XXX): <简短描述>

<可选详细说明>

Task: T-XXX
Spec: §X.X <标题>
Verify: PASS (B1-B3 + V1/V2/V3)
Review: PASS
Regression: <Phase>_regression PASS
```

`type` ∈ `feat | fix | refactor | test | chore | perf | docs`。

---

## 8. 失败处理

| 失败类型 | 处理方式 |
|---------|---------|
| Base 门 B1-B3 FAIL | 修代码，重跑 `./scripts/pack run VNEXT-XXX` |
| vNext 门 V1-V3 FAIL | 按 fix_prompt 修代码，重跑 |
| 同一 Pack FAIL 3 次 | STOP，在 Pack 内追加 `## Blocked` 段说明，等人工 |
| Regression FAIL | 定位退化来源，修代码，重跑当前 Phase regression |
| Pre-check 发现依赖未完成 | STOP，报告，等待依赖 Task 完成 |

---

## 9. 执行流程速查

```
对于每个 Task T-XXX：

1. TRANSLATE
   $ ./scripts/vnext-task-to-pack T-XXX
   $ 手工补全 Pack 的 Goal/Spec/Files/Contract/Out of Scope

2. PRE-CHECK
   $ 确认依赖 Task 已完成
   $ 生成 Contract Snapshot
   $ 判断兼容性

3. BUILD
   $ 遵守 Only Touch Pack Files
   $ Contract 先写后用
   $ No New Raw Consumer

4. VERIFY (6 道门)
   $ ./scripts/pack run VNEXT-XXX
   $ + vNext 专项门 (V1/V2/V3，按适用矩阵)

5. REVIEW
   $ 逐条核对 Spec → test 实现
   $ 逐条核对 Contract → 代码遵守
   $ 确认 Out of Scope 文件未被修改

6. REGRESSION
   $ ./scripts/pack suite harness/suites/vnext_phase_<X>_regression.yaml

7. COMMIT
   $ type(VNEXT-XXX): 描述
    Task: T-XXX
    Verify: PASS
    Review: PASS
    Regression: PASS
```

---

## 10. 产物清单

| 产物 | 路径 | 用途 |
|------|------|------|
| 规格文档 | `docs/vnext_new/spec.md` | 定义"做什么" |
| 设计文档 | `docs/vnext_new/design.md` | 定义"怎么做" |
| 任务文档 | `docs/vnext_new/task.md` | 定义"谁做什么" |
| Pack 模板 | `docs/vnext_new/pack-template-vnext.md` | vNext 迁移专用 Pack 模板 |
| 执行手册 | `docs/vnext_new/execution-guide.md` | 本文档 |
| 翻译脚本 | `scripts/vnext-task-to-pack` | Task → Pack 自动生成 |
| Phase A 回归 | `harness/suites/vnext_phase_a_regression.yaml` | Phase A 完成后全跑 |
| Phase B 回归 | `harness/suites/vnext_phase_b_regression.yaml` | Phase B 完成后全跑 |
| Phase C 回归 | `harness/suites/vnext_phase_c_regression.yaml` | Phase C 完成后全跑 |
| Phase D 回归 | `harness/suites/vnext_phase_d_regression.yaml` | Phase D 完成后全跑 |

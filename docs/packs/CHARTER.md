# Pack Charter

> If2Ai 唯一的开发流程真相。Agent 每次执行任何 pack 都必须先读这份文件。
>
> 最后更新: 2026-04-21
> 取代：`docs/_legacy/refactor-v1/CHARTER.md`、`docs/_legacy/exec-plans/`、`docs/_legacy/implementation-packs/README.md`

---

## 1. 唯一流水线（5 步）

```
① PACK   人写 1 个 ≤ 60 行的 Pack 文件（设计+规格+计划合一）
② BUILD  Agent 只读 1 个 Pack + Pack 中点名的现有代码
③④ RUN  ./scripts/pack run <PACK-ID>   ⭐ 一条命令完成：
            1. lint-architecture（CHARTER §1 / §5 不变量）
            2. snapshot before（refactor 自动 idempotent）
            3. verify（snapshot diff + cargo build/test/clippy）
            4. 打印 code-reviewer subagent 调用 prompt + commit msg 模板
         FAIL 时自动打印"agent feedback block"含 evidence + fix_prompt
⑤ COMMIT 1 PR、1 commit、更新 REGISTRY 状态
```

**整个循环必须在一个 agent 会话内完成**。失败 3 次自动 STOP 并报告原因。

**OpenAI 启发的关键设计**：每条 FAIL 都嵌入 `fix_prompt` 字段（"该怎么修"），让 agent 直接照做不必"理解错误"。这是 Ralph Wiggum loop 的实现基础。

---

## 2. Pack 类型

### 2.1 Refactor Pack（`docs/packs/refactor/GFR-XXX-*.md`）

零业务变更。snapshot diff 是核心验证。模板见 `GFR-001-extract-real-api-client.md`。

### 2.2 Feature Pack（`docs/packs/feature/FEAT-XXX-*.md`）

有业务变更。必须自带至少 1 条新 cargo test 或 e2e 行为。模板见本文件 §6。

> 现有 `CPD-001-turn-spine.md` 等遗留 pack 视同 FEAT，按新流程消费。

---

## 3. Hard Rules（agent 启动必读）

### 3.1 通用
- ❌ 禁止漫读 `docs/design-docs/` / `docs/product-specs/` / `docs/staff-remediation/`；只在 Pack 明确点名时按路径读
- ❌ 禁止读 `docs/_legacy/` 下任何文件
- ❌ 禁止在同一 PR 里做两个 Pack
- ❌ 禁止改 Pack 文件本身（人写、人改；agent 只读）
- ❌ 禁止使用 `unwrap()` / `expect()` / `todo!()` / `unimplemented!()` 在非测试代码
- ❌ 禁止跨模块 `use crate::xxx`，必须 `use crate::modules::*`
- ❌ 禁止引入 Pack 没声明的新 dependency
- ✅ 允许：在 Pack `## Files (scope)` 列出的文件内做任意结构变化（前提满足 Spec / Contract）

### 3.2 Refactor 专属（GFR-）
- ❌ 禁止改任何函数体（仅允许调整 `use` 路径与 `pub(crate)` 可见性以编译通过）
- ❌ 禁止删测试 / 改测试名 / 改 assertion
- ❌ 禁止重命名 pub 符号
- ✅ 允许：移动代码、新建文件、加 `pub use` shim、调整 `use` 导入

### 3.3 Feature 专属（FEAT- / CPD-）
- ✅ **必须** 至少新增 1 条 cargo test（或 e2e）覆盖 Pack `## Spec` 中的每条行为
- ✅ **必须** review 时让 code-reviewer 逐条核对 Spec 是否被实现
- ❌ 禁止顺手改 Pack `## Out of Scope` 列出的代码

---

## 4. 不变量（refactor pack 由 `scripts/pack verify` 检查）

| ID     | 不变量                                                           | 仅适用   |
| ------ | ---------------------------------------------------------------- | -------- |
| **I1** | 被搬代码的 `pub` 符号集合不增不减不改签名（白名单除外）          | refactor |
| **I2** | 测试名称集合不减；pass/fail 不退化                               | refactor |
| **I3** | 事件名 / IPC 命令名 / 事件字面量集合不变                         | 通用     |
| **I4** | 持久化 key 名（localStorage / sqlite column / config field）不变 | 通用     |
| **I5** | god-file 留 `pub use` shim；调用方 import 路径在本 pack 内不修改 | refactor |
| **I6** | 函数体一行不改（除 `use` 路径修正）                              | refactor |
| **I7** | `cargo fmt --check` + `cargo clippy -D warnings` 通过            | 通用     |

---

## 5. 文件大小目标

| 类型             | 目标     | 上限   |
| ---------------- | -------- | ------ |
| 后端 module 文件 | ≤ 500 行 | 800 行 |
| 前端组件 `.tsx`  | ≤ 300 行 | 500 行 |
| 前端 hook 文件   | ≤ 200 行 | 400 行 |
| Pack 文档        | ≤ 60 行  | 100 行 |

---

## 6. Feature Pack 模板（4 种变种）

所有 feature pack 都落 `docs/packs/feature/`。命名前缀决定 code-reviewer 的额外检查分支。

### 6.1 `FEAT-` — 新功能 / 升级

```markdown
# FEAT-XXX: <one-line goal>

## Status
- State: active

## Goal
<2–3 行：要什么行为存在>

## Spec (verifiable — 每条配 1 个 test name)
- <行为 1> → 测试 `tests::<module>::<test_name>`
- <行为 2> → 测试 `tests::<module>::<test_name>`
- <UI 行为>  → e2e step in `e2e/<file>.spec.ts`

## Files (scope — write list)
- src-tauri/src/.../foo.rs        (new)
- src-tauri/src/.../bar.rs        (modify)
- src-tauri/tests/foo_tests.rs    (new)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/.../existing_trait.rs
- docs/design-docs/<topic>.md §章节   ← 仅当确实必需

## Contract (review must check)
- 不改 IPC 命令名（I3）
- 不改 sqlite schema（I4）
- 不引入新 dependency

## Out of Scope
- ❌ 不顺手重构相邻代码
- ❌ 不改 UI 样式

## Verify
- ./scripts/pack run FEAT-XXX

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
```

### 6.2 `BUG-` — 修 bug（特殊要求：先写复现测试）

在 §6.1 基础上：

- `## Spec` 中**第一条**必须是 `regression: ...` 形式的复现测试，描述**触发 bug 的最小步骤**
- 该复现测试在 commit 历史中**应当先以 FAIL 出现**（修代码前的 commit），然后修复后 PASS
- 即：本 pack 至少包含 2 个 commit：`test(BUG-XXX): add failing regression test` + `fix(BUG-XXX): <one-line>`
- code-reviewer 会检查 git log 时间线，未发现 "test 先于 fix" 的痕迹 → REVIEW_FAIL
- `## Goal` 段必须包含一行 `Symptom:` 描述用户看到的现象

### 6.3 `PERF-` — 性能优化（特殊要求：before/after 数据）

在 §6.1 基础上：

- `## Spec` 中**至少**包含 1 条性能不变性测试（行为不能退化）+ 1 个 benchmark / measured timing
- `## Goal` 段必须包含一行 `Baseline:`（优化前的测量值）+ `Target:`（优化后的目标值）
- commit message **必须**含 `Before: <metric>` 和 `After: <metric>` 两行
- code-reviewer 会 grep commit message，缺失任一行 → REVIEW_FAIL
- `## Out of Scope` 必须显式："不引入新行为，不改 API"

### 6.4 `DEP-` — 升级依赖（特殊要求：diff 限定）

在 §6.1 基础上：

- `## Goal` 段必须列出 `From: <version>` + `To: <version>`
- `## Files (scope)` **只允许** lockfile / 包管理 manifest（`Cargo.toml`、`Cargo.lock`、`package.json`、`pnpm-lock.yaml`、`bun.lockb` 等）
- `## Spec` 必须有 1 条："所有现有测试通过 → `cargo test --workspace`"
- 若升级触发**任何**业务代码改动 → 必须拆为两个 pack：`DEP-XXX`（升版本）+ `FEAT-YYY`（适配新 API）
- code-reviewer 检查 diff 的所有非 lockfile 文件 → 任意业务代码改动 → REVIEW_FAIL

---

## 7. 与 legacy 的关系

- `docs/_legacy/exec-plans/` — M0~M5 的 YAML 已冻结，**不再消费**。如需历史决策依据，人手翻阅；agent 禁读。
- `docs/_legacy/implementation-packs/` — 老 ACTIVE/README/TEMPLATE 已冻结，模板被本 CHARTER §6 取代。
- `docs/_legacy/refactor-v1/` — 老 CHARTER/REGISTRY 已被本文件取代。
- `harness/` — 仅 `harness/suites/*.yaml` + `harness run --suite` 保留为可选集成测试；`diff-gate` / `promote` / `check-slice` / `review` 已废弃。
- `docs/design-docs/` / `docs/product-specs/` — 保留作参考库，**不再启动必读**；Pack 中可点名引用具体章节。

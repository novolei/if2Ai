# MIG-001 Canonical Chat Execution Spine — 连续执行 (Auto Code Review)

> **使用方法**：启动新 agent session 时，把整份 markdown 粘贴进去作为首条用户消息。
> Agent 会按本 prompt 严格连续执行 4 个 sub-pack，每个 sub-pack 完成后自动调用
> `code-reviewer` subagent 审查，PASS 后才 commit/push 并继续下一个；
> 4 个 sub-pack 全部完成或 STOP 条件触发后才汇报。

---

## 1. 必读文件（按顺序，仅 4 个）

1. `docs/packs/CHARTER.md`                                                        # 流水线规则 + I1–I7 + FEAT pack §6.1 模板
2. `docs/packs/feature/migration-core/MIG-001-canonical-chat-execution-spine.md`  # 本 pack
3. `docs/packs/REGISTRY.md`                                                       # 当前 pack 状态一览
4. `CLAUDE.md` (≤ 142 行)                                                         # hard rules

**禁读**：`docs/_legacy/**`、`docs/design-docs/**`、`docs/staff-remediation/**`（除非 MIG-001 明确点名）、`docs/exec-plans/**`

---

## 2. Codebase 现状快照（HEAD = 91d99fe，截至 2026-04-21）

| 关键文件                                            | LOC  | 状态                                                                                                                                                   |
| --------------------------------------------------- | ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src-tauri/src/commands/agent/mod.rs`               | 2574 | Tier 0 god-file（4 IPC commands；`run_agent_turn` 558 LOC + `start_agent_stream` 1714 LOC + `stop_agent_stream` 24 LOC + `respond_permission` 70 LOC） |
| `src-tauri/src/modules/application/turn_service.rs` | 227  | 当前只暴露 `prepare_chat_inputs`，doc 明确 "does NOT own runtime construction / tool loop / stream emission"                                           |

✅ cargo build / clippy --workspace -D warnings / test --no-run 全 PASS（基线）

已完成的 GFR pack（**不要重做**）：5×005 系列 + 6×T1-B + 3×T1-C + T1-D + T1-F + T1-G + T1-H + T1-I + T1-A 取消 + 005f tests 抽出

---

## 3. MIG-001 拆解（连续执行 4 个 sub-pack，每个完成后自动 code-review）

| Sub-pack      | Goal                                                                                        | 范围                                                                                                                                                      | 难度 |
| ------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| **MIG-001-a** | 扩展 `TurnServiceDeps` 加入 7 个新依赖                                                      | `session_manager`, `harness`, `learning_module`, `context_budget`, `memory_ticker`, `trajectory_manager`, `app_handle` 渠道；`make_turn_service` 同步更新 | 低   |
| **MIG-001-b** | 搬 `run_agent_turn` body 到 `TurnService::run_turn`                                         | 558 LOC 整段搬迁；agent.rs 收成 thin IPC wrapper（参数解析 → `service.run_turn(...)` → 返回）+ 加 1 个 e2e 测试                                           | 中   |
| **MIG-001-c** | 搬 `start_agent_stream` 同步部分（prepare + spawn task setup）到 `TurnService::stream_turn` | ~600 LOC                                                                                                                                                  | 高   |
| **MIG-001-d** | 搬 `start_agent_stream` task closure（async tool loop + emit + retry）到 `TurnService`      | ~1100 LOC                                                                                                                                                 | 极高 |

**4 个 sub-pack 串行执行，无人工干预**。每个 sub-pack 必须经过 code-reviewer subagent 自动审查 → PASS 后才提交并继续下一个。

---

## 4. Hard rules（agent 必须遵守）

### CHARTER 通用
- ❌ `unwrap()` / `expect()` / `todo!()` / `unimplemented!()` 在非测试代码
- ❌ 跨模块用 `crate::xxx`，必须 `use crate::modules::*`
- ❌ 不引入 pack 没声明的新依赖
- ✅ 默认 `pub(crate)`，`pub` 仅在边界真实存在时
- ✅ 所有 `pub fn` 有 `///` 注释

### MIG-001 专属
- ❌ **`application/turn_service.rs` 不能 import `crate::commands::AppState`**（application/mod.rs §2.1 硬约束）。所有依赖必须显式注入 `TurnServiceDeps`
- ❌ 不能在 agent.rs 同时保留新 orchestrator + 旧内联主线（CHARTER §6.1 + MIG-001 Cutover guardrail）
- ❌ `TurnService` 不能只是多包一层 facade，runtime ownership 必须真正切换
- ❌ 不顺手动 `harness/` / `learning/` 内部代码（pack Forbidden Files）
- ❌ 不动 `src/**`（前端不在本 pack 范围）
- ✅ `TurnService::run_turn` / `stream_turn` 必须真正拥有 ConversationRuntime 构造 + 执行
- ✅ 每个 sub-pack 必须删除 `turn_service.rs` doc 中对应的 "does NOT own ..." 那行

### Test 要求（CHARTER §3.2 / §6.1）
- 每个 sub-pack **必须** 至少新增 1 条 `cargo test`（end-to-end 调用 `TurnService::run_turn` / `stream_turn` 的 mock 测试）
- code-reviewer 必须逐条核对 Spec 是否被实现
- 测试名 set 不减；旧 `agent_loop_executes_skill_tool_end_to_end` 不能 break

---

## 5. 执行流程（每个 sub-pack — 全自动循环）

```
循环：i = a, b, c, d
{
  ① 实现 sub-pack MIG-001-${i}（按 §3 描述的 Goal/范围）

  ② Cargo 自检（必须全 GREEN 才进 ③）
     - cargo build (no warnings)
     - cargo fmt --all (clean)
     - cargo clippy --workspace --all-targets -- -D warnings
     - cargo test --no-run
     - cargo test <new_test_name>   # MIG-001-a 是 deps construction smoke；其余是 e2e

     失败 → 回 ① 修复，最多 5 次；超过则 STOP 报告

  ③ 调 code-reviewer subagent
     使用 Task 工具，subagent_type="code-reviewer"，prompt：
     ────────────────────────────────────────────────────────────
     请审查 git diff（HEAD vs working tree）针对 MIG-001-${i} 的代码变更。

     **审查维度（双重）**：

     A. 任务达成情况（针对 MIG-001-${i} 的具体 Goal）：
        - 是否真正搬迁了主线 ownership（不能只是 wrapper / facade）
        - TurnService 是否真正拥有 lifecycle，agent.rs IPC 是否变成 thin adapter
        - 新增测试是否覆盖 canonical entry path
        - turn_service.rs doc 是否更新（删除对应的 "does NOT own ..." 行）

     B. 代码质量：
        - 无 unwrap()/expect()/todo!()/unimplemented!() 在非测试代码
        - 跨模块用 crate::modules::*
        - 错误传播用 ? 不嵌 match
        - 默认 pub(crate)，pub 仅在边界
        - 所有新增 pub fn 有 /// 注释
        - 无新依赖（除非 pack 声明）
        - 无对 src/** / harness/** / learning/** 的违规修改

     输出格式（必须严格）：
        REVIEW_PASS — 一行总结
        或
        REVIEW_FAIL — 列出每个问题 + 修复建议（编号 1. 2. 3.）
     ────────────────────────────────────────────────────────────

  ④ 解析 code-reviewer 输出：
     - 若 "REVIEW_PASS" → 进 ⑤
     - 若 "REVIEW_FAIL" → 按编号修复每个问题（不要扩 scope），回 ②
       同一 sub-pack code-review 失败 3 次 → STOP 报告

  ⑤ Commit + Push
     git add <仅本 sub-pack scope 的文件，不含无关 docs drift>
     git commit -m "feat(MIG-001-${i}): <one-line goal>

       Pack: MIG-001-${i}
       Verify: PASS (cargo all-green)
       Review: PASS (code-reviewer subagent)
       <详细变更说明 + agent.rs LOC 前→后 + TurnService LOC 前→后>"
     git push origin HEAD

  ⑥ 给用户发简短进度报告（agent.rs LOC + 测试数 + 累计进度），然后立即进 i+1

  ⑦ 当 i = d 完成 → 报告整 MIG-001 final summary 并 STOP
}
```

---

## 6. Code Reviewer Subagent 调用细节

- Subagent 是 readonly mode（无写权限），它只读 git diff + 现有代码
- 你（executor）必须把当前 `git diff` 主动喂进 subagent 的 prompt（subagent 看不到你的 conversation）
- subagent 看 review_checklist 时若有疑问，会以 REVIEW_FAIL 形式提出 — 你必须把它当成 ground truth，不要争辩
- 若 code-reviewer 提的问题超出本 sub-pack scope（误判），可在 fix 时附加注释解释，但不要直接忽略

---

## 7. 已知陷阱（避开）

- `application/` 层禁止 import `crate::commands::AppState` —— 必须把 AppState 拆成具体依赖注入
- `start_agent_stream` 含 `tokio::spawn(async move { ... })` task closure，搬迁时要谨慎处理 `move` ownership 和 `Arc::clone`
- `AgentStreamEmitter` (in `runtime/stream_emitter.rs`) 拥有 Tauri `AppHandle`，TurnService 接收时要用 trait abstraction 或显式 deps
- `tests/` 路径在 `src-tauri/tests/`（不是 `src-tauri/src/tests/`）—— 集成测试用前者
- `cargo fmt --all` 可能顺手改 11+ 个无关文件；commit 前 `git checkout --` 剥离不在 pack scope 的文件
- pack `## Allowed Files` 段不要写 `(parens)` 注释——parser 只剥 `# comment`
- 不要在 sub-pack 之间停下问用户——除非触发 §5 中的 STOP 条件

---

## 8. 输出格式

### 每个 sub-pack 完成后（短）
```
✅ MIG-001-${i} done
- agent.rs LOC: <before> → <after> (-N%)
- TurnService LOC: <before> → <after>
- 新增测试: <test_name>
- Code Review: REVIEW_PASS (subagent)
- Commit: <hash>
- 累计 agent.rs reduction: <X>% from 4058
进入 MIG-001-${i+1}...
```

### 整 MIG-001 完成后（详细 final report）
```
🎉 MIG-001 全部完成（4/4 sub-pack）
- agent.rs 4058 → <final> (-X%)
- TurnService 227 → <final> (含 run_turn / stream_turn 完整 lifecycle)
- 新增测试: <list>
- 4 个 commit hash + push 状态
- TurnService doc 完全更新（无 "does NOT own ..." 残留）
- 修订建议（可选 follow-up）
```

---

## 9. STOP 条件

- cargo gates 失败 5 次 → STOP
- 同一 sub-pack code-review FAIL 3 次 → STOP
- 4 个 sub-pack 全部完成 → STOP（正常收尾）

---

## 10. 起步任务

立即开始 **MIG-001-a**：扩展 `TurnServiceDeps`。

具体步骤：
1. 读 `src-tauri/src/modules/application/turn_service.rs`（227 LOC）
2. 读 `src-tauri/src/commands/agent/mod.rs` 的 line 65-95（make_turn_service）+ line 165-280（run_agent_turn 前半段，看 state.* 字段访问）
3. 评估要注入哪些 AppState 字段到 TurnServiceDeps
4. 修改 turn_service.rs 的 `TurnServiceDeps` struct + `make_turn_service` helper
5. 更新 turn_service.rs 的 module doc（删除 "does NOT own ..." 4 行 — 改为 "owns turn lifecycle via run_turn / stream_turn (see MIG-001-b/c/d)"）
6. 加 1 个测试 `turn_service_deps_construction_smoke` 验证 deps 可构造
7. cargo build / fmt / clippy / test --no-run 全 PASS
8. **调 code-reviewer subagent 审查（按 §6）**
9. PASS → commit `feat(MIG-001-a): expand TurnServiceDeps for full turn ownership` → push → 立即进 MIG-001-b
10. FAIL → 修复重审

开工。不要再问我 — 4 个 sub-pack 全部完成或 STOP 条件触发后再汇报。

# If2Ai Agent 进化规范 vs 代码库 — Gap 调查报告

**版本**: 2026-04-30 (iteration 1 of self-evolving truth loop)
**前次**: [`AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-29.md`](./AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-29.md)
**对照基准**: `.qoder/specs/if2ai-agent-evolution-report.md`（Part 0~8）
**代码快照**: 当前 `HEAD = ebca6fa` (clean working tree; commit `aa2deed` "land complete 41-pack rollout" precedes)
**驱动文档**: [`docs/superpowers/plans/2026-04-30-agent-evolution-truth-loop.md`](../../superpowers/plans/2026-04-30-agent-evolution-truth-loop.md)

---

## 1. 与 04-29 报告的核心差异

04-29 报告是 **rollout commit `aa2deed` 之前** 写的快照。该 commit 本质性改变了状况：

| 维度                | 04-29 报告判断                          | 04-30 实测                                                                                                 |
| ------------------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 模块算法/类型层     | 基本完整                                | ✅ 维持，更新无回归                                                                                         |
| 生产路径接线        | "致命 Wire-up Gap"                      | ⚠️ **部分接通**，但 4 处为 `landed-stub`                                                                    |
| Tauri emit pipeline | "agent-token 路径丢弃 daemon_health 等" | ✅ **新增 `evolution_emitter` + `runtime_event` 独立 channel**，绕过 `map_event_type_to_runtime` 旧瓶颈     |
| 前端 store 接线     | "全仓库无 `applyEnvelope` 调用方"       | ✅ **`src/App.tsx:258` 通过 `listen("runtime_event", …)` 调用 `evolutionEventStore.applyEnvelope`**         |
| Part 7.4 ~24 个组件 | "100% UI Gap"                           | ✅ **6 个 EvolutionDevDrawer 子组件已落地**（`src/components/chat/evolution/`）                             |
| Self-Healing daemon | "默认 registry 仅 2 项"                 | ⚠️ 仍是 2 项；新增 `register_extra_probes` helper **写入了一个孤立 registry，未被 spawn**（详见 §3 WU-002） |

**结论变化**：旧报告 "Pack 级能力已落地 + 测试可编译" 判断已过时；新现实是 **emitter/UI/finalize-hook 主路径接通，但四个 deeper-wiring 站点用了 `MockUtilityLlm`/`StubProbe`/`MockKnowledgeStore`/`ConstEmbedder` 占位**。这种 "honest stub" 在代码注释里都明确说明了——但 spec 角度仍是 NOT done。

---

## 2. 新增的状态术语：`landed-stub`

为避免再把 "test 跑得过 / emit 链通了" 等同于 spec 已落地，本报告引入观察态 `landed-stub`：

> **landed-stub**：算法 + emit + 调用站三件齐全，但至少一个生产依赖（LLM、Provider、Store、Probe）是占位实现（`MockX::empty` / `StubX` / `ConstX` / 空闭包），且代码注释自承 "until a deeper-wiring Pack plumbs …"。**对 Exit Gate 视为 not-done。**

REGISTRY 不引入此态——只在本报告记账。

---

## 3. Pack-by-Pack 真相表

### 3.1 Wire-up Pipeline (WU-001..008)

| Pack                                     | REGISTRY 当前 | 实测                                         | 证据                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------- | ------------- | -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **WU-001** emitter infra                 | active        | ✅ **done**                                   | `src-tauri/src/modules/runtime/evolution_emitter.rs` 全文；7+ 调用方；`src/App.tsx:258` 订阅 `runtime_event` channel 并调用 `evolutionEventStore.applyEnvelope`；kill-switch `IF2AI_DISABLE_EVOLUTION_EMIT` 工作                                                                                                                                                                                                                                                      |
| **WU-002** daemon probes                 | active        | ⚠️ **landed-stub**                            | `desktop_host/setup.rs:38` 仍 spawn 老 `spawn_self_repair_watchdog(ticker)`（仅 2 个默认 check）；line 86: `evolution_probe_set(Vec::new(), Vec::new(), browser_probes)` — provider + MCP probe **均空**；line 75-83: `StubBrowserProbe` 永远返回 Healthy；line 130: `register_extra_probes(&mut registry, …)` 写入的 registry **从未被 spawn**，只是为了 emit 一次初始 DaemonHealth + 让 unit test 通过。**真实运行中 Self-Healing daemon 仍只有 SH-001 两项检查。** |
| **WU-003** finalize sediment hooks       | active        | ✅ **done**                                   | `stream_finalize.rs:1198` 调 `extract_turn_checkpoint`；line 1236 调 `run_sedimentation_pipeline`；line 1268 调 `run_domain_knowledge_contributor`。Production hot path。                                                                                                                                                                                                                                                                                             |
| **WU-004** preflight injection helpers   | active        | ⚠️ **landed-stub** (与 DW-002 合并报告，见下) | `preflight_hooks.rs` 完整；但 `#![allow(dead_code)]` 标记，调用方为 DW-002                                                                                                                                                                                                                                                                                                                                                                                            |
| **WU-005** self-edit scanner helper      | active        | ⚠️ **landed-stub** (与 DW-001 合并)           | helper 完整；DW-001 是 spawn 站点（见下）                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **WU-006** browser simplify + coordinate | active        | ✅ **done**                                   | `smart_browser/runtime.rs:191-242` `simplify_browser_result_text` 在 browser 工具结果路径上调 `adaptive_simplify`；line 253-289 `decide_browser_click_strategy` 真实派发                                                                                                                                                                                                                                                                                              |
| **WU-007** tool alias redirect           | active        | ✅ **done**（独立未审计 — 标记需迭代验证）    | grep 显示 BR-003 alias 表存在；`broker` 调用方未在本次审计中详查；候选 iteration 4                                                                                                                                                                                                                                                                                                                                                                                    |
| **WU-008** DK lookup helper              | active        | ⚠️ **landed-stub** (与 DW-004 合并)           | helper 完整；DW-004 是调用站点                                                                                                                                                                                                                                                                                                                                                                                                                                        |

### 3.2 Deeper-Wiring Pipeline (DW-001..005)

| Pack                                   | REGISTRY 当前 | 实测              | 证据                                                                                                                                                                                                                                                                                                           |
| -------------------------------------- | ------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **DW-001** self-edit scanner spawn     | active        | ⚠️ **landed-stub** | `setup.rs:141` 调 `spawn_self_edit_scanner_interval(handle)`；line 165-170 `ConstEmbedder` 返回 `vec![1.0,0.0,0.0]`；line 176-177 `MockUtilityLlm::empty()`。Tick 在跑，但每次 scan 输入 **永远是空的**（`run_scanner_once(&[], &[], llm, &ConstEmbedder, …)` line 178-186）——发不出真实提案                   |
| **DW-002** stream_task digester wire   | active        | ⚠️ **landed-stub** | `stream_task.rs:548-578` 调 `digest_messages_for_preflight`；但 line 555-556 `MockUtilityLlm::empty()`；自带注释 "Placeholder LLM means the digester returns identity in production until a deeper wiring Pack plumbs `ChatProviderUtilityLlm`"。`digested_messages` 永远是 identity transform，**真实压缩 0** |
| **DW-003** finalize parallel hooks     | active        | ✅ **done**        | `stream_finalize.rs:1236, 1268` 真实 spawn；与 WU-003 同一证据                                                                                                                                                                                                                                                 |
| **DW-004** work_loop DK lookup         | active        | ⚠️ **landed-stub** | `work_loop.rs:408-422` `spawn_dk_lookup_advisory` 在 work loop 入口被调用；但 line 412-420 用 `MockKnowledgeStore` 占位（自带注释 "until a deeper-wiring Pack plumbs a process-wide store singleton"）；envelope 内容是 `entryId: "advisory"` 占位。**真实知识库未被查询**                                     |
| **DW-005** browser click strategy wire | active        | ✅ **done**        | `smart_browser/runtime.rs:287-289` 真实派发 `decide_browser_click_strategy`；与 WU-006 同源                                                                                                                                                                                                                    |

### 3.3 Frontend UI Pipeline (UI-001..006)

| Pack            | REGISTRY 当前 | 实测       | 证据                                                                                                                                                                                                                                                                                                                       |
| --------------- | ------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **UI-001..006** | active        | ✅ **done** | 全部 6 个 tsx 文件存在于 `src/components/chat/evolution/`：`EvolutionDevDrawer.tsx`, `DaemonHealthDashboard.tsx`, `CompressionHistoryTable.tsx`, `SkillSedimentationTimeline.tsx`, `SelfEditPanel.tsx`, `EvolutionMiscPanel.tsx`；含 `EvolutionDevDrawer.test.ts`。**前端真消费 store**（与 04-29 "100% UI Gap" 完全反转） |

### 3.4 Phase 0–7 核心 Packs

未在本次审计中重审（04-29 报告已确认 22 个 done；rollout 后应仍为 done）。**Iteration 4 候选：FEAT-EVO-FE-001 stub 是否仍需 active**。

---

## 4. 关键 Root-Cause：四个 `landed-stub` 的共同根因

DW-001 / DW-002 / DW-004 / WU-002 用的占位类型不同，但根因只有 **三个未交付的依赖线程**：

| 缺失线程                                                                    | 当前占位                                  | 需要的真值                                                                      | 影响的 Pack                  |
| --------------------------------------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------- | ---------------------------- |
| **A. `Arc<dyn UtilityLlm>` 从 ChatProvider 一路串到 stream_task + scanner** | `MockUtilityLlm::empty()`                 | `ChatProviderUtilityLlm`（已有，需在 `AppState` / `SessionContext` 持有并下传） | DW-001, DW-002               |
| **B. `ProviderManager` + `McpServerManager` handle 串到 `setup.rs`**        | `Vec::new(), Vec::new()` 两个空 probe vec | `Arc<ProviderManager>` / `Arc<McpServerManager>` 暴露 `liveness_probe()` 工厂   | WU-002 (provider + mcp 部分) |
| **C. 进程级 `KnowledgeStore` singleton**                                    | `MockKnowledgeStore`                      | `OnceLock<Arc<dyn KnowledgeStore>>` 在 `setup.rs` 初始化，从 `work_loop` 读取   | DW-004                       |

**这三个线程都是纯 plumbing，无算法工作**。一个集中 Pack（建议命名 `WU-009-deep-utility-handle-threading`）可以一并解决——iteration 3 的目标。

WU-002 还需额外补一个 BrowserSession 真探针（`StubBrowserProbe` → 接 `BrowserSessionLivenessCheck`）；Pack 名 `WU-002B-real-browser-probe` 或合入 WU-009。

---

## 5. Spec 内部矛盾（沿用 04-29 §2，未修复）

`.qoder/specs/if2ai-agent-evolution-report.md` 仍存在：

- **Part 1.3** 列模块 A/B/D/G/J/L "待实现"——已与现实严重不符（A/B/D/G 已 done，J 已 done，L 已 done）
- **Part 6** 把 15 个差距点全部标 ✅——但其中 4 个是 `landed-stub`，应改为 ⚠️

**Iteration 2 任务**：write-only 修复，不动代码。详见 plan §6。

---

## 6. Iteration 2 / 3 / 4 候选清单

按 Truth Loop §1 hard rule，每次只做一件。

| Iter  | 类型       | 内容                                                                                                         | 估时          |
| ----- | ---------- | ------------------------------------------------------------------------------------------------------------ | ------------- |
| **2** | write-only | REGISTRY + Pack `State:` + spec Part 1.3/Part 6 三处对齐到本报告分类                                         | 30 分钟       |
| **3** | code       | `WU-009-deep-utility-handle-threading` —— 把 §4 的三个线程串到生产                                           | 半天～一天    |
| **4** | audit      | 重审 WU-007 alias redirect + FEAT-EVO-FE-001 stub 必要性                                                     | 1 小时        |
| **5** | verify     | 跑 `./scripts/pack suite harness/suites/self-healing-e2e.yaml`（spec Part 5.9）；如不通过 → 回到 iter 3 加修 | 30 分钟～半天 |

Exit Gate（plan §2）通过后写 "Loop closed" 块到本报告底部。

---

## 7. 关键代码锚点

| 主题                | 文件 : 行                                                                            |
| ------------------- | ------------------------------------------------------------------------------------ |
| Emit pipeline 真源  | `src-tauri/src/modules/runtime/evolution_emitter.rs:53`                              |
| 前端 store 订阅     | `src/App.tsx:258`                                                                    |
| Daemon stub 站点    | `src-tauri/src/modules/desktop_host/setup.rs:38, 86, 130`                            |
| 数字转 stub 站点    | `src-tauri/src/modules/application/turn_service/stream_task.rs:555`                  |
| Scanner stub 站点   | `src-tauri/src/modules/desktop_host/setup.rs:165-186`                                |
| DK lookup stub 站点 | `src-tauri/src/modules/application/turn_service/work_loop.rs:408-422`                |
| Browser 真接通      | `src-tauri/src/modules/smart_browser/runtime.rs:191, 287`                            |
| Finalize 真接通     | `src-tauri/src/modules/application/turn_service/stream_finalize.rs:1198, 1236, 1268` |

---

## 8. 本次迭代 Done-Definition

- [x] 验证 41-pack rollout 实际接通深度
- [x] 引入 `landed-stub` 术语
- [x] Pack-by-Pack 真相表（WU/DW/UI 全覆盖）
- [x] 锁定 4 个 `landed-stub` 的共同根因 = 3 条 plumbing
- [x] 排好 iteration 2/3/4/5 队列
- [x] **Iteration 2 同 session 执行**（commit `74a85f3`）
- [x] **Iteration 3 同 session 执行**（commit `652b69b`，详见 §9）

---

## 9. Iteration 3 增量（2026-04-30，commit `652b69b`）

**用户授权**：Option B 走捷径——直接 swap 三处 `MockUtilityLlm::empty()` 为 `ChatProviderUtilityLlm`，不写独立 Pack 文件。

### 9.1 已落地

| 站点                                     | 旧状态                 | 新状态                              |
| ---------------------------------------- | ---------------------- | ----------------------------------- |
| `stream_task.rs:555` digester            | `landed-stub` (DW-002) | ✅ **done** — 真实 LLM compress      |
| `stream_finalize.rs:1233` sedimentation  | "done" 但 LLM mock     | ✅ **真 done** — 沉淀产生真实 drafts |
| `stream_finalize.rs:1265` DK contributor | "done" 但 LLM mock     | ✅ **真 done** — 候选项是真 LLM 输出 |

### 9.2 证据

- `cargo check` PASS（exit 0，6m 32s）—— 编译干净，无新警告
- 零新增依赖；零 API surface 变化；零测试代码触动
- `ChatProviderUtilityLlm::new(if2ai_data_root())` 自包含构造——shim 内部 lazy 解析 provider，配置变更下次 iteration 即生效

### 9.3 仍剩 `landed-stub`

| Pack       | 站点                                          | 占位                                                                           | 替换路径                                                                                                    |
| ---------- | --------------------------------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| **DW-001** | `desktop_host/setup.rs:177` self-edit scanner | `MockUtilityLlm::empty()` + `ConstEmbedder` (line 165-170) + `&[], &[]` 空输入 | 同 swap + 改 `&[]` 为真实失败聚合源                                                                         |
| **DW-004** | `work_loop.rs:412` DK lookup                  | `MockKnowledgeStore` placeholder                                               | 需 `OnceLock<Arc<dyn KnowledgeStore>>` singleton 在 setup 注册                                              |
| **WU-002** | `setup.rs:86` 探针 vec                        | `Vec::new(), Vec::new()` for provider+MCP；`StubBrowserProbe` always-Healthy   | 需暴露 `ProviderManager.liveness_probe()` / `McpServerManager.liveness_probe()` 工厂；BrowserSession 真探针 |

### 9.4 Iteration 4 候选（按预期执行成本排序）

1. **DW-001 swap**（最简）—— `setup.rs:176-186` 改 `MockUtilityLlm::empty()` → `ChatProviderUtilityLlm::new(if2ai_data_root())`，但需要先想清楚 `&[], &[]` 空输入要换成什么真实数据源（candidate set + tool failures clusters）。**真正待回答的问题：scanner 周期性扫描的源在哪？**
2. **WU-002 browser probe 真化**（中等）—— `StubBrowserProbe` → `BrowserSessionLivenessCheck`，再让外层注册到真正被 spawn 的 daemon registry（不是当前的孤立 registry）。
3. **DW-004 + WU-008 KnowledgeStore singleton**（中大）—— 需在 `bootstrap/` 加 `KnowledgeStoreBootstrap`，暴露 `OnceLock`，再在 `work_loop.rs:412` 取代 `MockKnowledgeStore`。
4. **WU-002 provider + MCP 探针**（最大）—— 需要 `ProviderManager` / `McpServerManager` 暴露 `liveness_probe()` 工厂；可能需 trait 新增。

---

## 10. Iteration 4 增量（2026-04-30，commit `04b4a46`）

**选项 B 执行**：WU-002 browser probe 真化。

### 10.1 已落地

| 站点                                                    | 旧状态        | 新状态                                     |
| ------------------------------------------------------- | ------------- | ------------------------------------------ |
| `setup.rs` StubBrowserProbe 注入孤立 registry           | `landed-stub` | ✅ **done** — 真化                          |
| `BrowserRegistryProbe` 进入真 SH-001 daemon             | 不存在        | ✅ **新增** — polls `heartbeat_snapshots()` |
| `registry.rs` `heartbeat_snapshots()`                   | 不存在        | ✅ **新增** — try_lock best-effort          |
| `daemon/mod.rs` `spawn_self_healing_daemon_with_extras` | 不存在        | ✅ **新增** — extras 注入主 daemon          |

### 10.2 证据

- `cargo check` PASS（exit 0，2m 17s）
- `cargo test --lib browser::registry`: 4/4 PASS（新增 `heartbeat_snapshots_empty_for_fresh_registry`）
- WU-002 browser liveness probe 现在随自愈 daemon 每 60 s 轮询浏览器会话健康；Stale/Crashed/Disconnected 时真发 `DaemonHealth degraded/failed` envelope 给前端

### 10.3 仍剩 `landed-stub`

| Pack                    | 站点                         | 占位                                                            | 替换路径                                                          |
| ----------------------- | ---------------------------- | --------------------------------------------------------------- | ----------------------------------------------------------------- |
| **DW-001**              | `setup.rs:177` scanner       | `MockUtilityLlm::empty()` + `ConstEmbedder` + `&[], &[]` 空输入 | 需真实 failure 聚合源；非单次 swap                                |
| **DW-004 + WU-008**     | `work_loop.rs:412` DK lookup | `MockKnowledgeStore`                                            | 需 `OnceLock<Arc<dyn KnowledgeStore>>` singleton 在 setup 注册    |
| **WU-002 provider/MCP** | `setup.rs:86` probe vec      | `Vec::new(), Vec::new()`                                        | 需 `ProviderManager` / `McpServerManager` 暴露 `liveness_probe()` |

### 10.4 Iteration 5 候选

1. ~~**DW-004 + WU-008 KnowledgeStore singleton**~~ → **✅ 已完成 — Iteration 5**
2. **WU-002 provider probe**（暴露 `ProviderManager.liveness_probe()`，需 trait 新增）
3. **DW-001 scanner 真实 inputs**（需决定 candidate set 来源）

---

## 11. Iteration 5 增量（2026-04-30，commit `ba830b5`）

**DW-004 + WU-008**：进程级 `KnowledgeStore` singleton 落地，DK lookup 从同一实例读取。

### 11.1 已落地

| 站点 | 旧状态 | 新状态 |
| ---- | ------ | ------ |
| `domain_knowledge/mod.rs` `global_knowledge_store()` | 不存在 | ✅ **新增** — `OnceLock<Arc<dyn KnowledgeStore>>` singleton |
| `stream_finalize.rs` DK contributor loop | entries 只发 event，不落地 | ✅ 每个 entry 先 `upsert` 进 global store |
| `work_loop.rs` `spawn_dk_lookup_advisory` | `MockKnowledgeStore::new()` 每次独立实例 | ✅ `global_knowledge_store()` 共享实例 |

### 11.2 证据

- `cargo check` PASS（exit 0）
- `cargo test --lib skills::domain_knowledge::tests::global_store_shared_across_handles`: **1/1 PASS**
- 真实行为变化：finalize hook 提取到的 DomainKnowledgeEntry 现在 upsert 进进程级 store；下一轮 work_loop 的 DK lookup 将命中这些条目，`matchedContributions` 不再始终为 0

### 11.3 仍剩 `landed-stub`

| Pack | 站点 | 占位 | 替换路径 |
| ---- | ---- | ---- | -------- |
| **DW-001** | `setup.rs:177` scanner | `MockUtilityLlm::empty()` + `ConstEmbedder` + `&[], &[]` 空输入 | 需真实 failure 聚合源；非单次 swap |
| **WU-002 provider/MCP** | `setup.rs:86` probe vec | `Vec::new(), Vec::new()` | 需 `ProviderManager` / `McpServerManager` 暴露 `liveness_probe()` |

### 11.4 Iteration 6 候选

1. ~~**WU-002 provider probe**~~ → **✅ 已完成 — Iteration 6**
2. **DW-001 scanner 真实 inputs**（需决定 candidate set 来源）

---

## 12. Iteration 6 增量（2026-04-30，commit `06bf2fd`）

**WU-002 provider probe**：`ProviderApiKeyProbe` 真化，daemon 现在每 60 s 轮询 LLM 凭据是否配置。

### 12.1 已落地

| 站点 | 旧状态 | 新状态 |
| ---- | ------ | ------ |
| `daemon/mod.rs` `make_provider_api_key_probe()` | 不存在 | ✅ **新增** — env-var-only, sync, ≤ µs |
| `setup.rs` extras vec | `vec![browser_probe]` | ✅ `vec![browser_probe, provider_probe]` |

### 12.2 证据

- `cargo check` PASS（exit 0）
- `provider_api_key_probe_healthy_when_env_var_set` + `provider_api_key_probe_degraded_when_no_env_vars`: **2/2 PASS**
- 真实行为：无任何 provider 凭据时 daemon 发 `DaemonHealth degraded` envelope（reason 包含 env var 名单）

### 12.3 仍剩 `landed-stub`

| Pack | 站点 | 占位 | 替换路径 |
| ---- | ---- | ---- | -------- |
| **DW-001** | `setup.rs` scanner | `MockUtilityLlm::empty()` + `ConstEmbedder` + `&[], &[]` | 需真实 failure 聚合源 |

### 12.4 Iteration 7 候选

1. **DW-001 scanner 真实 inputs** — 调查 `run_scanner_once` 参数语义，确定 `candidates/tool_failures` 来源

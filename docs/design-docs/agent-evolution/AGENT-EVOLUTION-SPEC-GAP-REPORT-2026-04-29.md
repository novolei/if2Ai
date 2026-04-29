# If2Ai Agent 进化规范 vs 代码库 — Gap 调查报告

**版本**: 2026-04-29  
**对照基准**: `.qoder/specs/if2ai-agent-evolution-report.md`（全文，含 Part 0~8）  
**代码快照**: 当前工作区 `main` 等价状态（含未提交变更中的进化相关模块）  
**视角**: Staff 系统架构师（后端控制面、事件契约、生产路径）+ 资深前端 UI/UX（投影管道、渐进增强、可观测性）

---

## 1. 执行摘要

仓库在 **库级实现（modules + 单测 + Pack 验证）** 上已大量对齐进化路线图（`docs/packs/REGISTRY.md` 将多数 FEAT-TE/SE/SH/AE/DK/BR/INT 标为 done），但与 **设计文档中描述的端到端产品行为** 仍存在系统性断层，可归纳为三类：

| 断层类型                                    | 含义                                                                                                                                            | 对用户的可见影响                           |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| **A. 生产路径未接线（Wire-up Gap）**        | 算法与类型存在于 `src-tauri/src/modules/**`，但未在 `turn_service` / `stream_task` / `stream_finalize` / Smart Browser 工具链中调用             | 真实对话中行为与 spec 描述不一致           |
| **B. 可观测性与契约 Gap（Projection Gap）** | `RuntimeEventType` 已扩展，但 `agent-token` 路径的 `map_event_type_to_runtime` 不含进化族；生产代码中几乎无 `RuntimeEventEnvelope` 发射进化事件 | 前端无法依赖事件流展示压缩/沉淀/自愈等状态 |
| **C. 前端 UI/UX Gap（FE-001）**             | `FEAT-INT-001` 明确排除 UI 与 emitter；`evolution-event-store` 未接入网关                                                                       | Part 7 所列 24+ 组件与导航集成基本缺失     |

**结论**: 当前状态更接近 **「Pack 级能力已落地 + 集成测试证明模块可编译/可测」**，而非 spec 中 **「Self-healing harness × GenericAgent 式端到端闭环」**。下一步应以 **单一 Wire-up Pack（或 Epic）** 串起：preflight/finalize → 各模块 → `StreamTokenPayload`/独立 emit → 前端 `applyEnvelope`。

---

## 2. 元问题：设计文档与仓库元数据不一致

在深入模块前，需修正对 **单一真相源** 的预期：

1. **同一份 spec 内部矛盾**  
   - **Part 1.3（2026-04-28 扫描）** 仍将 Module A/B/D/G/J/L 标为「待实现」、C/E/F/H/I/K 为「部分」。  
   - **Part 6（差距矩阵）** 将 15 个差距点全部标为 ✅。  
   以 **当前代码为准**：Part 1.3 的「生产路径缺失」判断更接近事实；Part 6 更像 **Pack 覆盖表**（设计上已映射到 Pack），而非 **运行时已全部生效**。

2. **`docs/packs/REGISTRY.md` vs 个别 Pack 文件**  
   例：`FEAT-SH-002-provider-mcp-liveness-checks.md` 内 `State: pending`，但 REGISTRY 标为 **done**。以代码为准：`default_registry` 仍只注册 **MemoryTicker + BrokenToolStreak** 两项，`provider_heartbeat_check_fn` / `McpServerLivenessCheck` **未** 挂入 `spawn_self_healing_daemon` 的默认注册表（仅见于 `tests/evolution_phase0.rs` 等测试装配）。

**建议**: 在 spec 中增加「**实现深度分级**」：L0 类型/测试、L1 生产调用、L2 前端投影、L3 用户可操作的 UI。

---

## 3. 后端：逐模块对照 spec Part 2 / Part 5

下列表中 **Spec 要求** 摘自设计文档模块目标与 Part 5 验证条目；**代码现状** 以 `src-tauri` 生产调用链与关键 grep 为准。

### 3.1 Module A — 上下文压缩管线（P0）

| Spec 要点                           | 代码现状                                                                        | Gap                                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| 五档 `TierBudgetAllocation` 硬约束  | `stream_preflight.rs` 调用 `compress_for_request` + 默认 `TierBudgetAllocation` | **部分满足**：tier 硬帽在迭代请求路径上生效                                                                                  |
| 消息级 LLM 压缩（TE-002）           | `MessageDigester` / `apply_digest` 存在于 `context_compression/digester.rs`     | **Gap**: `stream_task.rs` 中 `build_iteration_request` 传入 **`digested_messages: None`**（多处），生产路径 **未** 跑 digest |
| Mini-index L1（TE-003）             | `build_mini_index` 在 `mini_index.rs` + planner 有 `render_mini_index_block`    | **Gap**: **`build_mini_index` 未在 application 层调用**（仅模块内测试），会话索引未自动注入                                  |
| 压缩事件可观测（Part 5.1 / Part 7） | `RuntimeEventType::CompressionEvent` 已定义                                     | **Gap**: 生产路径 **无** 向 UI 发射 `compression_event`（见 §5）                                                             |
| 120K chars → ~30K 量级目标          | 依赖 digest + mini-index + 工具摘要（TE-004 deferred）                          | **Gap**: 在 digest 未接线、TE-004 推迟的情况下，**无法保证**达到 spec 量化目标                                               |

### 3.2 Module B — Skill 沉淀（P0）

| Spec 要点                       | 代码现状                                                                    | Gap                                                                                                               |
| ------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| 从会话工具序列沉淀 `SkillDraft` | `extract_skill_drafts` 在 `skills/sedimentation/mod.rs`                     | **Gap**: **`application/turn_service` 内无引用**；沉淀仅在 `tests/evolution_e2e.rs` / `skill_evolution.rs` 等验证 |
| 去重、宪法、向量索引            | `dedup.rs`、`constitution.rs`、`vector_index.rs` 与 learning 自编辑验证联动 | **库级存在**；无 **turn 结束** 流水线自动消费                                                                     |
| `skill_sedimented` 事件         | 类型已定义                                                                  | **Gap**: 无生产 emit                                                                                              |

### 3.3 Module C — Self-Healing Daemon（P0）

| Spec 要点                        | 代码现状                                                                            | Gap                                                                                                |
| -------------------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| 守护进程 + 状态机                | `spawn_self_healing_daemon` 在 setup 路径可运行；`DaemonState::transition` 实现完整 | **基础满足**                                                                                       |
| Provider/MCP 活性（SH-002）      | `api/resilience.rs`、`McpServerLivenessCheck`                                       | **Gap**: **未注册进 `default_registry`**，与 spec「Provider/MCP 熔断与恢复」及 Part 5.3 不完全一致 |
| 前端展示 daemon 健康（Part 5.3） | 无 emit                                                                             | **Gap**（见 §4、§5）                                                                               |

### 3.4 Module D — 宪法级记忆（P1）

| Spec 要点                            | 代码现状                                                       | Gap                                                                                                    |
| ------------------------------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `evaluate_constitution` 守卫自动内容 | `skills/guard/constitution.rs` + contributor/verification 复用 | **库级满足**；注入/拦截是否在 **每一次** 相关写入路径需逐 call-site 审计（本报告未逐行证明「全覆盖」） |
| `constitution_violation` 投影        | 类型已定义                                                     | **Gap**: 无生产 emit + 无 UI 警报组件                                                                  |

### 3.5 Module E — Agent 自编辑（P1）

| Spec 要点                              | 代码现状                                                  | Gap                                                                                                                                        |
| -------------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 提案生成、验证门、升级状态机           | `learning/self_edit/{proposal,verification,promotion}.rs` | **模块存在**；需在 **工具失败闭环** 上与 `turn_service`/`tool_execution_broker` 做全链路核对（是否每类失败都触发提案属 **深度验证** 范围） |
| 运行时热加载 harness（Part 0 原则 #2） | 设计强调 daemon 不重启热加载                              | **Gap**: 与当前 Tauri 进程模型是否一致 **未** 在设计文档中更新为可执行契约                                                                 |
| `self_edit_proposal` 事件              | 类型已定义                                                | **Gap**: 无生产 emit                                                                                                                       |

### 3.6 Module F — 浏览器会话自愈（P1）

| Spec 要点                                   | 代码现状                                                                          | Gap                                            |
| ------------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------- |
| `session_health` / `build_recovery_plan`    | `smart_browser/session_health.rs`；`runtime.rs` 暴露 `next_browser_recovery_step` | **注释写明**供未来 supervisor / daemon 绑定    |
| `BrowserSessionLivenessCheck` 注册到 daemon | 仅在 `tests/evolution_phase0.rs` 装配                                             | **Gap**: **默认 daemon 未** 包含浏览器会话探针 |
| `browser_health` 事件                       | 类型已定义                                                                        | **Gap**: 无生产 emit                           |

### 3.7 Module G — 域知识仓库（P1）

| Spec 要点                                    | 代码现状                                                                               | Gap                  |
| -------------------------------------------- | -------------------------------------------------------------------------------------- | -------------------- |
| 提取/校验/查找 API                           | `skills/domain_knowledge/{mod.rs,contributor.rs}`                                      | **库级存在**         |
| 成功任务后自动贡献、二次访问注入（Part 5.4） | `turn_service` / `stream_finalize` **无** `extract_domain_knowledge_candidates` 等调用 | **Wire-up Gap**      |
| `domain_knowledge` 事件                      | 类型已定义                                                                             | **Gap**: 无生产 emit |

### 3.8 Module H — Working Checkpoint（P1）

| Spec 要点                       | 代码现状                                                | Gap                                                                                                                     |
| ------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `runtime/working_checkpoint.rs` | 模块存在；`ContextTier::WorkingCheckpoint` 在预算中占位 | **Gap**: **finalize/preflight 未见** 与 working checkpoint 提取/注入的明确生产接线（与「压缩后仍存在」Part 5.4 强相关） |
| `checkpoint_updated` 事件       | 类型已定义                                              | **Gap**: 无生产 emit                                                                                                    |

### 3.9 Module I — Skill 向量语义搜索（P1）

| Spec 要点                                | 代码现状                                  | Gap                             |
| ---------------------------------------- | ----------------------------------------- | ------------------------------- |
| 向量索引与检索                           | `skills/vector_index.rs` + tests          | **库级满足**                    |
| 第二次相似任务检索优于关键词（Part 5.2） | 依赖技能解析/注入与沉淀物写入磁盘或 store | **端到端 Gap** 与 Module B 同源 |

### 3.10 Module J — 坐标优先策略（P2）

| Spec 要点                          | 代码现状                               | Gap                                             |
| ---------------------------------- | -------------------------------------- | ----------------------------------------------- |
| 降级链坐标 → 标签 → CSS            | `smart_browser/coordinate_strategy.rs` | **仅** `tests/browser_refinement.rs` 等测试引用 |
| Smart Browser 生产路径默认坐标优先 | `runtime.rs` 仍以 MCP/CDP 桥为主       | **Gap**: **策略未接入** 实际 browser 工具执行链 |

### 3.11 Module K — 执行验证记忆门（P2）

| Spec 要点                    | 代码现状                                                    | Gap                                                                      |
| ---------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------ |
| 无工具证据拒绝写入等         | `learning/self_edit/verification.rs` 与 memory 策略模块并存 | 需专项对照 `memory_write_policy` / AE-002；本报告标记为 **待端到端证明** |
| `verification_decision` 事件 | 类型已定义                                                  | **Gap**: 无统一进化族 emit（见 §5）                                      |

### 3.12 Module L — 浏览器内容简化（P2）

| Spec 要点                             | 代码现状                     | Gap                                                                           |
| ------------------------------------- | ---------------------------- | ----------------------------------------------------------------------------- |
| `adaptive_simplify` / `simplify_html` | `content_simplifier.rs` 完整 | **仅测试** `browser_refinement.rs`；**web_scan / browser 工具结果路径未调用** |
| Part 5.5 200K→15–35K 与可交互元素保留 | 无生产调用则 **无法验证**    | **Wire-up Gap**                                                               |
| `content_simplified` 事件             | 类型已定义                   | **Gap**: 无生产 emit                                                          |

### 3.13 Module M / N（spec 新发现）

| 模块                            | 代码现状              | 相对 spec                                             |
| ------------------------------- | --------------------- | ----------------------------------------------------- |
| M Jiaochang 音频沙箱            | 大体完整（spec 亦认） | 与 Agent 进化主线 **弱耦合**，可不记为 Gap            |
| N MCP Workbench / Control Plane | 已存在                | 同上；注意 **daemon 的 MCP 活性** 仍未进默认 registry |

### 3.14 附加：AWL Todo Ledger（设计文档延伸）

`turn_service/todo_ledger.rs` + `work_loop` / `stream_finalize` 中与 `todo_ledger_incomplete` 的联动表明 **Todo 账本执行约束** 已在主路径落地。该项在 `.qoder/specs` 主文档 Part 2 模块表中 **未单独成章**，属于 **文档滞后** 而非代码缺失。

---

## 4. 前端 UI/UX：对照 Part 7

### 4.1 契约与 Store（FEAT-INT-001 范围内）

| Spec（Part 7.2 / 7.5）                                                   | 代码现状                                                                                                                                | Gap                                                                                                                            |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `contracts.ts` 扩展 `RuntimeEventType`                                   | **已** 含 10 个进化族字面量                                                                                                             | ✅                                                                                                                              |
| `RuntimeProjectionStore` 扩展 `daemon` / `skills` / `domainKnowledge` 等 | `src/runtime-projection/**` **无** 这些字段；进化状态放在 **`src/transport/runtime-event-*.ts` + `src/state/evolution-event-store.ts`** | **架构偏差**：spec 写「扩展 projection store」，实现为 **平行管道**。若长期保留，应 **回写 spec** 或 **合并投影模型** 避免双轨 |
| Translator / Reducer                                                     | `translateEnvelope` + `evolutionEventReducer` + vitest                                                                                  | ✅（骨架）                                                                                                                      |

### 4.2 接线（超出 INT-001 明确 Out of Scope — 故为 Gap）

| Spec                                                                 | 代码现状                                                                                                 | Gap                                   |
| -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| Gateway 对每个进化 envelope 调用 `evolutionEventStore.applyEnvelope` | **全仓库仅定义**，无任何 `listen('agent-token'…)` 或 bridge **调用** `applyEnvelope`                     | **致命 Gap**：Store 永不为 UI 更新    |
| Part 7.4 新建 ~24 个组件文件                                         | Glob 验证：`DaemonHealthDashboard`、`CompressionHistoryDrawer`、`DomainKnowledgeBrowser` 等 **均不存在** | **UI Gap 100%**（相对 Part 7.4 清单） |
| Part 7.5 修改 `GlobalNavbar` / `ContextBar` / `MemoryWriteCard` 等   | 未系统性植入进化可视化（需产品级 PR 核对）                                                               | **待查 / 大概率未完成**               |

### 4.3 UI/UX 设计层建议（设计师视角）

1. **信息架构**: 在双轨（`runtime-projection` vs `evolution-event-store`）未收敛前，避免为每个模块做重复订阅；优先 **单一事件入口 + 按 family 分 reducer**。  
2. **渐进增强**: 先落地 **DaemonStatusIndicator + CompressionEventToast** 两个高信号、低耦合组件，再展开 Part 7.4 全量。  
3. **可信与可解释**: Part 7 中大量「前后对比 / diff / 证据链」组件对应 spec 的 **信任 harness** 叙事；无后端 `*_event` 时 UI 只能做空态或 mock，**应阻塞纯前端假数据**。

---

## 5. 横切：运行时事件与 `stream_emitter`

| 项                                                              | 现状                                                                  | Gap                                                      |
| --------------------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------- |
| `RuntimeEventType` 进化 10 变体                                 | Rust + TS 已对齐 serde / 字面量                                       | ✅ 类型层                                                 |
| `StreamTokenPayload::to_envelope` → `map_event_type_to_runtime` | 仅映射 conversation/tool/memory/… **不包含** `daemon_health` 等字符串 | **Gap**: 即使后端发送字符串事件，**也会被丢弃为 `None`** |
| 生产代码中 `RuntimeEventType::{DaemonHealth,…}` 的使用          | **仅** `tests/evolution_phase0.rs`（及类似测试）                      | **Gap**: **测试专属**，与真实 agent-token 流无关         |

---

## 6. 验证与 Harness（Part 5 / 5.9）

| Spec 要求                                                          | 现状                      | Gap                                                             |
| ------------------------------------------------------------------ | ------------------------- | --------------------------------------------------------------- |
| Part 5.1–5.7 量化验证                                              | 依赖生产路径与可观测事件  | **无法在用户环境复现 spec 断言**                                |
| `./scripts/pack suite harness/suites/self-healing-e2e.yaml`（5.9） | 未在本调查中执行          | 建议作为 **Wire-up 完成定义（DoD）** 之一                       |
| `FEAT-INT-002` evolution e2e                                       | Rust 集成测试覆盖模块组合 | **补充**：多测 **「经 turn_service 的一遍流」**，而非仅库级拼接 |

---

## 7. 建议优先级（供排期）

| 优先级 | 主题                                                                                                                                      | 理由                                        |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| **P0** | **Wire-up Pack**：finalize 后异步队列调用 `extract_skill_drafts`、域知识提取、checkpoint 更新；preflight 接入 digest + `build_mini_index` | 直接解锁 spec 核心叙事「进化 + 最小上下文」 |
| **P0** | **Emitter Pack**：统一 `emit_evolution_envelope` + 扩展 `map_event_type_to_runtime` 或 **独立 Tauri 事件名**（避免污染 agent-token）      | unblock 前端与 Part 5.3                     |
| **P1** | `default_registry` 注册 SH-002/SH-003 探针；修正 Pack 文件 State 与 REGISTRY 一致                                                         | 对齐 Self-Healing 设计                      |
| **P1** | Smart Browser：`adaptive_simplify` + `coordinate_strategy` 接入实际 web_scan/browser 返回路径                                             | 对齐 Part 0 原则 #5、#8                     |
| **P2** | `FEAT-EVO-FE-001`（REGISTRY 已 stub）：Part 7.4 UI + `applyEnvelope` 接线                                                                 | 用户可见                                    |

---

## 8. 附录：关键代码锚点（便于 Code Review）

- **Tier 硬约束与 digest 入口**: `src-tauri/src/modules/application/turn_service/stream_preflight.rs`（`digested_messages`）、`stream_task.rs`（`digested_messages: None`）  
- **进化事件映射缺口**: `src-tauri/src/modules/runtime/stream_emitter.rs` — `map_event_type_to_runtime`  
- **Daemon 默认注册表**: `src-tauri/src/modules/runtime/daemon/mod.rs` — `default_registry`  
- **沉淀 API 无应用层调用**: `src-tauri/src/modules/skills/sedimentation/mod.rs` — `extract_skill_drafts`  
- **简化器仅测试**: `src-tauri/tests/browser_refinement.rs`  
- **前端 Store 未订阅**: `src/state/evolution-event-store.ts` — 无外部 `applyEnvelope` 调用方  

---

**文档维护**: 建议在 `if2ai-agent-evolution-report.md` 顶部增加「**最后代码对齐日期**」与指向本 Gap 报告的链接；合并 Wire-up 后应 **删减或标注过时** Part 1.3 与 Part 6 中互相冲突的陈述。

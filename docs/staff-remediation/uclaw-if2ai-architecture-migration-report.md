# UClaw -> If2Ai 第二阶段架构迁移与实施报告

> 面向 If2Ai 下一阶段整改与演进的横向对照实施文档。
>
> 最后更新: 2026-04-19
> 作者: Codex Staff Architecture Draft

## 1. 文档目的

本报告将上一阶段的横向 review 进一步收敛为可实施的迁移蓝图，目标不是“复制 UClaw”，而是从 UClaw 已验证有效的结构叙事中，提炼出 If2Ai 当前最值得吸收的架构设计、策略原理和实施顺序。

本文档回答 5 个问题：

1. UClaw 到底强在哪里。
2. If2Ai 当前真正缺的是什么。
3. 哪些能力可以立即迁移。
4. 哪些能力必须改造后再迁移。
5. 前后端应按什么顺序重构，才能不把系统搞乱。

## 2. 核心结论

### 2.1 总判断

UClaw 的优势不在于“模块更多”，而在于它形成了 4 个 If2Ai 当前最欠缺的结构条件：

1. 领域模型真相明确。
2. 工作流真相明确。
3. 运行时契约明确。
4. 前端是后端真相的投影层，而不是另一套隐性编排层。

对应证据：

- 模块与闭环状态表：[MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:5)
- 领域模型：[DOMAIN_MODEL.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/DOMAIN_MODEL.md:3)
- 当前工作流真相：[CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:9)
- 目标 UX 原则：[TARGET_PRODUCT_UX.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/TARGET_PRODUCT_UX.md:3)

### 2.2 If2Ai 当前主问题

If2Ai 当前的核心问题不是“没有 memory / harness / learning”，而是这些能力还没有收敛为单一叙事：

1. 顶层文档真相仍漂移。[ARCHITECTURE.md](/Users/ryanliu/Documents/IfAI/if2Ai/ARCHITECTURE.md:7)
2. 前端入口和聊天主 UI 已进入 God-file 区间。[App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:81) [chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:84)
3. 后端 command 层承担了过多 runtime 编排职责。[agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
4. IPC/types 文件变成了半个领域中心，但并不是正式契约层。[tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
5. `AppState` 聚合了大量 memory/learning/harness 字段，但缺一个真正的系统级 spine。[commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:28)

## 3. 对照原则

### 3.1 本次迁移不是复制 UI，而是复制治理结构

If2Ai 不应该照搬 SwiftUI 分层、命名或文件组织，而应该借鉴 UClaw 已经证明有效的 6 条治理原则：

1. 领域名词必须先定义，再编码。
2. 工作流必须标明“已跑通 / 半闭环 / 未成立”。
3. Prompt、Policy、Memory、Runtime Event 都必须有显式契约。
4. 前端必须消费 canonical event，而不是自己发明运行时语义。
5. 主链路必须优先于壳子页面。
6. 占位能力必须诚实标注，不伪装成真实闭环。

### 3.2 迁移判断标准

下文按三类输出：

- 可立即迁移：可直接在 If2Ai 落地，主要是结构和治理模式，不依赖 UClaw 特定实现。
- 需改造迁移：理念正确，但 If2Ai 需要按 Tauri + React + 当前代码现实做适配。
- 不建议迁移：UClaw 中存在价值，但不适合作为 If2Ai 当前阶段优先路径。

## 4. 可立即迁移

### 4.1 领域模型文档作为官方真相

UClaw 的 `agent / workers / session / project / automation / memory` 定义非常清楚，并且写到了代码导向文档里。[DOMAIN_MODEL.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/DOMAIN_MODEL.md:3)

If2Ai 可以立即迁移的不是“字段定义”，而是这种工作方式：

1. 先定义 If2Ai 官方实体集：`agent`、`session`、`project`、`run`、`worker`、`memory`、`automation`、`harness_eval`。
2. 每个实体写清楚：
   - 是什么
   - 不是什么
   - 生命周期
   - 所属层
   - UI 感知点
   - 持久化归属
3. 后续 design doc、exec-plan、代码命名统一向它对齐。

实施建议：

- 新建 `docs/staff-remediation/if2ai-canonical-domain-model.md`
- 作为后续重构和 harness 指标映射的上位依据

### 4.2 工作流真相文档机制

UClaw 最值得借鉴的一点，是它会明确告诉团队哪些 workflow 已跑通，哪些只是半闭环。[CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:18)

If2Ai 可以立即迁移这种治理机制：

1. 为主链路建立单页真相文档：
   - app 启动
   - chat send prompt
   - stream / tool / approval / memory event
   - session 恢复
   - harness record / replay / eval
2. 对每条 workflow 只允许 3 种状态：
   - 已跑通
   - 半闭环
   - 未成立
3. 每次架构变更，先更新 workflow truth，再改代码或同步改代码。

### 4.3 Prompt 结构化规划

UClaw 的 `PromptPlan`、`PromptBlock`、`PromptContribution`、`diagnostics`、`block_hash` 值得直接借鉴。[planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs:17)

If2Ai 立即可迁移的内容：

1. 建立 `PromptPlan` 领域对象。
2. 建立 block 分类：
   - persona
   - policy
   - environment
   - project context
   - memory injection
   - user intent
3. 每轮 turn 产出 prompt diagnostics。
4. harness 记录 prompt hash 与 block composition。

这样做的直接收益：

- prompt 可比较
- prompt 改动可回归
- memory 注入影响可量化
- self-evolution 不再是黑箱 prompt 漂移

### 4.4 Policy 三层链

UClaw 的 `Boundary -> Permission -> Sandbox` 三层策略链已经足够成熟，值得直接成为 If2Ai 的目标抽象。[coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs:1)

If2Ai 立即可迁移的内容：

1. `ExecutionBoundary`
2. `PermissionDecision`
3. `SandboxMaterialization`
4. 单入口 `prepare_step_execution`

当前 `permissionMode` 只覆盖用户设置视角，还不是真正的执行治理链。[App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:210)

### 4.5 Memory 单协调器叙事

UClaw 的 `MemoryManager` 最大价值是让“记忆系统”从一组特性变成一个统一流程。[memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs:1)

If2Ai 可立即迁移的不是它的具体策略参数，而是这条单线叙事：

1. turn 前 `prepare_context`
2. retrieval + rerank
3. 组装多层 memory payload
4. turn 后 `after_turn`
5. selective write policy
6. quality gate
7. 再把结果交给 compile / reflect / offline evolution

### 4.6 Runtime Event Canonical Envelope

UClaw 后端把 runtime message、tool spec、tool result、stream event、memory captured item 都固化为显式合同。[contracts.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/runtime/contracts.rs:6)

If2Ai 可以立即迁移的核心做法：

1. 不再让 `tauri.ts` 既当 transport facade 又当事实契约中心。
2. 在 Rust 侧建立 canonical runtime contract。
3. 前端只消费 canonical envelope。
4. 历史 payload 通过 translator 兼容。

## 5. 需改造迁移

### 5.1 EngineFacade 式 backend spine

UClaw 的 `EngineFacade` 很像系统脊柱，集中持有 session、summary、memory、run store、policy store、replay lineage 等系统级对象。[facade.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/engine/facade.rs:38)

这套思想非常值得借鉴，但 If2Ai 不能直接搬：

原因：

1. If2Ai 当前是 Tauri command-first 结构。
2. `AppState` 已经聚合了大量系统对象，但缺少应用服务边界。
3. 直接新建一个巨型 facade，容易只是把 God-file 从 `agent.rs` 挪到别处。

If2Ai 的改造迁移方式应是：

1. 先建 `application/turn_service.rs`
2. 再建 `application/session_service.rs`
3. 再抽 `application/prompt_service.rs`
4. 最后形成轻量 `runtime_facade` 或 `execution_kernel`

也就是说，要先拆服务，再形成脊柱，而不是先造新的大中心。

### 5.2 AppStore 式前端总投影层

UClaw 的 `AppStore` 虽然很大，但它至少具备清晰的 feature stores 和事件投影意识。[AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:64)

If2Ai 不适合机械地复制一个超大 React `AppStore`，但非常值得迁移其核心思路：

1. 会话、项目、run、approval、memory、activity 各自有 feature store。
2. UI 只读这些 store 的投影。
3. stream event 进入统一 event reducer，而不是散落在页面逻辑中。

推荐的 If2Ai 改造方案：

- `src/state/runtime-store.ts`
- `src/state/session-store.ts`
- `src/state/project-store.ts`
- `src/state/memory-store.ts`
- `src/state/approval-store.ts`
- `src/state/harness-store.ts`
- `src/state/event-router.ts`

### 5.3 RuntimeEventProcessor + EventTranslator

UClaw 前端通过 `EventTranslator` 把 legacy payload 转成 canonical envelope，再由 `RuntimeEventProcessor` 做去重、排序、批量刷入。[EventTranslator.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/RuntimeEvents/EventTranslator.swift:3) [RuntimeEventProcessor.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/RuntimeEvents/RuntimeEventProcessor.swift:13)

If2Ai 非常应该迁移这套思路，但要按 React 实现：

1. 建 `runtime-event-translator.ts`
2. 建 `runtime-event-queue.ts`
3. 建 `runtime-event-reducer.ts`
4. 让页面组件不再直接理解原始 token / tool / memory payload

这一步和 PromptPlan、RuntimeContract 是联动的，不能孤立做。

### 5.4 启动门禁与主壳层分离

UClaw 的 `AppState` 把 startup、onboarding、activation gate 明确建模了。[AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift:6)

If2Ai 当前 [App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:94) 已经开始有 startup gating，但仍和项目、session、UI 布局、权限状态混在一起。

推荐改造迁移方式：

1. `app-boot-state.ts`
2. `AppBootGate.tsx`
3. `MainShell.tsx`
4. `ChatSectionContainer.tsx`

### 5.5 模块状态诚实标注机制

UClaw 的 `MODULE_MAP.md` 和 `TARGET_PRODUCT_UX.md` 会明确标注哪些模块只是半实现或壳子。[MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:25) [TARGET_PRODUCT_UX.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/TARGET_PRODUCT_UX.md:13)

If2Ai 应迁移这种产品治理机制，但要结合现有 `exec-plans` 与 remediation docs：

1. 每个模块标记：
   - canonical
   - partial
   - shell-only
   - experimental
2. Settings、Skills、Memory、Harness 页的所有假数据都必须显式加标识。

## 6. 不建议迁移

### 6.1 直接复制 UClaw 的文件组织和命名

不建议直接把 UClaw 的 SwiftUI / Rust 命名体系搬进 If2Ai。

原因：

1. 技术栈不同。
2. If2Ai 当前已有自己的模块命名和历史负担。
3. 生搬硬套会制造第二套陌生命名，而不是收敛系统。

### 6.2 直接复制“大而全的 AppStore”

UClaw 的 `AppStore` 是成熟演进后的结果，不是起点。[AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:4)

If2Ai 当前如果直接新造一个更大的 React store，只会把复杂度集中，而不是消除。

### 6.3 先扩 Projects / Automation / 多工作区再谈主链收敛

UClaw 自己也承认很多非 Chat 工作区仍然半闭环。[CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:57)

If2Ai 当前不应优先把“面”铺得更大，而应先收敛 chat runtime、memory policy、harness eval、session truth。

### 6.4 把 UClaw 当前半成品区域误当成最佳实践

例如 workspace adapter 中仍有大量壳子路径，这部分不值得学习，只说明 UClaw 也有产品面与主链脱节的问题。[MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:27)

## 7. If2Ai 目标架构

### 7.1 目标后端结构

建议 If2Ai 后端重构为六层：

1. `commands`
   - Tauri IPC adapter
   - 参数校验
   - 错误映射
2. `application`
   - `turn_service`
   - `session_service`
   - `settings_service`
   - `harness_service`
3. `runtime`
   - canonical contracts
   - conversation runtime
   - stream orchestration
   - prompt planning
4. `control_plane`
   - boundary
   - permission
   - sandbox
   - audit
5. `memory_learning`
   - capture
   - retrieval
   - write policy
   - quality gate
   - reflection
   - trajectory feedback
6. `observability`
   - trace
   - replay
   - eval
   - metrics

### 7.2 目标前端结构

建议 If2Ai 前端重构为五层：

1. `boot`
   - startup
   - onboarding
   - env checks
2. `shell`
   - section routing
   - nav
   - inspector entry
3. `runtime-projection`
   - event translator
   - event queue
   - reducers
   - feature stores
4. `features`
   - chat
   - project
   - memory
   - harness
   - settings
5. `transport`
   - thin tauri bridge
   - contract adapters

## 8. 前后端重构顺序

### 8.1 Phase M0：真相收口

目标：先让系统知道自己现在是什么。

后端：

1. 定义 canonical runtime contract v1。
2. 定义 canonical domain model。
3. 把 `agent.rs` 中与 prompt/memory/stream 相关的领域结构先抽成独立模块，不改行为。

前端：

1. 定义 runtime event envelope TS 类型。
2. 建立 translator 层，允许旧事件继续接入。
3. 梳理 `App.tsx` 当前负责的启动、项目、会话、stream 责任清单。

验收：

- 文档与代码对同一组实体、同一组 event 使用相同命名。

### 8.2 Phase M1：后端先拆 command god-file

目标：把主脑从 command 层迁走。

后端顺序：

1. `agent.rs` -> `application/turn_service.rs`
2. `prompt assembling` -> `runtime/prompt_plan.rs`
3. `memory injection` -> `memory_learning/injection_service.rs`
4. `provider client resolving` -> `application/provider_service.rs`
5. `stream payload emission` -> `runtime/stream_emitter.rs`

前端只做兼容，不做大改。

验收：

- `commands/agent.rs` 只负责 IPC 输入输出和调用 service。

### 8.3 Phase M2：前端建立投影层

目标：让 React 不再直接消费原始 runtime 细节。

前端顺序：

1. 从 `tauri.ts` 拆出 `contracts.ts`
2. 新建 `runtime-event-translator.ts`
3. 新建 `runtime-event-reducer.ts`
4. 新建 `session-store` / `run-store` / `memory-store`
5. 让 `ChatWorkspace` 改读 store projection

后端同步：

1. 保持旧 payload 可用
2. 增发 canonical envelope

验收：

- `App.tsx` 不再直接承载大段 stream 业务逻辑。
- `chat-ui.tsx` 不再直接理解多种 runtime 原始事件。

### 8.4 Phase M3：Memory 变成单协调器系统

目标：让 memory 成为正式平台能力。

后端顺序：

1. 抽 `MemoryManager` 或 `MemoryCoordinator`
2. 抽 `WritePolicy`
3. 抽 `QualityGate`
4. 抽 `RecallAssembler`
5. 抽 `ReflectionFeedback`

前端顺序：

1. Memory UI 全部读 memory store
2. 区分 candidate / approved / recalled / pinned / compiled
3. 禁止 seeded trust/risk 数据伪装真实状态

验收：

- 每次 turn 都能说明：
  - 是否召回
  - 召回了什么
  - 为什么写入
  - 为什么拒绝写入

### 8.5 Phase M4：Policy 与 Harness 联动

目标：让治理系统真正接管演进。

后端顺序：

1. 引入 `prepare_step_execution`
2. 用 boundary/permission/sandbox 重构工具执行前链路
3. harness 记录 prompt plan、policy decision、memory decision、run outcome
4. 把 replay/eval 接入发布门

前端顺序：

1. Settings 中展示可解释的 policy 状态，而不是只有 mode
2. Harness/Diagnostics 页展示 run eval、failure cluster、strategy compare

验收：

- 新策略没有 eval 通过就不能升默认。

### 8.6 Phase M5：自我进化闭环

目标：把 memory + harness + reflection 变成真正的智能增强飞轮。

后端：

1. trajectory scoring
2. failure clustering
3. reflection synthesis
4. strategy candidate generation
5. offline eval and gated promotion

前端：

1. diagnostics 中看得到策略版本和效果变化
2. memory 页面看得到 recall gain / failure notes / suppression

验收：

- “更智能”必须能被量化为：
  - 完成率提升
  - 重复错误下降
  - recall precision 提升
  - tool misuse 下降

## 9. 实施优先级清单

### 9.1 P0

1. canonical domain model
2. canonical runtime contract
3. `commands/agent.rs` service 化拆分
4. React runtime event translator/reducer
5. workflow truth 文档

### 9.2 P1

1. prompt plan
2. policy coordinator
3. memory coordinator
4. run-store / memory-store / approval-store
5. seeded fake state 清理

### 9.3 P2

1. harness 评测门
2. reflection feedback
3. strategy compare
4. automation / project / memory workspace 深化

## 10. 风险与防错

### 10.1 主要风险

1. 一边重构一边继续加 feature，导致迁移永远无法收口。
2. 只拆文件不拆边界，最后只是把 God-file 分裂成多份耦合文件。
3. 先重做 UI 壳层，后端真相仍旧混乱，导致前端再次背负编排责任。
4. 过早上 self-evolution，结果只是 prompt 漂移和行为随机化。

### 10.2 约束规则

1. 每个迁移 phase 必须先定义 contract，再写 feature code。
2. 每个 phase 必须有 harness slice 验证。
3. 任何新增“智能增强”都必须经过 replay/eval。
4. 若新设计不能减少 `App.tsx` / `chat-ui.tsx` / `agent.rs` 的责任，就不算成功。

## 11. 最终建议

If2Ai 下一阶段不该把 UClaw 当作模板工程，而应该把它当作一个已经证明以下命题成立的参考系：

1. 系统可以先有一套诚实的领域真相。
2. 系统可以把 workflow 真相写清楚。
3. 系统可以把 prompt、policy、memory、runtime event 都变成正式契约。
4. 系统可以让前端主要承担“投影与控制”，而不是“隐性编排”。

对 If2Ai 而言，真正的迁移目标不是“更像 UClaw”，而是：

> 让 If2Ai 形成一套自己的 canonical domain model、canonical runtime contract、canonical workflow truth，并让 harness 成为 memory 与自我进化的治理中枢。

只有做到这一点，If2Ai 才适合继续往“更智能的智能体”推进，而不是继续在功能堆叠中扩大复杂度。

# If2Ai Staff 级整改与演进蓝图

> 面向当前 If2Ai codebase 的 Staff 级系统整改、横向对照迁移与下一阶段演进设计总纲。
>
> 最后更新: 2026-04-19
> 作者: Codex Staff Architecture Draft

## 1. 文档定位

本蓝图是 If2Ai 当前阶段的统一总纲，整合以下两类内容：

1. 当前 codebase 的 Staff 级问题画像、风险和 90 天演进目标。
2. 横向对照 UClaw 后端与 SwiftUI 前端后，提炼出的可迁移架构结构、策略原理与实施顺序。

本文档的目标不是生成“漂亮的未来图”，而是给出：

1. If2Ai 当前到底卡在哪里。
2. UClaw 真正值得借鉴的是什么。
3. 哪些内容可以立即迁移，哪些必须改造，哪些不应照搬。
4. 前后端应按什么顺序重构。
5. memory、harness、自我进化如何在一个统一架构里闭环。
6. activation gate 如何并入 If2Ai 的启动门禁与远端服务体系。
7. UClaw 四类聊天执行场景自动匹配机制如何迁入 If2Ai。

## 2. 执行摘要

If2Ai 已经具备一个有清晰 ambition 的智能体桌面平台雏形：Tauri + Rust 后端、React 前端，以及 memory / skills / browser / voice / harness 等子系统。

但系统的核心矛盾已经从“功能是否存在”转向“系统是否收敛”：

1. 文档真相与实现真相漂移。
2. 前后端主链路均出现 God-file。
3. prompt、memory、policy、runtime event 还未成为正式契约。
4. harness 还不是治理中枢，只是局部记录/评测能力。
5. memory 与 learning 已有实现，但尚未形成可验证的智能体增强飞轮。
6. 前端目前仍承担了相当多隐性编排责任，而不仅是系统真相的投影层。
7. app 启动链路尚未纳入正式的激活/反激活门禁与远端 license lifecycle。
8. chat 入口尚未建立类似 UClaw 的四类执行场景自动匹配与 explainable routing。

横向对照 UClaw 之后，结论很明确：

> If2Ai 当前最需要的不是继续扩能力面，而是先形成自己的 canonical domain model、canonical workflow truth、canonical runtime contract，并让 harness 成为 memory 与自我进化的治理中枢。

## 3. 本轮综合评审结论

### 3.1 已确认的高优先级问题

1. 前端测试门声明存在，但当前不可运行。
2. Harness 注释与真实启用路径不一致，默认运行时无法按 IPC 方式打开。
3. Session 持久层在磁盘脏数据场景下存在 panic 风险。
4. 主入口 HomeScreen 存在基础无障碍缺口。
5. Skills 市场页使用静态 seeded 风险评级，产品可信度受损。
6. 顶层架构文档已明显过时，不再可靠反映系统现状。

### 3.2 系统级问题画像

#### A. 架构问题

- 前端状态、视图编排、Tauri 调用边界耦合过深。
- 后端 `commands` 层承担了过多 runtime 编排职责。
- 领域能力虽多，但系统边界未完全固化为稳定接口。
- prompt / policy / memory / runtime event 仍主要通过实现惯例连接，而不是通过正式契约连接。

#### B. 质量问题

- 部分质量门“写在文档中”，但不在工程中真正可执行。
- harness 更接近 trace/record 设施，还未成为发布和回归决策器。
- 文档、代码、执行计划三者之间缺乏强一致机制。

#### C. 产品问题

- 专业能力面板过多，主路径与次路径认知分层不够清晰。
- 占位数据与真实数据混用，会削弱用户信任。
- 部分 UI 具备视觉完成度，但语义和可访问性尚未跟上。

#### D. 智能体能力问题

- memory 已开始工程化，但 recall / promote / reflect / compile 还未形成可度量闭环。
- learning 模块存在，但还未真正成为“模型行为被改进”的系统主轴。
- 缺少基于 harness 的策略 AB、回放回归、失败归因与策略上线流程。

#### E. 平台接入与分发问题

- If2Ai 目前尚未形成与 UClaw 同等级别的 activation gate 生命周期管理。
- app 启动、onboarding 完成、license refresh、revocation check、失效回流之间尚无统一门禁。
- 若未来走桌面分发、邀请制、Beta 灰度、组织级授权，当前结构无法作为可控入口。

#### F. 请求智能与场景路由问题

- If2Ai 目前缺少 UClaw 那种由 runtime 驱动的 ingress classifier。
- chat 入口还不能将用户请求自动分流到四类执行场景。
- execution mode、risk、complexity、route hint 还没有成为前后端共享契约。
- 前端没有稳定展示“为什么这次进入这个场景”的解释性投影。

## 4. UClaw 横向对照后的核心判断

### 4.1 UClaw 真正领先的地方

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

### 4.2 If2Ai 当前主问题

If2Ai 当前的核心问题不是“没有 memory / harness / learning”，而是这些能力还没有收敛为单一叙事：

1. 顶层文档真相仍漂移。[ARCHITECTURE.md](/Users/ryanliu/Documents/IfAI/if2Ai/ARCHITECTURE.md:7)
2. 前端入口和聊天主 UI 已进入 God-file 区间。[App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:81) [chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:84)
3. 后端 command 层承担了过多 runtime 编排职责。[agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
4. IPC/types 文件变成了半个领域中心，但并不是正式契约层。[tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
5. `AppState` 聚合了大量 memory/learning/harness 字段，但缺一个真正的系统级 spine。[commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:28)

### 4.3 迁移总原则

本次迁移不是复制 UClaw 的 UI 或代码组织，而是复制它已经证明有效的治理结构：

1. 领域名词必须先定义，再编码。
2. 工作流必须标明“已跑通 / 半闭环 / 未成立”。
3. Prompt、Policy、Memory、Runtime Event 都必须有显式契约。
4. 前端必须消费 canonical event，而不是自己发明运行时语义。
5. 主链路必须优先于壳子页面。
6. 占位能力必须诚实标注，不伪装成真实闭环。
7. 启动门禁必须覆盖 startup、onboarding、activation gate 三段，而不是只做本地首次启动判断。
8. chat 场景匹配必须由 runtime classifier 决定，前端只做投影和覆写，不可自行重算。

## 5. 现阶段系统定位

建议将 If2Ai 从“功能并行开发中的 agent desktop app”重新定义为：

> 一个以本地工作空间为中心、以可观测和可评测为内核、逐步具备记忆增强与策略演进能力的智能体操作系统雏形。

这个定位意味着后续所有优化都要优先回答三件事：

1. 这个能力是否可评测？
2. 这个能力是否可观测？
3. 这个能力是否能被稳定集成到 agent 主回路？

## 6. Staff 级整改目标

未来 90 天建议围绕四个目标推进：

### G1. 建立可信基线

- 修复测试、文档、harness 启用、持久层鲁棒性问题。
- 让“代码库即记录系统”重新成立。
- 让核心质量门可在本地与 CI 一致执行。

### G2. 收敛平台架构

- 拆分 God-file。
- 将前后端主链路明确成稳定边界。
- 降低新增功能继续堆叠的结构性成本。

### G3. 让 memory 成为真实能力

- 建立 memory capture / recall / pin / compile / reflect 的闭环。
- 让 memory 对 turn 质量产生可验证提升。
- 将 memory 指标纳入 harness 评测。

### G4. 让 harness 成为治理系统

- 从“录 traces”升级到“决定是否能发布某个策略”。
- 对工具调用、恢复能力、成本、用户完成率给出基准。
- 成为自我进化回路的验证器。

## 7. 目标架构蓝图

建议将系统收敛为四层主平面。

### 7.1 Conversation Plane

职责：

- 用户输入接入
- turn 生命周期
- streaming 输出
- 中断 / 恢复 / completion 协议

原则：

- 只管理一次对话执行，不直接承载 memory/learning 细节。
- 对上暴露 UI 友好的状态投影，对下调用 runtime contract。

### 7.2 Control Plane

职责：

- tool routing
- permission policy
- audit
- session execution context
- boundary resolve

原则：

- 所有横切策略统一进入 control plane。
- command 只做 IPC adapter，不直接组合复杂策略。

### 7.3 Memory & Learning Plane

职责：

- episodic / pinned / compiled memory
- retrieval
- summarization
- reflection
- trajectory scoring
- policy feedback

原则：

- 允许异步后台处理，但所有写入都必须可审计。
- 不直接修改主对话逻辑，而是通过注入和策略输入影响下一轮行为。

### 7.4 Experience Plane

职责：

- harness traces
- telemetry
- offline eval
- regression suite
- policy comparison

原则：

- 这是“自我进化”的仲裁层。
- 没有被 Experience Plane 验证过的优化，不应直接升级为默认策略。

## 8. 可迁移内容分级

### 8.1 可立即迁移

#### A. 领域模型文档作为官方真相

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

#### B. 工作流真相文档机制

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

#### C. Prompt 结构化规划

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

#### D. Policy 三层链

UClaw 的 `Boundary -> Permission -> Sandbox` 三层策略链已经足够成熟，值得直接成为 If2Ai 的目标抽象。[coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs:1)

If2Ai 立即可迁移的内容：

1. `ExecutionBoundary`
2. `PermissionDecision`
3. `SandboxMaterialization`
4. 单入口 `prepare_step_execution`

#### E. Memory 单协调器叙事

UClaw 的 `MemoryManager` 最大价值是让“记忆系统”从一组特性变成一个统一流程。[memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs:1)

If2Ai 可立即迁移的内容：

1. turn 前 `prepare_context`
2. retrieval + rerank
3. 组装多层 memory payload
4. turn 后 `after_turn`
5. selective write policy
6. quality gate
7. compile / reflect / offline evolution

#### F. Runtime Event Canonical Envelope

UClaw 后端把 runtime message、tool spec、tool result、stream event、memory captured item 都固化为显式合同。[contracts.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/runtime/contracts.rs:6)

If2Ai 可以立即迁移的核心做法：

1. 不再让 `tauri.ts` 既当 transport facade 又当事实契约中心。
2. 在 Rust 侧建立 canonical runtime contract。
3. 前端只消费 canonical envelope。
4. 历史 payload 通过 translator 兼容。

#### G. 四类聊天执行场景自动匹配

这一项也应列为 If2Ai 的正式迁移目标。UClaw 在运行时并不是简单把所有请求都当“聊天”，而是通过 task complexity / ingress classifier 先做执行场景判断，再决定进入哪条执行路径。

对应证据：

- `ExecutionMode` 明确定义了四类执行模式：`direct_execute`、`auto_plan_execute`、`plan_then_confirm`、`specialized_surface`。[task_complexity.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/coding/task_complexity.rs:4)
- classifier 输出同时包含 `risk_level`、`complexity_level`、`reason_codes`、`route_hint`、`requires_plan`。[task_complexity.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/coding/task_complexity.rs:206)
- PRD 明确写出决策策略：`deterministic gate -> heuristic scorer -> ambiguous-only mini classifier`。[PHASE6-AGENT-EXECUTION-PRD-ARCHITECTURE.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/Docs/Migration/Phase%206%20Docs/PHASE6-AGENT-EXECUTION-PRD-ARCHITECTURE.md:149)
- route decision 会根据 `execution_mode` 和 `route_hint` 进入不同 entry surface。[chat_ws.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/api/routes/chat_ws.rs:744)

If2Ai 建议照搬的核心不是某几个关键词规则，而是这套“先分类，再执行”的运行时主轴：

1. `direct_execute`
   - 一步式、低风险、低依赖任务
2. `auto_plan_execute`
   - 多步但可安全自动推进
3. `plan_then_confirm`
   - 多步且高风险或高副作用，先出计划再确认
4. `specialized_surface`
   - 命中 coding / compose / workflow 等专门入口

这四类执行场景应成为 If2Ai chat 主入口的 canonical execution modes。

同时，UClaw 的 `ScenarioAgentStudio` 里还有上层 profile，例如 `Chat`、`Coding`、`Research`、`Planning`、`Review`。[ScenarioAgentStudioStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/ScenarioAgentStudioStore.swift:100)

If2Ai 对这部分的借鉴原则应是：

1. 把四类 `execution_mode` 作为运行时真相。
2. 把 `chat / coding / research / planning / review` 这类 scenario profile 作为上层产品模板或 focus mode。
3. 不要把 profile 概念和 runtime execution mode 混为一谈。

### 8.2 需改造迁移

#### A. EngineFacade 式 backend spine

UClaw 的 `EngineFacade` 很像系统脊柱，集中持有 session、summary、memory、run store、policy store、replay lineage 等系统级对象。[facade.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/engine/facade.rs:38)

这套思想值得借鉴，但 If2Ai 不能直接搬：

1. If2Ai 当前是 Tauri command-first 结构。
2. `AppState` 已经聚合了大量系统对象，但缺少应用服务边界。
3. 直接新建一个巨型 facade，容易只是把 God-file 从 `agent.rs` 挪到别处。

正确迁移方式：

1. 先建 `application/turn_service.rs`
2. 再建 `application/session_service.rs`
3. 再抽 `application/prompt_service.rs`
4. 最后形成轻量 `runtime_facade` 或 `execution_kernel`

#### B. AppStore 式前端总投影层

UClaw 的 `AppStore` 虽然很大，但它具备清晰的 feature stores 和事件投影意识。[AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:64)

If2Ai 不适合机械复制一个超大 React `AppStore`，但应迁移其核心思路：

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

#### C. RuntimeEventProcessor + EventTranslator

UClaw 前端通过 `EventTranslator` 把 legacy payload 转成 canonical envelope，再由 `RuntimeEventProcessor` 做去重、排序、批量刷入。[EventTranslator.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/RuntimeEvents/EventTranslator.swift:3) [RuntimeEventProcessor.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/RuntimeEvents/RuntimeEventProcessor.swift:13)

If2Ai 应迁移这套思路，但按 React 实现：

1. 建 `runtime-event-translator.ts`
2. 建 `runtime-event-queue.ts`
3. 建 `runtime-event-reducer.ts`
4. 让页面组件不再直接理解原始 token / tool / memory payload

#### D. 启动门禁与主壳层分离

UClaw 的 `AppState` 把 startup、onboarding、activation gate 明确建模了。[AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift:6)

If2Ai 当前 [App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:94) 已经开始有 startup gating，但仍和项目、session、UI 布局、权限状态混在一起。

推荐改造方案：

1. `app-boot-state.ts`
2. `AppBootGate.tsx`
3. `MainShell.tsx`
4. `ChatSectionContainer.tsx`

#### E. Activation Gate 与远端激活服务

这一项需要明确列为 If2Ai 的正式迁移目标：不仅迁移 UClaw 的 `startup + onboarding + activation gate` 三段式门禁，还要迁移其“远端激活服务 + 本地 license 生命周期 + 失效回流”的完整模式。

对应证据：

- `RootView` 中 activation gate 叠加在主壳层之上，并在 license invalidated 时强制回流 gate。[RootView.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/RootView.swift:33)
- `AppState` 将 activation gate 作为独立状态对象管理，并在 startup 结束后 bootstrap。[AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift:33)
- `ActivationService` 通过远端接口完成 request / status / redeem / redeem-by-code / refresh / revoke-check。[ActivationService.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationService.swift:32)
- `ActivationLifecycleManager` 负责 refresh loop、revoke-check loop、validity check 和失效通知。[ActivationLifecycleManager.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationLifecycleManager.swift:8)

If2Ai 在这一项上不应只“借鉴交互形式”，而应尽量照搬其产品和服务模式，使用同样类型的远程后端服务实现 APP 的激活与反激活：

1. 启动链路必须变为：
   - startup checking
   - onboarding
   - activation gate
   - main shell
2. activation gate 必须是主窗口上的硬门禁，而不是设置页中的软入口。
3. 后端必须引入远端 activation service client，至少覆盖：
   - activation request
   - activation status poll
   - redeem
   - redeem by invite code
   - license refresh
   - revoke check
4. 本地必须有 activation persistence + secure token/license storage。
5. license 过期、吊销、刷新失败超宽限时，必须支持反激活回流主 gate。
6. Settings 或 account surface 中必须提供显式的反激活 / 解绑 / revoke 本机授权入口。

这项迁移在 If2Ai 中的落地方式建议为：

前端：

1. `boot/activation-gate-state.ts`
2. `boot/ActivationGate.tsx`
3. `boot/activation-client.ts`
4. `boot/license-lifecycle.ts`

后端：

1. `application/activation_service.rs`
2. `application/license_lifecycle_service.rs`
3. `runtime/contracts/activation.rs`
4. `commands/activation.rs`

这一项虽然是“需改造迁移”，但要求非常强：目标不是做一个类似的页面，而是引入与 UClaw 同等级别的远端激活与反激活体系。

#### F. 模块状态诚实标注机制

UClaw 的 `MODULE_MAP.md` 和 `TARGET_PRODUCT_UX.md` 会明确标注哪些模块只是半实现或壳子。[MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:25) [TARGET_PRODUCT_UX.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/TARGET_PRODUCT_UX.md:13)

If2Ai 应迁移这种治理机制，但结合现有 `exec-plans` 与 remediation docs：

1. 每个模块标记：
   - canonical
   - partial
   - shell-only
   - experimental
2. Settings、Skills、Memory、Harness 页的所有假数据都必须显式加标识。

#### G. 场景模板与 Focus Mode

UClaw 在 runtime `execution_mode` 之外，还维护了上层场景模板，例如 `Chat`、`Coding`、`Research`、`Planning`、`Review`。[ScenarioAgentStudioStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/ScenarioAgentStudioStore.swift:100)

If2Ai 可以改造迁移这部分，但要注意层次：

1. `execution_mode` 是运行时自动分类结果。
2. scenario/focus mode 是用户可见的产品模板。
3. scenario 只能影响默认 prompt、agent binding、model preference、merge/gate policy。
4. scenario 不应取代 runtime classifier 的最终决策权。

### 8.3 不建议迁移

#### A. 直接复制 UClaw 的文件组织和命名

不建议直接把 UClaw 的 SwiftUI / Rust 命名体系搬进 If2Ai。

原因：

1. 技术栈不同。
2. If2Ai 当前已有自己的模块命名和历史负担。
3. 生搬硬套会制造第二套陌生命名，而不是收敛系统。

#### B. 直接复制“大而全的 AppStore”

UClaw 的 `AppStore` 是成熟演进后的结果，不是起点。[AppStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/AppStore.swift:4)

If2Ai 当前如果直接新造一个更大的 React store，只会把复杂度集中，而不是消除。

#### C. 先扩 Projects / Automation / 多工作区再谈主链收敛

UClaw 自己也承认很多非 Chat 工作区仍然半闭环。[CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:57)

If2Ai 当前不应优先把“面”铺得更大，而应先收敛 chat runtime、memory policy、harness eval、session truth。

#### D. 把 UClaw 当前半成品区域误当成最佳实践

例如 workspace adapter 中仍有大量壳子路径，这部分不值得学习，只说明 UClaw 也有产品面与主链脱节的问题。[MODULE_MAP.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/MODULE_MAP.md:27)

## 9. 前端整改设计

### 9.1 当前问题

- `App.tsx` 承载过多全局编排。
- `src/components/ui/chat-ui.tsx` 体量过大，兼具容器、视图、行为与状态。
- `src/lib/tauri.ts` 既是 transport layer，又承载大量领域类型。
- HomeScreen / ProjectRail / Settings 多处存在可访问性与交互语义债。

### 9.2 目标前端结构

建议将前端收敛为：

- `boot`
  - startup、onboarding、activation gate、env checks
- `shell`
  - 窗口级布局、全局导航、右侧辅助面板入口
- `runtime-projection`
  - event translator、event queue、reducers、feature stores、execution-mode projection
- `features`
  - chat、project、memory、harness、settings、focus mode / scenario templates
- `transport`
  - thin tauri bridge、contract adapters

### 9.3 UI/UX 原则

1. 主任务路径优先
   - 用户默认只看到“提问、选择项目、查看输出”。
2. 专业面板二级展开
   - telemetry、memory debug、skills 市场、voice debug 不应与主路径并列抢注意力。
3. 真数据优先
   - 未经验证的风险评级、可用性状态、市场分数都必须标为 preview/demo。
4. 语义优先于视觉
   - 主操作必须使用真实 button / label / focus 语义，而不是视觉模拟交互。
5. activation gate 是平台门禁，不是普通设置项
   - 未激活、已吊销、已过期、已解绑设备时，必须阻断进入主工作台。
6. execution mode 要可解释
   - 用户和开发者都应看得到这次是 `direct_execute`、`auto_plan_execute`、`plan_then_confirm` 还是 `specialized_surface`，以及为什么。

## 10. 后端整改设计

### 10.1 当前问题

- `commands/agent.rs` 体量过大，聚合了 runtime、memory、provider、permissions、learning 多个责任。
- 主命令层正在成为新的 orchestrator，实现上绕过了原本设计约束。
- 某些持久层和文件系统遍历路径仍然建立在“环境总是干净”的假设上。

### 10.2 目标后端结构

建议把 Tauri 后端主路径拆成：

- `commands`
  - IPC 输入输出、序列化、错误映射、activation commands
- `application`
  - turn service、session service、settings service、harness service、activation service、license lifecycle service、request intelligence service
- `runtime`
  - canonical contracts、conversation runtime、stream orchestration、prompt planning、activation contracts、execution-mode contracts
- `control_plane`
  - permission、audit、tool execution broker、context resolve、boundary、ingress classifier
- `workers`
  - 统一工具执行后端抽象、typed execution input/output/failure、registry、event emitter
- `memory_learning`
  - retrieval、pinned、compiler、summary、ticker、injection、reflection、trajectory feedback
- `observability`
  - harness、telemetry、trace export、metrics aggregation

### 10.3 关键工程原则

1. IPC 命令不可承载主业务编排。
2. filesystem / JSON / model provider 都视为不可信边界。
3. memory 与 harness 必须通过接口接入 runtime，不允许反向把 runtime 埋进子模块。
4. 所有策略行为要有可记录输入和输出。
5. activation / deactivation / revoke / refresh 同样必须进入可观测和可审计链路。
6. execution mode、risk、complexity、route hint 必须由后端统一产出，前端只允许投影与手动覆写调试。
7. worker 只作为统一工具执行后端抽象引入，不提前引入重型 background worker runtime。

## 11. Memory 与自我进化设计

### 11.1 目标定义

“更智能”在 If2Ai 里不应只等于“回答更长”或“上下文更多”，而应等于：

- 更好地记住用户偏好、项目事实和长期目标
- 更少重复犯错
- 更稳定地完成多步任务
- 在可控前提下逐步改善策略

### 11.2 目标 memory 叙事

目标结构应收敛为：

1. `prepare_context`
2. retrieval + rerank
3. multi-layer memory payload
4. `after_turn`
5. write policy
6. quality gate
7. compile / reflect / offline evolution

### 11.3 自我进化边界

自我进化不应等于在线自改 prompt，而应是：

1. trajectory 记录
2. failure clustering
3. reflection synthesis
4. strategy candidate generation
5. harness eval
6. gated promotion

没有经过 Experience Plane 验证的策略，不得成为默认策略。

## 12. Harness 治理设计

### 12.1 Harness 角色升级

Harness 必须从“trace recorder”升级为“治理系统”：

1. 记录 run
2. 回放 run
3. 对 prompt / policy / memory 变化做对比评测
4. 给出是否能默认启用某个策略的发布依据

### 12.2 Harness 的治理对象

1. prompt plan
2. memory recall/write decisions
3. policy decisions
4. run outcome
5. recovery quality
6. cost / latency / tool misuse

## 13. 目标前后端架构

### 13.1 目标后端结构

建议 If2Ai 后端重构为六层：

1. `commands`
2. `application`
3. `runtime`
4. `control_plane`
5. `memory_learning`
6. `observability`

其中 `application + control_plane` 需要正式纳入 `request intelligence / ingress classifier / execution mode routing`。

### 13.2 目标前端结构

建议 If2Ai 前端重构为五层：

1. `boot`
2. `shell`
3. `runtime-projection`
4. `features`
5. `transport`

其中 `runtime-projection` 必须承担 `execution_mode / risk / complexity / route_hint / classifier evidence` 的可解释投影。

## 14. 前后端重构顺序

### 14.1 Phase M0：真相收口

目标：先让系统知道自己现在是什么。

后端：

1. 定义 canonical runtime contract v1。
2. 定义 canonical domain model。
3. 把 `agent.rs` 中与 prompt/memory/stream 相关的领域结构先抽成独立模块，不改行为。
4. 定义 activation contract v1，明确激活、刷新、吊销、反激活事件和状态机。
5. 定义 execution-mode contract v1，明确四类场景、risk、complexity、route hint、reason codes。

前端：

1. 定义 runtime event envelope TS 类型。
2. 建立 translator 层，允许旧事件继续接入。
3. 梳理 `App.tsx` 当前负责的启动、项目、会话、stream 责任清单。
4. 梳理 startup / onboarding / activation gate 三段式 boot state。
5. 梳理 chat 主入口如何接入 execution-mode projection，而不是把场景判断散落在组件中。

验收：

- 文档与代码对同一组实体、同一组 event 使用相同命名。

### 14.2 Phase M1：后端先拆 command god-file

目标：把主脑从 command 层迁走。

后端顺序：

1. `agent.rs` -> `application/turn_service.rs`
2. `prompt assembling` -> `runtime/prompt_plan.rs`
3. `memory injection` -> `memory_learning/injection_service.rs`
4. `provider client resolving` -> `application/provider_service.rs`
5. `stream payload emission` -> `runtime/stream_emitter.rs`
6. `task classifier / ingress route` -> `application/request_intelligence_service.rs`
7. `tool execution backend abstraction` -> `workers/contract.rs + registry.rs + prepare_step_execution seam`

前端只做兼容，不做大改。

验收：

- `commands/agent.rs` 只负责 IPC 输入输出和调用 service。

### 14.3 Phase M2：前端建立投影层

目标：让 React 不再直接消费原始 runtime 细节。

前端顺序：

1. 从 `tauri.ts` 拆出 `contracts.ts`
2. 新建 `runtime-event-translator.ts`
3. 新建 `runtime-event-reducer.ts`
4. 新建 `session-store` / `run-store` / `memory-store`
5. 让 `ChatWorkspace` 改读 store projection
6. 建立 `AppBootGate.tsx`，把 activation gate 从主 UI 逻辑中独立出来
7. 新建 `execution-mode-store`，统一投影 classifier/evidence/explainability

后端同步：

1. 保持旧 payload 可用
2. 增发 canonical envelope
3. 新增 activation commands 与远端 activation client 封装
4. 增发 execution-mode / classifier-evidence 字段

验收：

- `App.tsx` 不再直接承载大段 stream 业务逻辑。
- `chat-ui.tsx` 不再直接理解多种 runtime 原始事件。

### 14.4 Phase M3：Memory 变成单协调器系统

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

### 14.5 Phase M4：Policy 与 Harness 联动

目标：让治理系统真正接管演进。

后端顺序：

1. 引入 `prepare_step_execution`
2. 用 boundary/permission/sandbox 重构工具执行前链路，并让高价值工具接到统一 worker execution backend
3. harness 记录 prompt plan、policy decision、memory decision、worker decision、run outcome
4. 把 replay/eval 接入发布门

前端顺序：

1. Settings 中展示可解释的 policy 状态，而不是只有 mode
2. Harness/Diagnostics 页展示 run eval、failure cluster、strategy compare
3. Account/Settings 中提供显式反激活、解绑设备、license 状态与最近 refresh/revoke 记录
4. Chat/Inspector/Developer Settings 中展示 execution mode、classifier reason、matched rules、manual override

验收：

- 新策略没有 eval 通过就不能升默认。
- license 失效、吊销、反激活后，系统必须可靠回流 activation gate。
- execution mode 必须在 UI 可解释，并能被 harness 回放验证。

### 14.6 Phase M5：自我进化闭环

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

## 15. 90 天路线图

### Phase A：可信基线

1. 修复测试/文档/harness 启用/持久层鲁棒性问题。
2. 建立 canonical domain model 与 workflow truth。
3. 建立 activation contract 与三段式 boot truth。
4. 建立 execution-mode contract 与四场景自动匹配 truth。
5. 开始 `M0`。

### Phase B：结构收口

1. 完成 `M1` 与 `M2`。
2. 拆掉 3 个最大 God-file。
3. 建立前端投影层。

### Phase C：memory 与治理收口

1. 完成 `M3` 与 `M4`。
2. 让 prompt、policy、memory 都进入 harness 评测口径。

### Phase D：自我进化闭环

1. 完成 `M5`。
2. 让策略提升走“候选 -> 评测 -> 放量”闭环。

## 16. 实施优先级清单

### P0

1. canonical domain model
2. canonical runtime contract
3. `commands/agent.rs` service 化拆分
4. React runtime event translator/reducer
5. workflow truth 文档
6. activation contract + activation gate boot state
7. execution-mode contract + classifier evidence projection

### P1

1. prompt plan
2. policy coordinator
3. memory coordinator
4. run-store / memory-store / approval-store
5. seeded fake state 清理
6. request intelligence / execution-mode routing

### P2

1. harness 评测门
2. reflection feedback
3. strategy compare
4. automation / project / memory workspace 深化

## 17. 风险与防错

### 17.1 主要风险

1. 一边重构一边继续加 feature，导致迁移永远无法收口。
2. 只拆文件不拆边界，最后只是把 God-file 分裂成多份耦合文件。
3. 先重做 UI 壳层，后端真相仍旧混乱，导致前端再次背负编排责任。
4. 过早上 self-evolution，结果只是 prompt 漂移和行为随机化。

### 17.2 约束规则

1. 每个迁移 phase 必须先定义 contract，再写 feature code。
2. 每个 phase 必须有 harness slice 验证。
3. 任何新增“智能增强”都必须经过 replay/eval。
4. 若新设计不能减少 `App.tsx` / `chat-ui.tsx` / `agent.rs` 的责任，就不算成功。
5. 若 activation 仍只是本地页面状态，而不是远端 license lifecycle + gate 控制，也不算成功。
6. 若四类聊天执行场景仍由前端散乱判断，而不是 runtime classifier 统一产出，也不算成功。

## 18. 最终建议

If2Ai 下一阶段不该把 UClaw 当作模板工程，而应该把它当作一个已经证明以下命题成立的参考系：

1. 系统可以先有一套诚实的领域真相。
2. 系统可以把 workflow 真相写清楚。
3. 系统可以把 prompt、policy、memory、runtime event 都变成正式契约。
4. 系统可以让前端主要承担“投影与控制”，而不是“隐性编排”。

对 If2Ai 而言，真正的迁移目标不是“更像 UClaw”，而是：

> 让 If2Ai 形成一套自己的 canonical domain model、canonical workflow truth、canonical runtime contract，并让 harness 成为 memory 与自我进化的治理中枢。

只有做到这一点，If2Ai 才适合继续往“更智能的智能体”推进，而不是继续在功能堆叠中扩大复杂度。

# UClaw 对比 if2Ai：Staff 级 Gap 迁移审计

> 最后更新: 2026-04-21
> 审计范围:
> - UClaw backend: `/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs`
> - UClaw frontend: `/Users/ryanliu/Documents/iClaw/UClaw/UClawApp`
> - if2Ai current codebase: `/Users/ryanliu/Documents/IfAI/if2Ai`

## 1. 执行结论

这次对比后的结论很明确:

1. if2Ai 当前最大问题不是“功能少”，而是没有形成像 UClaw 那样的单一执行真相。
2. 我们真正要移植的不是 UClaw 的全部代码，而是它已经验证过的 9 个核心架构设计。
3. 如果不先补这些主干模块，而继续沿着“文档 -> backlog -> exec-plan YAML -> Cursor 执行”这条老 loop 往前推，AI 仍然会在多份文档和多条半成品链路之间迷失目标。
4. 迁移优先级必须按“主链路闭环能力”排序，而不是按页面、功能点、设置页或工具数量排序。

一句话总结:

**UClaw 已经实现的是一套可闭环的运行时架构；if2Ai 目前实现的更多是 seam、projection、governance 和 placeholder。真正的 Gap 是运行时主脊柱没有收束。**

## 2. 审计判断标准

本审计按下面四类判断每个迁移点:

- `P0 Must Port`: 不移植，这一轮就不会形成可闭环产品能力
- `P1 Should Port`: 强烈建议移植，但可以在 P0 稳住后并行推进
- `P2 Adapt Carefully`: 设计值得移植，但不能照抄，需要按 Tauri + React + Rust 现状重构
- `P3 Defer`: UClaw 有价值，但在 if2Ai 当前阶段不应优先投入

## 3. 核心结论：真正要迁移的 9 个模块

## 模块 01: Canonical Chat Execution Spine

- 优先级: `P0 Must Port`
- UClaw 参考:
  - `uclaw-rs/src/engine/facade.rs`
  - `uclaw-rs/src/runtime/contracts.rs`
  - `UClawApp/UClaw/UClaw/Core/Store/AppStore+ChatOrchestration.swift`
- if2Ai 当前对应:
  - [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
  - [src-tauri/src/modules/application/turn_service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/turn_service.rs:1)
- 当前 Gap:
  - `agent.rs` 仍是 4058 行主神文件，真实编排还在这里。
  - `TurnService` 明确写着它不拥有 runtime construction、tool loop、stream emission、session restoration。
  - 这意味着 if2Ai 还没有真正形成“唯一 chat turn 编排入口”。
- Staff 审核判断:
  - 这是 if2Ai 的头号结构性缺口。
  - 没有 canonical turn spine，后面的 memory、policy、projection、harness 都只能是并行旁路，不可能变成产品真相。
- 迁移目标:
  - 建立 `ChatTurnOrchestrator` 或 `EngineFacade` 等价层，收口:
  - provider resolution
  - prompt plan
  - memory prepare/after_turn
  - tool preflight/policy
  - runtime start/stream/recovery
  - terminal persistence and run report
- 不要照抄的部分:
  - 不要直接复制 UClaw 的文件结构或 Swift 端 store 命名。
  - 要移植的是“一个 turn 只有一个编排器”这个原则。

## 模块 02: Execution Mode Routing + Policy Enforcement

- 优先级: `P0 Must Port`
- UClaw 参考:
  - `uclaw-rs/src/policy/coordinator.rs`
  - `uclaw-rs/src/runtime/contracts.rs`
- if2Ai 当前对应:
  - [src-tauri/src/modules/application/request_intelligence_service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/request_intelligence_service.rs:1)
  - [src-tauri/src/modules/control_plane/prepare_step_execution.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/prepare_step_execution.rs:1)
  - [src/modules/execution-mode/ExecutionModePill.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/execution-mode/ExecutionModePill.tsx:1)
- 当前 Gap:
  - request intelligence 只是 advisory。
  - `prepare_step_execution` 明确是 skeleton/shadow mode，不阻断调用者。
  - 前端 `ExecutionModePill` 只是“判定展示”，自己也明确写了不会 auto-route。
- Staff 审核判断:
  - if2Ai 当前其实没有 execution-mode system，只有 execution-mode diagnostics。
  - 这就是用户体感“什么都没实现”的核心原因之一，因为系统会说自己会判断，但不会真的改变执行路径。
- 迁移目标:
  - 把 execution mode 从“分类结果”升级成“真实路由器”。
  - 把 `boundary -> permission -> sandbox` 从 trace seam 升级成真实 gate。
  - 明确:
  - 哪些请求直接执行
  - 哪些请求先 plan 再确认
  - 哪些请求必须走 specialized surface
  - 哪些请求必须停在 approval
- 推荐交付物:
  - 可执行 `ExecutionModeRouter`
  - 可阻断 `StepExecutionCoordinator`
  - 前端不再只是 pill，而是 route outcome 的 projection UI

## 模块 03: Runtime Event Canonical Envelope + Run State Projection

- 优先级: `P0 Must Port`
- UClaw 参考:
  - `uclaw-rs/src/runtime/contracts.rs`
  - `uclaw-rs/src/run_state/projection.rs`
  - `UClawApp/UClaw/UClaw/Core/RuntimeEvents/RuntimeEventProcessor.swift`
  - `UClawApp/UClaw/UClaw/Core/Store/AppStore+EventProcessing.swift`
- if2Ai 当前对应:
  - [src/runtime-projection/runtime-projection-bridge.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/runtime-projection/runtime-projection-bridge.ts:1)
  - [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
  - [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- 当前 Gap:
  - runtime projection bridge 明确写着它和老 chat listeners “并行运行”，不是主真相。
  - 事件翻译、run state、chat render 目前没有被一个统一状态机接管。
  - `App.tsx` 与 `chat-ui.tsx` 仍然超大，说明 UI 编排和 runtime 语义仍然耦合。
- Staff 审核判断:
  - UClaw 真正先进的地方，不只是 event schema，而是 “event -> processor -> store -> projection -> UI” 这条链路已经闭环。
  - if2Ai 现在只有 event schema 和部分 projection store，没有完成“前端由 canonical projection 驱动”这一步。
- 迁移目标:
  - 建立唯一 runtime event ingestion pipeline。
  - 所有 turn state、tool state、approval state、memory state 都通过同一 reducer/projection 收束。
  - 让 chat UI 从 canonical run state 读，不再双写或并行监听。

## 模块 04: Prompt Planning with Traceability

- 优先级: `P0 Must Port`
- UClaw 参考:
  - `uclaw-rs/src/prompt/planner.rs`
- if2Ai 当前对应:
  - [src-tauri/src/modules/application/prompt_planner.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/prompt_planner.rs:1)
- 当前 Gap:
  - if2Ai 的 prompt planner 还是 skeleton。
  - 缺少 UClaw 那种 `trace_id`、`block_hash`、diagnostics、plan identity。
  - 目前更多是“把 prompt 组起来”，不是“把 prompt 变成可审计资产”。
- Staff 审核判断:
  - 没有 prompt traceability，就无法真正做 replay、compare、regression diagnosis。
  - 后面 harness 想做评估，也会因为 prompt plan 不可追踪而变成半盲。
- 迁移目标:
  - PromptPlan 成为真实 contract，而不是中间结构体。
  - 每个 block 有稳定 identity。
  - 整体 plan 有 hash、trace id、diagnostic summary。
  - 运行结果、memory 注入、tool availability 都能关联回 prompt plan。

## 模块 05: Real Memory Lifecycle

- 优先级: `P0 Must Port`
- UClaw 参考:
  - `uclaw-rs/src/memory/memory_manager.rs`
- if2Ai 当前对应:
  - [src-tauri/src/modules/application/memory_coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_coordinator.rs:1)
  - [src-tauri/src/modules/application/memory_recall_assembler.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_recall_assembler.rs:1)
  - [src-tauri/src/modules/application/memory_write_policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_write_policy.rs:1)
- 当前 Gap:
  - `MemoryCoordinator.after_turn` 明确写着不做 persistence。
  - recall 6 槽结构虽然有了，但 critical facts / preferences 仍然没有真实填充。
  - write policy 多数仍是 skeleton/shadow 形态。
- Staff 审核判断:
  - if2Ai 当前是“memory governance shape 已经出现，但 memory product loop 没有闭环”。
  - 这意味着 memory 仍然偏向 trace/gate artifact，而不是真正可用的 agent memory。
- 迁移目标:
  - 完整闭环:
  - prepare_context 真 recall
  - after_turn 真 decision
  - persistence 真写入
  - conflict resolution 真更新
  - recall ranking 真影响下一轮
- 推荐交付物:
  - `MemoryManager` 或等价统一生命周期服务
  - recall/write/quality/conflict/persist 一个闭环里完成

## 模块 06: Frontend AppStore / Shell Truth

- 优先级: `P1 Should Port`
- UClaw 参考:
  - `UClawApp/UClaw/UClaw/Core/Store/AppStore.swift`
  - `UClawApp/UClaw/UClaw/App/AppState.swift`
  - `UClawApp/UClaw/UClaw/Core/Store/AppStore+EventProcessing.swift`
- if2Ai 当前对应:
  - [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
  - [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
  - [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
  - [src/boot/ActivationGateOverlay.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/boot/ActivationGateOverlay.tsx:1)
- 当前 Gap:
  - 前端虽然已经开始切模块，但主要 shell truth 还没有收口。
  - activation overlay 自己明确说是 placeholder。
  - diagnostics page 是治理页，不是产品闭环页。
- Staff 审核判断:
  - UClaw 前端值得移植的不是 SwiftUI 写法，而是 AppStore 作为 shell truth 的组织方法。
  - if2Ai 只有在 runtime state 真正接管前端后，才能停止“页面自己解释事件”的局面。
- 迁移目标:
  - 建立前端统一 shell store。
  - 把 activation、session、run、approval、memory badge、timeline、composer status 全部投影到统一 store。
  - 页面只消费 projection，不自己理解 runtime payload。

## 模块 07: Worker / Tool Execution Contract

- 优先级: `P1 Should Port`
- UClaw 参考:
  - `uclaw-rs/src/workers/*`
  - `uclaw-rs/src/runtime/contracts.rs`
- if2Ai 当前对应:
  - `docs/staff-remediation/if2ai-worker-adoption-design.md`
  - [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)
  - [src-tauri/src/modules/tools/registry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/tools/registry.rs:1)
- 当前 Gap:
  - if2Ai 有 worker adoption 设计，但真正 worker contract / sandbox / capability/risk execution 还没有接成产品主链路。
  - 许多工具仍然沿用历史执行语义。
- Staff 审核判断:
  - 这是第二层主干，不是第一层。
  - 必须接在 canonical chat spine 和 policy gate 之后做，否则只是把旧混乱执行路径再包一层名词。
- 迁移目标:
  - 每个 tool/worker 有稳定 contract:
  - capability
  - required permission
  - sandbox profile
  - approval behavior
  - event emission shape

## 模块 08: Harness Replay / Eval on Canonical Run Report

- 优先级: `P1 Should Port`
- UClaw 参考:
  - UClaw run history / lineage / store 体系
  - `CURRENT_WORKFLOWS.md` 中对 replay/closure 的诚实描述
- if2Ai 当前对应:
  - [src-tauri/src/modules/harness/run_report.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/run_report.rs:1)
  - [docs/staff-remediation/if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1)
- 当前 Gap:
  - harness 在 if2Ai 很强，但建立在大量 skeleton/seam truth 之上。
  - truth 文档已明确:
  - `harness_replay` 尚未建立
  - `harness_eval` 尚未建立
- Staff 审核判断:
  - harness 不是不能做，而是现在做得太前置了。
  - 没有 canonical run spine、prompt trace、memory lifecycle，harness 再丰富也只能评估半成品。
- 迁移目标:
  - replay / eval 必须建立在 canonical run report 上。
  - report 必须能回放:
  - prompt plan
  - event stream
  - tool decisions
  - memory decisions
  - final outcome

## 模块 09: Activation / License / Deactivation Lifecycle

- 优先级: `P2 Adapt Carefully`
- UClaw 参考:
  - UClaw 的 shell / activation 生命周期设计思路
- if2Ai 当前对应:
  - [src/boot/ActivationGateOverlay.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/boot/ActivationGateOverlay.tsx:1)
  - `docs/staff-remediation/gap-modules/activation-license-lifecycle/*`
- 当前 Gap:
  - overlay 有了，但后端 remote lifecycle 还是 skeleton。
  - 目前更像 onboarding/本地状态 gate，不是真正 license system。
- Staff 审核判断:
  - 这件事要做，但绝对不该排在 runtime spine 之前。
  - 如果主链路还没收口，license 再完整也只是挡在一个半成品前面。
- 迁移目标:
  - 等 runtime spine 稳定后，再补:
  - activation request
  - redeem
  - refresh
  - revoke check
  - deactivate

## 4. 明确不建议“整包照抄”的内容

下面这些东西我不建议直接从 UClaw 原样移植:

- SwiftUI 目录结构和命名
- AppStore 的 Swift actor / MainActor 具体实现方式
- UClaw 的产品边界里还没闭环的 Projects / Automation / Inbox 表层能力
- 任何只是因为 UClaw 里存在、但还没证明对 if2Ai 当前主链路有价值的附属界面

原因很简单:

UClaw 值得学的是已经被实践证明有效的系统骨架，不是所有表面功能和语言层写法。

## 5. 推荐的迁移顺序

我建议按下面顺序做，不要跳:

1. `Migrate-01` Canonical Chat Execution Spine
2. `Migrate-02` Execution Mode Routing + Policy Enforcement
3. `Migrate-03` Runtime Event Envelope + Projection Truth
4. `Migrate-04` Prompt Planning with Traceability
5. `Migrate-05` Real Memory Lifecycle
6. `Migrate-06` Frontend AppStore / Shell Truth
7. `Migrate-07` Worker / Tool Contract
8. `Migrate-08` Harness Replay / Eval on Canonical Run Report
9. `Migrate-09` Activation / License Lifecycle

这里的关键不是“先后好看”，而是依赖关系:

- 先有 chat spine，route/policy 才能真生效
- 先有 canonical events，前端 projection 才能变成真相
- 先有 prompt trace + memory lifecycle，harness replay/eval 才有意义

## 6. 从 Staff 视角给你的最终判断

如果只能用一句话来定义这次审计结果:

**if2Ai 当前真正缺的，不是更多文档，也不是更多治理页，而是把 UClaw 已经跑通的“单一运行时真相”完整迁移过来。**

所以接下来最正确的动作不是继续扩 backlog，而是:

1. 以这 9 个模块为唯一迁移清单
2. 每个模块只保留一份 design doc
3. 每个模块只保留一份 implementation pack
4. 每个 pack 必须绑定具体代码入口、验收信号、替换旧链路的 cutover 条件

这样 Cursor 才不会再在“很多文档都对，但没有一条是执行真相”的状态里失焦。

# If2Ai Prompt Control Plane And Coordinator Implementation

> 文档类型：Implementation Design Doc  
> 状态：Draft  
> 创建日期：2026-04-22  
> 关联 Pack：`FEAT-PCP-001` ~ `FEAT-PCP-005`

---

## 1. Executive Summary

If2Ai 需要从“已有 prompt planner”升级为“真正的 prompt control plane”。

当前系统已经具备：

1. `runtime/prompt` 作为 base system scaffold
2. `application/prompt_planner` 作为 turn-level block assembler
3. `identity` 作为 `Soul / Persona` 领域层
4. `execution_mode + scenario_profile_hint` 作为 runtime/surface 分层
5. `memory` 与 `tool routing` 的独立注入链路

但它还缺少一层真正统一的控制逻辑：

- 谁决定某轮该注入哪些 prompt layers
- 不同场景下该激活哪些 catalog entries
- tool / utility / coordinator prompts 怎样按条件进入主 prompt 或旁路任务
- diagnostics 如何解释 prompt activation

因此，本设计提出一个新的平台能力：

`PromptCoordinator`

它将位于 `TurnService` 与 `PromptPlanner` 之间，负责：

1. 读取 turn context
2. 选择 prompt catalogs / overlays
3. 生成 `PromptAssemblyDecision`
4. 输出给 planner 的标准化 contributions

---

## 2. Architectural Positioning

### 2.1 当前真实拓扑

当前主链路大致是：

`TurnService -> request intelligence / memory -> PromptPlanner -> PromptPlan`

这条链路里，`PromptPlanner` 的职责本质上是：

- 组装
- 排序
- 诊断
- 渲染

它不应该承担越来越多的“决策”职责。

### 2.2 新拓扑

建议升级为：

`TurnService -> PromptCoordinator -> PromptPlanner -> PromptPlan`

职责分离如下：

- `TurnService`
  - 负责 turn orchestration
  - 负责收集 runtime context
  - 调用 coordinator

- `PromptCoordinator`
  - 负责 prompt activation policy
  - 负责 catalog 选择
  - 负责生成 `PromptAssemblyDecision`

- `PromptPlanner`
  - 负责 block 组装
  - 负责排序
  - 负责 diagnostics
  - 负责渲染

---

## 3. Core Domain Model

### 3.1 PromptAssemblyLane

建议定义闭合集合：

```rust
pub enum PromptAssemblyLane {
    BaseSystem,
    Identity,
    Scenario,
    ToolPolicy,
    Memory,
    Utility,
    Coordinator,
}
```

作用：

- diagnostics 解释 prompt 来自哪个 lane
- policy engine 决定哪些 lanes 激活
- UI control panel 映射到这些 lanes

### 3.2 PromptCatalogEntry

建议统一所有 catalog entry 的基础接口：

```rust
pub struct PromptCatalogEntry {
    pub id: String,
    pub lane: PromptAssemblyLane,
    pub title: String,
    pub priority: i32,
    pub is_sensitive: bool,
}
```

它不是最终 prompt 文本，而是可被渲染的 entry。

### 3.3 PromptAssemblyDecision

这是协调器的核心输出：

```rust
pub struct PromptAssemblyDecision {
    pub activated_entries: Vec<ActivatedPromptEntry>,
    pub suppressed_entries: Vec<SuppressedPromptEntry>,
    pub activation_reasons: Vec<PromptActivationReason>,
}
```

它要回答：

1. 激活了哪些 prompt entry
2. 为什么激活
3. 哪些 entry 被抑制
4. 最终生成了哪些 contributions

### 3.4 ActivatedPromptEntry

```rust
pub struct ActivatedPromptEntry {
    pub entry_id: String,
    pub lane: PromptAssemblyLane,
    pub source: String,
    pub contribution_kind: String,
}
```

### 3.5 PromptActivationReason

```rust
pub struct PromptActivationReason {
    pub entry_id: String,
    pub reason_code: String,
    pub detail: String,
}
```

例如：

- `scenario_profile_planning`
- `resolved_identity_present`
- `web_tool_family_available`
- `complex_orchestration_required`

---

## 4. Catalog Structure

建议把 prompt catalog 分成 5 大类。

### 4.1 ScenarioPromptCatalog

负责：

1. `chat`
2. `coding`
3. `research`
4. `planning`
5. `review`

说明：

- 这些是产品层场景模板
- 不应该和 `Soul / Persona` 混淆
- 不应该和 `execution_mode` 混淆

### 4.2 ToolPromptCatalog

建议首批支持 family：

1. `web`
2. `file`
3. `memory`
4. `ask_user`

原则：

- 按 family 激活
- 按场景和任务 phase 条件注入
- 不做所有 tool prompts 常驻注入

### 4.3 UtilityPromptCatalog

首批建议支持：

1. `session_title`
2. `tool_summary`
3. `away_recap`
4. `next_action_suggestion`

这些通常不是主聊天 turn prompt，而是 utility lane 的单用途 prompt。

### 4.4 CoordinatorPromptCatalog

作用：

- 在复杂 orchestration turn 中注入 planning / delegation / verification 约束

它更接近外部参考仓库的 `coordinator-prompt.md`。

重要原则：

- 不是所有 turn 常驻
- 只在复杂任务、multi-step、workflow-heavy 场景激活

### 4.5 IdentityPromptCatalog

这层当前可以继续复用已有：

- `render_soul_block`
- `render_persona_block`

未来如需扩展为 catalog，也应保留：

- `Soul` 是长期身份
- `Persona` 是交互投影

---

## 5. Activation Policy

### 5.1 输入源

协调器至少应读取：

1. `ResolvedIdentity`
2. `ScenarioProfileHint`
3. `ExecutionModeDecision`
4. registered tools
5. memory injection state
6. request text
7. session metadata

### 5.2 基础激活规则

#### Identity lane

- 若存在 `ResolvedIdentity`，激活 `Soul`
- 若存在 `persona_id`，额外激活 `Persona`

#### Scenario lane

- 若存在 `ScenarioProfileHint`，激活对应 scenario entry
- 若无 hint，则可根据 `PromptBuildMode` fallback

#### ToolPolicy lane

- 若当前工具注册表存在 family 且当前 turn 属于相关场景，则激活

例子：

- `research + web tools available` -> `web_research_policy`
- `coding + file tools available` -> `file_edit_policy`

#### Coordinator lane

在这些条件下激活：

1. `ExecutionMode::PlanThenConfirm`
2. `ExecutionMode::AutoPlanExecute` 且 complexity 高
3. `scenario = planning / review` 且用户问题明显多阶段

#### Utility lane

只在 utility task 上激活，不进入普通聊天主 prompt

---

## 6. Injection Strategy

### 6.1 主 turn prompt

进入 `PromptPlan.blocks` 的主要是：

1. base system
2. identity
3. scenario
4. selected tool policy
5. memory
6. optional coordinator overlay

### 6.2 旁路 utility prompts

像 `session_title` / `tool_summary` 这种，建议：

- 不进主 turn prompt
- 走 utility lane 的独立调用

### 6.3 条件式而非全量式注入

核心原则：

- 不是 catalog 里有的都进 prompt
- 而是由 coordinator 选择最小必要集合

这既避免 token 浪费，也避免 prompt 互相打架。

---

## 7. Diagnostics

当前 `PromptPlanDiagnostics` 只记录：

1. block kinds
2. count
3. redacted preview
4. validation issues

建议升级为还能记录：

1. activated catalog entry ids
2. suppressed entry ids
3. activation reasons
4. lane-level summary

例如：

```json
{
  "activatedEntries": [
    "identity:soul@if2ai-core",
    "identity:persona@staff-architect",
    "scenario:planning",
    "tool:web-family",
    "coordinator:complex-plan"
  ]
}
```

---

## 8. Prompt Control Panel

### 8.1 首版用户可控项

建议暴露：

1. `Default Soul`
2. `Default Persona`
3. `Default Scenario Profile`
4. `Prompt Diagnostics Enabled`

### 8.2 高级项

建议放高级设置：

1. `Coordinator Overlay Auto`
2. `Tool Policy Strictness`
3. `Memory Prompt Verbosity`

### 8.3 首版不暴露

不建议给普通用户直接暴露：

1. base system prompt 原文编辑
2. tool prompt 原文编辑
3. memory extraction prompt 原文编辑
4. coordinator markdown 直接编辑

---

## 9. Codebase Mapping

### 9.1 建议新增模块

```text
src-tauri/src/modules/application/
├── prompt_coordinator.rs
└── prompt_catalog/
    ├── mod.rs
    ├── scenario.rs
    ├── tool.rs
    ├── utility.rs
    └── coordinator.rs
```

### 9.2 现有模块协作关系

- `turn_service/mod.rs`
  - 调 coordinator

- `prompt_coordinator.rs`
  - 读 identity / scenario / execution mode / tool registry
  - 产出 decision + contributions

- `prompt_planner/*`
  - 接受 contributions
  - 做最终 block 组装

---

## 10. Recommended Implementation Order

### Phase 1

`FEAT-PCP-001`

- 引入 coordinator
- 定义 decision / lane / reason 数据结构
- 接入 turn service

### Phase 2

`FEAT-PCP-002`

- 把 scenario block 提升成 catalog
- 支持 task-focus overlays

### Phase 3

`FEAT-PCP-003`

- 引入 tool prompt family catalog
- 做条件式注入

### Phase 4

`FEAT-PCP-004`

- 引入 utility / coordinator prompt lanes

### Phase 5

`FEAT-PCP-005`

- 接 settings control panel
- 扩展 diagnostics projection

---

## 11. Key Decisions

1. `PromptPlanner` 保持 assembler，不升级成 policy brain
2. `PromptCoordinator` 才是 control plane 的核心调度器
3. `complete-prompts` 的价值主要体现在 catalog 分层，不在具体 wording
4. `tool prompts` 按 family + condition 激活，不做全量注入
5. `coordinator prompt` 是按需 overlay，不做 base prompt 常驻

---

## 12. Bottom Line

If2Ai 现在最需要的，不是再加几段 prompt，而是补齐 prompt 的“调度层”。

只有当我们拥有：

1. catalogs
2. coordinator
3. activation policy
4. diagnostics
5. control panel

这五件事时，If2Ai 的 prompt system 才能真正从“可拼装”升级成“可治理的 prompt operating system”。

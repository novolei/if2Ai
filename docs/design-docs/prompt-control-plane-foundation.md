# If2Ai Prompt Control Plane Foundation

> 文档类型：Design Doc  
> 状态：Draft  
> 创建日期：2026-04-22  
> 关联 Pack：`FEAT-ID-002`、`MIG-006`、`MIG-008`

---

## 1. Executive Summary

If2Ai 现在需要的不是继续堆一个更长的 system prompt，而是建立一个统一的 `prompt control plane`。

这层的目标不是“让用户直接编辑所有 prompt 文本”，而是把 prompt 变成一个可组合、可观测、可扩展的运行时控制系统，统一管理：

1. base system prompt
2. identity prompt (`Soul` / `Persona`)
3. scenario profile prompt（不只 coding）
4. tool policy prompt
5. memory prompt
6. utility prompt
7. coordinator / planner prompt

参考 [repowise-dev/claude-code-prompts complete-prompts](https://github.com/repowise-dev/claude-code-prompts/tree/master/complete-prompts) 后，一个重要结论是：

- 优秀的 Agent prompt 体系不是“一个 prompt”
- 而是“多类 prompt contract 的编排系统”

If2Ai 当前已经有一个很好的起点：

- `runtime/prompt` 负责 static system scaffold
- `application/prompt_planner` 负责 turn-level block composition
- `memory` 已有独立注入链路
- `identity` 已有 `Soul / Persona / ResolvedIdentity` 基础
- `execution_mode` 与 `scenario_profile_hint` 已经分层

因此最适合 If2Ai 当前架构的方向是：

1. 不重写现有 prompt pipeline
2. 在 `PromptPlan` 之上建立统一的 prompt control plane 语义
3. 用结构化 block + contribution contract 承载不同 prompt 类型
4. 把 “coding / research / planning / review / chat” 作为 scenario profile，而不是 system prompt 分叉

---

## 2. Why We Need A Unified Prompt Control Plane

### 2.1 当前问题

当前系统虽然已经有 `PromptPlan`，但仍存在几个结构性问题：

1. `identity` 已经建模，但 prompt 层刚开始接入
2. `PromptBuildMode` 过去偏向 `chat / coding` 二元视角
3. `runtime/prompt/mod.rs` 中的 base 文本仍有明显 coding bias
4. `tool routing` 已有独立 block，但其他 prompt contract 还未系统化
5. `memory` prompt、`utility` prompt、`coordinator` prompt 还没有统一编排语义

### 2.2 如果不建 control plane，会出现什么问题

1. 每加一个场景就新增一份大 prompt 变体
2. identity、memory、skills、tool policy 会相互缠绕
3. prompt 诊断会越来越难解释
4. Settings UI 无法提供稳定的“用户可控项”
5. 后续四个场景计划会演化成多套 prompt 分支，维护成本快速失控

---

## 3. Reference Findings From complete-prompts

参考仓库：

- [system-prompt.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/system-prompt.md)
- [coordinator-prompt.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/coordinator-prompt.md)
- [agent-prompts/general-purpose.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/agent-prompts/general-purpose.md)
- [agent-prompts/solution-architect.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/agent-prompts/solution-architect.md)
- [memory-prompts/memory-extraction.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/memory-prompts/memory-extraction.md)
- [memory-prompts/session-notes.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/memory-prompts/session-notes.md)
- [tool-prompts/web-search.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/tool-prompts/web-search.md)
- [utility-prompts/tool-summary.md](https://raw.githubusercontent.com/repowise-dev/claude-code-prompts/master/complete-prompts/utility-prompts/tool-summary.md)

### 3.1 最值得借鉴的不是 wording，而是“职责分层”

这个仓库最值得借鉴的点不是它写了哪些句子，而是它把 prompt 分成了几类责任：

1. `system prompt`
2. `coordinator prompt`
3. `agent prompts`
4. `memory prompts`
5. `tool prompts`
6. `utility prompts`

这对应的是一个 runtime 编排模型，而不是一个 prompt 模板库。

### 3.2 对 If2Ai 最有价值的启发

1. `system prompt` 负责底层行为契约，不负责所有场景差异
2. `coordinator prompt` 负责多 worker / 多阶段编排心智
3. `agent prompts` 负责角色化 focus，不等于 identity
4. `memory prompts` 是 memory 子系统自己的 contract
5. `tool prompts` 是每类工具的调用规约，不应该混在总 prompt 里
6. `utility prompts` 适合做 session title、tool summary、away recap 之类的小型专用 prompt

---

## 4. Recommended If2Ai Prompt Layering

建议把 If2Ai 的 prompt 分层固定为以下结构：

### 4.1 Layer A: Base System Layer

归属：

- `src-tauri/src/modules/runtime/prompt/mod.rs`

职责：

- 平台共识
- 输出媒介约束
- 权限 / 安全 / sandbox 基础规则
- 环境上下文
- 项目上下文

这个层不应该承担：

- 具体 persona 文案
- 场景差异
- memory extraction 规则
- tool-specific SOP

### 4.2 Layer B: Identity Layer

归属：

- `src-tauri/src/modules/identity/*`
- `application/prompt_planner`

职责：

- `Soul` 定义 Agent 长期身份
- `Persona` 定义当前交互投影
- 通过 `ResolvedIdentity` 注入 turn-level prompt

这层回答的问题是：

- 这个 Agent 是谁
- 这个 Agent 现在如何与用户协作

### 4.3 Layer C: Scenario Profile Layer

建议作为统一的上层产品模板，当前至少支持：

1. `chat`
2. `coding`
3. `research`
4. `planning`
5. `review`

这层不是 `execution_mode`。

语义上：

- `execution_mode` 是 runtime truth
- `scenario_profile` 是 prompt/UX/product template

这层回答的问题是：

- 这次 turn 更像哪一种工作场景
- 在这个场景里，Agent 应该如何组织思考和输出

### 4.4 Layer D: Tool Policy Layer

归属建议：

- `runtime/prompt_tools_guide.rs`
- future `runtime/tool_prompt_catalog.rs`

职责：

- 工具选择顺序
- 何时升级到更重的工具
- 工具结果的注入防护
- 每类工具的 source attribution 规范

现在已有的 `web_tools_routing_block` 就是这个层的雏形。

### 4.5 Layer E: Memory Layer

归属：

- `modules/memory/*`
- `application/memory_*`

职责：

- memory injection
- memory extraction
- consolidation
- session notes
- after-turn memory write guidance

重要原则：

- memory prompt 应归 memory 子系统拥有
- 不应该散落到总 system prompt 中

### 4.6 Layer F: Coordinator / Orchestration Layer

归属建议：

- `application/turn_service`
- future `application/coordinator_prompt.rs`

职责：

- 多阶段流程组织
- plan / execute / verify 的切换约束
- 多 worker 协作规则
- “什么时候自己做，什么时候分派”

这层更接近 `complete-prompts` 里的 `coordinator-prompt.md`。

### 4.7 Layer G: Utility Prompt Layer

归属建议：

- future `runtime/utility_prompt_catalog.rs`

职责：

- session title
- tool summary
- away recap
- next action suggestion

这些 prompt 不属于 chat main prompt，但属于产品行为质量。

---

## 5. Mapping complete-prompts To If2Ai Codebase

| complete-prompts 分类 | If2Ai 对应层 | 推荐归属代码 |
| --- | --- | --- |
| `system-prompt.md` | Base System Layer | `runtime/prompt/mod.rs` |
| `coordinator-prompt.md` | Coordinator Layer | `application/turn_service` + future coordinator prompt module |
| `agent-prompts/*` | Scenario Layer / task-focused overlays | `application/prompt_planner` |
| `memory-prompts/*` | Memory Layer | `modules/memory/*` |
| `tool-prompts/*` | Tool Policy Layer | `runtime/prompt_tools_guide.rs` + future tool prompt catalog |
| `utility-prompts/*` | Utility Layer | future utility prompt catalog |

---

## 6. How To Use `system-prompt.md` And `coordinator-prompt.md`

### 6.1 `system-prompt.md` 该怎么用

建议把它当作“base system contract 的参考蓝本”，而不是照抄文本。

If2Ai 应该吸收的是这些结构：

1. identity & context
2. permission model
3. task execution discipline
4. risk-aware action
5. tool usage protocol
6. communication style

这些内容应该继续落在：

- `runtime/prompt/mod.rs`
- `runtime/prompt_tools_guide.rs`
- system/developer instruction contracts

### 6.2 `coordinator-prompt.md` 该怎么用

建议把它当作 orchestration contract 的蓝本，用于 future coordinator layer，而不是直接塞进所有主对话。

更适合的使用方式：

1. 当 If2Ai 进入多 worker / 多阶段执行模式时启用
2. 或在 `planning / review / complex execution` 场景下作为 planner overlay

不建议：

1. 把 coordinator prompt 永久混入所有 turn
2. 让普通单轮聊天也承担多 worker 编排 token 成本

---

## 7. Prompt Control Plane Product Recommendation

### 7.1 是否应该建统一的 Prompt Control Panel

建议：`要建，但不要做成巨型文本编辑器`

更合理的是一个结构化控制面板，管理以下配置：

1. `Default Soul`
2. `Default Persona`
3. `Default Scenario Profile`
4. `Prompt Diagnostics`
5. `Advanced Prompt Policies`

### 7.2 Prompt Control Panel 最小版应该有哪些项

#### 用户层

1. 当前默认 `Persona`
2. 当前默认 `Scenario Profile`
3. 是否开启 `Prompt Diagnostics`

#### 高级层

1. `Soul`
2. 场景级 prompt profile override
3. memory injection level
4. tool routing strictness
5. coordinator overlay enablement

### 7.3 不建议首版暴露给用户的内容

1. 直接编辑 base system prompt
2. 逐条编辑 tool prompt 文本
3. 逐条编辑 memory extraction prompt
4. 任意拼接 coordinator rules

原因是这些内容更像平台 contract，而不是普通用户设置。

---

## 8. Recommended Scenario Model

用户提到不应只服务 coding，这一点需要被正式写进模型。

建议 If2Ai 统一使用以下 scenario profile：

1. `chat`
2. `coding`
3. `research`
4. `planning`
5. `review`

这五类已经和现有 `ScenarioProfileHint` 对齐，且适合后续产品扩展。

如果产品层后续确实只计划“四个场景”，建议不要在底层裁掉第五个枚举，而是：

- UI 先只开放 4 个
- runtime 仍保留 5 个 canonical scenario profiles

这样可以避免后续再改 wire contract。

---

## 9. Codebase Recommendations

### 9.1 已适合承接 control plane 的模块

1. `runtime/prompt/mod.rs`
2. `application/prompt_planner/mod.rs`
3. `runtime/prompt_tools_guide.rs`
4. `identity/*`
5. `memory/*`
6. `runtime/contracts/execution_mode.rs`

### 9.2 建议新增的模块

1. `application/prompt_planner/scenario.rs`
2. `runtime/tool_prompt_catalog.rs`
3. `runtime/utility_prompt_catalog.rs`
4. `application/coordinator_prompt.rs`
5. `settings` 侧的 prompt control DTO / commands

### 9.3 代码实现建议

#### 当前阶段

1. 先把 `ResolvedIdentity` 真正接入 prompt planner
2. 把 scenario profile 作为独立 block 接入
3. 保持 tool policy 先用已有 `web_tools_routing_block`
4. 文档上明确 memory/tool/utility/coordinator 的 ownership

#### 下一阶段

1. 抽 `PromptControlPlaneConfig`
2. 为 scenario profiles 建 registry
3. 给 utility prompts 建独立 catalog
4. 给 coordinator prompt 建独立 overlay

---

## 10. Immediate Architectural Decisions

### Decision 1

`Soul / Persona` 继续作为 prompt planner block，不回退到 `SystemPromptBuilder`

### Decision 2

`scenario profile` 作为独立 block，而不是把 coding/research/planning 分叉成多个 system prompt 文件

### Decision 3

`complete-prompts` 的 memory/tool/utility/coordinator 采用“分层映射”，不采用“文本照抄”

### Decision 4

首版 prompt control panel 只暴露结构化配置，不暴露原始 prompt 文本自由编辑

---

## 11. Acceptance Criteria

如果 If2Ai 建成统一的 prompt control plane，那么至少应满足：

1. Prompt diagnostics 能解释每个 block 来自哪里
2. identity / scenario / memory / tool policy 可以独立演进
3. coding 不是唯一一等场景
4. 后续引入 research / planning / review 不需要复制一整套 prompt pipeline
5. Settings 可以承接用户可控项，而不是只让 prompt 成为内部魔法

---

## 12. Bottom Line

If2Ai 现在应该做的，不是继续优化“一份 coding system prompt”，而是把 prompt 当成平台控制面来建设。

更具体地说：

- `Soul / Persona` 解决 identity
- `Scenario Profile` 解决多场景
- `Tool Prompt / Memory Prompt / Utility Prompt / Coordinator Prompt` 解决子系统职责分层
- `PromptPlan` 继续作为统一的装配总线

这条路线和当前代码架构是相容的，也是把 `complete-prompts` 的优点吸收进 If2Ai 的最稳方式。

# If2Ai Identity Architecture: Soul / Persona / Memory Foundation

> 文档类型：Feature Design Doc  
> 状态：Draft  
> 适用范围：If2Ai Tauri + Rust runtime + React settings/chat UI  
> 创建日期：2026-04-22  
> 相关 Pack：`FEAT-ID-001` ~ `FEAT-ID-005`
>
> 说明：本设计已被纳入更高一层的 `prompt control plane` 视角，详见
> `docs/design-docs/prompt-control-plane-foundation.md`。

---

## 1. Executive Summary

If2Ai 需要引入一个统一的 Agent identity 架构，以解决以下三个正在收敛到同一个根因的问题：

1. Agent 缺乏稳定且可解释的长期身份模型。
2. 用户需要手动切换不同的交互风格，但当前系统没有结构化 persona 概念。
3. Memory 子系统当前不知道一条记忆“属于哪个 identity”，未来会导致跨风格污染、长期学习失真、回忆语义不稳定。

本设计文档提出一个分层 identity 模型：

- `Soul`：稳定、长期、低频变化的 Agent identity 内核
- `Persona`：在具体会话或任务中采用的交互风格 / 协作姿态
- `ResolvedIdentity`：某一 turn 最终实际生效的 identity 结果

并明确三项实现原则：

1. `Soul` 与 `Persona` 必须是独立的一等对象，不能混成一个大 prompt patch。
2. Prompt 层必须以结构化 block 注入 identity，而不是拼接进单一 system prompt 字符串。
3. Memory 第一阶段不做大改，只做 identity-aware tagging，为后续 retrieval / reflection / consolidation 演进打基础。

本设计同时覆盖：

- 后端 identity 领域模型
- Prompt Planner 集成方式
- Session / Settings / UI 交互模型
- Memory tagging 最小闭环
- Pack 级实施分解

---

## 2. Background And Problem Statement

### 2.1 当前痛点

If2Ai 当前已经具备以下能力：

- 独立的 runtime config / prompt planner / session manager / memory injection 分层
- Prompt planner 的 block 化能力，且已经存在 `PromptBlockKind::Persona` 预留位
- Session 元数据已经支持部分 session-scoped feature toggle，如 `memory_enabled`
- 前后端已有 Settings 页面与 Session API façade

但 identity 语义仍然缺失，具体体现在：

1. Agent 没有稳定 identity 内核  
   当前 prompt 更接近“系统角色 + 工具规则 +运行时上下文”，缺少产品级 identity 建模。

2. 用户无法显式控制“这个 Agent 现在以什么人格/风格工作”  
   虽然已有 output-style 类思路和 prompt contribution 结构，但产品层没有 Soul / Persona 概念。

3. Memory 没有 identity 归属  
   记忆可以被写入、编译、注入，但当前没有稳定的 `soul_id / persona_id` 语义。未来引入多 persona 后，记忆将无法区分：
   - 这是长期 identity 的稳定经验
   - 还是某个会话下的临时交互风格偏好

### 2.2 为什么现在做

现在做这件事的时机合适，原因如下：

1. Prompt planner 已经具备结构化扩展基础。  
   当前 `PromptPlan` / `PromptBlock` / diagnostics / block hash 已经存在，identity block 接入成本可控。

2. Session 模型已支持 session override 模式。  
   `memory_enabled: Option<bool>` 已经证明 session-scoped override 与 global default 的模式可复用。

3. Settings 页与 Session UI 已具备承载入口。  
   用户手动切换默认值与会话级覆盖，属于增量型产品扩展，而不是新 surface 发明。

4. Memory 当前还没有深度 identity-aware behavior。  
   趁着 retrieval / reflection / consolidation 还未过度复杂化，优先补归属 metadata，成本最低。

### 2.3 不解决会发生什么

如果在引入 persona 之前不先建立 identity 分层，后续会出现以下问题：

1. 多 persona 的风格偏好相互污染
2. 长期 memory 把“表达风格”误学成“核心原则”
3. Reflection / self-model 无法区分身份与姿态
4. 无法解释“为什么这个 session 今天像另一个 Agent”
5. 用户在 Settings 里修改人格后，系统行为与 memory 归属不一致

---

## 3. Goals

### 3.1 Product Goals

1. 用户可以在 Settings 页面手动设置默认 `Soul` 和默认 `Persona`
2. 用户可以在当前 Session 中切换 `Persona`
3. 用户可以在高级选项中切换当前 Session 的 `Soul`
4. 切换 identity 后，后续 turn 的 prompt 和 memory tagging 反映新身份

### 3.2 Architecture Goals

1. 建立统一 identity 领域模型：`SoulDefinition`、`PersonaDefinition`、`ResolvedIdentity`
2. 将 Soul / Persona 作为 prompt planner 的结构化 block 注入
3. 建立 global default + session override 的 identity 解析规则
4. 为 memory 写入增加 `soul_id` / `persona_id` tagging
5. 保持现有 memory 生命周期和 retrieval 主流程不被重写

### 3.3 Quality Goals

1. Legacy session 无 identity 字段时仍可正常读取与恢复
2. Legacy memory entry 无 identity 字段时仍可正常使用
3. Prompt diagnostics 可观测当前生效的 Soul / Persona
4. UI 切换与后端实际生效 identity 一致

---

## 4. Non-Goals

本设计第一阶段**明确不做**以下事项：

1. 不做 project-level default identity
2. 不做 request-level identity override
3. 不做用户自定义 Soul / Persona 编辑器
4. 不做 persona marketplace / plugin-distributed persona 首版
5. 不重写 memory retrieval ranking
6. 不重写 reflection / compaction / consolidation 逻辑
7. 不让 Persona 直接控制 permission policy、tool policy、sandbox policy

---

## 5. Design Principles

### 5.1 Soul 与 Persona 分层原则

- `Soul` 表示 Agent “是谁”
- `Persona` 表示 Agent “现在怎么和用户相处”

因此：

- 同一个 `Soul` 可以拥有多个 `Persona`
- `Persona` 必须从属于某个 `Soul`
- `Persona` 不能覆盖 `Soul` 的核心原则

### 5.2 Prompt 层结构化原则

Identity 不得作为一大段自由文本直接塞进 system prompt builder。

必须：

- 进入 `PromptPlan`
- 以独立 block 表示
- 在 diagnostics 中可观测
- 参与 block hash / trace identity

### 5.3 Memory 轻演进原则

第一阶段不做 memory 大改，只做以下最小闭环：

- 写入时带 identity metadata
- 持久化允许 identity metadata 缺省
- retrieval 暂不强制按 identity 做过滤

### 5.4 用户可控原则

Identity 不是纯内部机制。用户必须可以在产品界面中手动控制默认值与会话级覆盖。

---

## 6. Comparative Reference: cc-haha-main

本设计参考了 `/Users/ryanliu/Documents/IfAI/cc-haha-main` 的两个关键模式：

### 6.1 Output Style 独立配置模式

`cc-haha-main` 中：

- `outputStyle` 是独立 settings 字段
- 会被注入为独立 prompt section
- 会被暴露到 session init 元数据

这证明“表达风格层”适合独立建模，而不是与 agent definition 混在一起。

### 6.2 Agent Definition + Memory Scope 模式

`cc-haha-main` 中 agent definition 具备：

- system prompt
- model
- tools
- skills
- permission mode
- memory scope

这说明“稳定 identity / 专长 / 能力边界”更适合归入 agent definition，而不是 output style。

### 6.3 对 If2Ai 的映射结论

对 If2Ai 来说，最合理的借鉴方式是：

- 吸收其 `outputStyle` 独立建模的思想 -> 用于 `Persona`
- 吸收其 `AgentDefinition + memory scope` 的思想 -> 用于 `Soul`
- 不直接复制其“字符串替换型 system prompt 合成方式”，继续使用 If2Ai 自己已有的 `PromptPlan` block 化机制

---

## 7. Domain Model

### 7.1 Core Definitions

建议新增 `src-tauri/src/modules/identity/`，并定义以下核心模型。

#### 7.1.1 SoulDefinition

```rust
pub struct SoulDefinition {
    pub id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub mission: String,
    pub core_principles: Vec<String>,
    pub decision_contract: String,
    pub non_negotiables: Vec<String>,
}
```

字段语义：

- `id`：稳定主键，如 `if2ai-core`
- `version`：identity prompt 内容版本
- `summary`：给 UI 列表和 diagnostics 使用的短说明
- `mission`：长期目标
- `core_principles`：核心原则
- `decision_contract`：判断方式说明
- `non_negotiables`：不可被 Persona 覆盖的底线

#### 7.1.2 PersonaDefinition

```rust
pub struct PersonaDefinition {
    pub id: String,
    pub soul_id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub tone_rules: Vec<String>,
    pub collaboration_rules: Vec<String>,
    pub output_preferences: Vec<String>,
}
```

字段语义：

- `soul_id`：显式绑定所属 Soul
- `tone_rules`：语气/表述规则
- `collaboration_rules`：协作姿态
- `output_preferences`：输出组织偏好

#### 7.1.3 IdentitySettings

```rust
pub struct IdentitySettings {
    pub default_soul_id: Option<String>,
    pub default_persona_id: Option<String>,
}
```

用于 runtime config 对外投影和 Settings UI。

#### 7.1.4 SessionIdentityOverride

```rust
pub struct SessionIdentityOverride {
    pub soul_id: Option<String>,
    pub persona_id: Option<String>,
}
```

#### 7.1.5 ResolvedIdentity

```rust
pub struct ResolvedIdentity {
    pub soul_id: String,
    pub soul_version: String,
    pub persona_id: Option<String>,
    pub persona_version: Option<String>,
    pub source: IdentitySource,
}
```

#### 7.1.6 IdentitySource

```rust
pub enum IdentitySource {
    BuiltInFallback,
    GlobalDefault,
    SessionOverride,
}
```

首版不引入 project/request 级别。

---

## 8. Resolution Rules

### 8.1 Resolution Order

首版解析优先级：

1. `Session override`
2. `Global default`
3. `Built-in fallback`

后续可扩展为：

1. request override
2. session override
3. project default
4. global default
5. built-in fallback

### 8.2 Validation Rules

解析时必须满足：

1. `Soul` 必须存在
2. `Persona` 若存在，必须存在于 registry
3. `Persona.soul_id` 必须与最终解析出的 `Soul.id` 一致

若出现不一致：

- 优先保 Soul
- 丢弃无效 Persona
- 记录 diagnostics warning

### 8.3 Fallback Rules

如果 settings / session 指向不存在的 Soul / Persona：

- Soul fallback 到 built-in default
- Persona fallback 到 `None`
- 记录 warning，不抛 fatal error

---

## 9. Prompt Architecture Integration

### 9.1 Why PromptPlan, Not SystemPromptBuilder

当前 `SystemPromptBuilder` 更适合承载：

- static intro
- system rules
- dynamic environment / config

而 identity 更适合进入 `PromptPlan`，原因是：

1. 可以作为独立 block 出现在 diagnostics
2. 能参与 block hash
3. 能支持结构化插入顺序
4. 能与 memory / strategy overlay 并列建模

### 9.2 New Prompt Block Kinds

在 `PromptBlockKind` 中新增：

```rust
Soul,
Persona,
```

当前 `Persona` 已预留但未打通；需要真正接通。  
`Soul` 需要新增。

### 9.3 Block Ordering

推荐顺序：

1. `System`
2. `Soul`
3. `Persona`
4. `WebToolsRoutingGuide`
5. `ActiveStrategyOverlay`
6. `MemoryInjectionPinned`
7. `MemoryInjectionCompiled`
8. `MemoryInjectionRules`
9. `RetrievedMemory`

### 9.4 Prompt Rendering Contract

#### 9.4.1 Soul Block 示例

```markdown
# Soul: If2Ai Core
- Mission: ...
- Core principles:
  - ...
  - ...
- Decision contract: ...
- Non-negotiables:
  - ...
```

#### 9.4.2 Persona Block 示例

```markdown
# Persona: Staff Architect
- Tone:
  - clear, structured, tradeoff-oriented
- Collaboration:
  - propose phased rollout
  - name risks explicitly
- Output preferences:
  - concise summary first
  - architecture options when consequential
```

### 9.5 Conflict Rule

如果 Soul 和 Persona 内容冲突：

- Soul 优先
- Persona 不可覆盖 `non_negotiables`
- diagnostics 记录冲突 warning

---

## 10. Runtime And Session Integration

### 10.1 Runtime Config

在 runtime settings 中新增：

```json
{
  "identity": {
    "defaultSoulId": "if2ai-core",
    "defaultPersonaId": "staff-architect"
  }
}
```

建议放在统一命名空间下，不要散落在顶层。

### 10.2 Session Persistence

在 session metadata 增加：

- `soul_id: Option<String>`
- `persona_id: Option<String>`

目标：

1. 新 session 可显式保存当前 identity override
2. 恢复 session 后 identity 保持一致
3. 老 session 无此字段时仍可正常解析

### 10.3 Session Semantics

建议语义：

- Session 内切 Persona：常见操作
- Session 内切 Soul：高级操作
- 切换后不 retroactively 修改历史消息
- 仅影响后续 turn 的 prompt 与 memory tagging

---

## 11. Memory Integration

### 11.1 Scope

本设计的 memory 部分只做**identity tagging**，不做 retrieval / compiler / compaction 重写。

### 11.2 Required Metadata

新写入 memory entry 需要带：

- `soul_id: Option<String>`
- `persona_id: Option<String>`
- `session_id: Option<String>`
- `project_id: Option<String>`
- `origin_kind: String`

其中首版最关键的是：

- `soul_id`
- `persona_id`

### 11.3 Why Soul Is Primary, Persona Is Secondary

建议语义：

- `Soul`：长期身份锚点，主归属
- `Persona`：会话/风格标签，辅助标签

因此：

- Reflection / durable learnings 未来应更偏 Soul
- Style preference / local interaction pattern 可以带 Persona 标签

### 11.4 Write Path Changes

所有 after-turn memory write 路径需要拿到 `ResolvedIdentity`，并在写入 entry 时带上 identity metadata。

### 11.5 Read Path Changes

首版不强制改 retrieval 策略，但需要：

- schema 兼容 identity metadata
- query API 预留未来 filter 参数扩展能力

### 11.6 Backward Compatibility

旧 memory entry 若缺少 identity 字段：

- 视为 `None`
- 不影响现有 recall / compile / inject

---

## 12. Settings And UI Design

### 12.1 Global Settings

Settings 页面新增 `Agent Identity` 区块，包含：

1. `Default Soul`
2. `Default Persona`

交互规则：

- Persona 列表按当前 Soul 过滤
- 改动写入 runtime settings
- 对新 session 自动生效

### 12.2 Session-Level Controls

聊天页 / session header 建议新增 identity 切换入口。

#### 12.2.1 Persona

- 直接可切换
- 文案提示“仅影响当前会话后续回复”

#### 12.2.2 Soul

- 放高级选项
- 切换前确认
- 文案提示“切换 Agent 核心身份，后续记忆将按新 Soul 记录”

### 12.3 UX Constraints

1. 不要让普通用户一开始就被 Soul 概念淹没
2. Persona 要比 Soul 更容易找到、更容易切换
3. 当前生效 identity 应可见，但不要造成 UI 噪音

建议：

- 常驻显示 Persona badge
- Soul 放在 tooltip 或 expanded details

---

## 13. API And Contract Changes

### 13.1 New Backend Commands

建议新增：

- `identity_list_souls`
- `identity_list_personas`
- `identity_get_defaults`
- `identity_set_defaults`
- `session_set_identity`

也可根据现有 command 风格拆为 settings/session 现有命名空间下的增量 command。

### 13.2 Frontend API Facade

建议新增：

- `src/api/identity.ts`
- `src/api/sessions.ts` 增补 session identity methods

### 13.3 DTO Changes

前端 DTO 需补齐：

- `SessionMeta.soul_id?`
- `SessionMeta.persona_id?`
- `Session.soul_id?`
- `Session.persona_id?`

---

## 14. Observability

### 14.1 Prompt Diagnostics

`PromptPlanDiagnostics` 应能体现：

- 当前是否注入 Soul block
- 当前是否注入 Persona block
- 解析来源：global / session / fallback

### 14.2 Logging

建议在 identity resolve 时打印调试日志：

- resolved soul id
- resolved persona id
- source
- fallback / validation warning

### 14.3 Memory Audit

memory audit / debug 路径应能看到：

- memory entry identity tags
- 某条 memory 是在何种 identity 下写入的

---

## 15. Migration Strategy

### 15.1 Existing Sessions

现有 session 文件没有 `soul_id/persona_id` 字段。迁移策略：

- 不做离线迁移
- 运行时读取时字段缺省为 `None`
- 解析时 fallback 到 global default / built-in default

### 15.2 Existing Memory Entries

现有 memory entries 没有 identity tags。迁移策略：

- 不回填
- 允许缺省
- 新 entry 开始带标签

### 15.3 Existing Prompt Flow

旧 prompt 逻辑在未配置 identity 时仍然可工作：

- Soul fallback 到 built-in default
- Persona 默认缺省

---

## 16. Risks And Mitigations

### 风险 1：Soul / Persona 边界模糊

后果：

- Persona 变成另一个 system prompt
- Soul 被过度细分成多个近似 persona

缓解：

- 明确 schema 字段语义
- 明确 Persona 不得覆盖 Soul non-negotiables
- 文档和 Pack 中显式要求 review 校验边界

### 风险 2：Memory 被错误地按 Persona 主归属

后果：

- 长期学习碎片化
- 同一 Agent identity 无法积累长期经验

缓解：

- 文档明确 Soul 为 primary key
- Persona 只做 secondary tag

### 风险 3：UI 暴露过多概念，用户困惑

后果：

- 用户不知道该改 Soul 还是 Persona
- 误把 Soul 当 output style 用

缓解：

- Settings 默认突出 Persona
- Soul 放高级设置
- 用简短解释文案降低理解成本

### 风险 4：Legacy data 读写兼容性破坏

缓解：

- 所有新增字段都走 `Option`
- 不做强制 schema migration

---

## 17. Success Criteria

### 17.1 Functional

1. 用户可在 Settings 页面修改默认 Soul / Persona
2. 用户可在当前 Session 切换 Persona
3. Prompt diagnostics 可见 Soul / Persona block
4. 新 memory entry 带 `soul_id/persona_id`

### 17.2 Technical

1. Legacy session 读取不报错
2. Legacy memory entries 不报错
3. PromptPlan hash / diagnostics 稳定
4. Settings change -> new session identity 生效

### 17.3 Product

1. 用户可感知“切人格但还是同一个 Agent”
2. 后续 memory / reflection 演进不需要返工 identity 基础层

---

## 18. Implementation Plan Overview

建议拆成 5 个 Pack，按依赖顺序执行：

1. `FEAT-ID-001` Identity Domain + Registry + Resolver + Config
2. `FEAT-ID-002` Prompt Planner Soul / Persona Blocks
3. `FEAT-ID-003` Session Persistence + IPC + API Contracts
4. `FEAT-ID-004` Settings UI + Session Identity UX
5. `FEAT-ID-005` Memory Identity Tagging + Diagnostics

每个 Pack 详细内容见 `docs/packs/feature/identity-foundation/`。

---

## 19. Final Recommendation

If2Ai 不应把 Persona 直接做成简单的 output-style 替代物，也不应把 Soul 混入一个大而模糊的 agent prompt。

更合理的长期架构是：

- `Soul` 作为 durable identity
- `Persona` 作为 interaction projection
- `ResolvedIdentity` 作为 runtime turn-level contract
- `Memory tagging` 作为 identity-aware memory 的第一阶段基础设施

这样既能满足用户在 Settings 中手动切换 Soul / Persona 的需求，也能保证 memory、prompt、session 三条产品主线在未来继续演进时语义稳定。

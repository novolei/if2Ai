# If2Ai Harness Frontend Transparency UI/UX Design

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 定义 harness 在 If2Ai 前端的产品化呈现方式，使其从开发者专用遥测面板演进为面向普通用户也可理解、可追溯、可检测、非黑盒的透明体验层。

---

## 1. 为什么这份文档重要

如果 harness 只存在于：

- 后端 trace
- archive 文件
- 开发者抽屉
- CLI 查询

那它仍然是一个“系统知道、用户不知道”的黑盒治理框架。

对 If2Ai 这种桌面 agent 来说，这样不够。

因为用户真正关心的是：

- 你刚刚到底做了什么
- 你依据了什么
- 你有没有改动文件 / 使用记忆 / 触发外部动作
- 你为什么停下、为什么失败、为什么建议我确认
- 我如何判断这次结果是否值得信任

所以 harness 的一部分价值，必须在前端被看见。

---

## 2. 当前现状评估

基于当前代码库，前端已经有 4 个非常好的锚点：

1. [TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx)
   已有开发者侧遥测抽屉，但它仍然是 dev-only。
2. [ThinkingBlock.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/ThinkingBlock.tsx)
   已有“思考过程”折叠块，说明聊天流支持渐进披露。
3. [MemoryEvidencePanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryEvidencePanel.tsx)
   已经证明“证据面板”这种 UI 语言是可行的。
4. [ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx) + [InspectorPanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ds/InspectorPanel.tsx)
   当前壳层已经具备顶部状态区、消息时间线和右侧检查面板这三种展示层。

这意味着：我们不需要新造一套“监控后台 UI”。  
更合理的方向，是把 harness 变成聊天体验中的“透明层”。

---

## 3. 核心结论

### 3.1 不应只做开发者遥测面板

`TelemetryDrawer` 这种 UI 只能解决：

- 开发调试
- 统计观察
- session recording

但不能解决普通用户的信任感和可理解性。

### 3.2 最佳方案是分层透明，而不是一次性全暴露

前端不应该把 raw trace、scores、taxonomy 直接倾倒给普通用户。  
更好的做法是 3 层透明：

1. `Ambient Transparency`
   轻量状态感知，不打断主流程
2. `Inline Explainability`
   在消息和动作附近给出“发生了什么”
3. `Deep Inspection`
   给想深挖的用户 / 开发者一个可钻取面板

### 3.3 普通用户需要的是“可理解的行动证据”，不是“系统指标”

对普通用户来说，最有价值的信息不是：

- token 数
- tool call 总数
- JSONL recording status

而是：

- 本次用了哪些来源
- 有没有调用记忆
- 有没有改文件
- 有没有进入外部渠道
- 哪一步需要用户确认
- 这次结果有没有已知风险

---

## 4. 设计原则

### 4.1 Progressive Disclosure

默认只展示对信任和控制最有帮助的少量信息，深层信息按需展开。

### 4.2 Evidence Over Telemetry

优先展示“依据”和“动作结果”，其次才是系统统计。

### 4.3 User Language Over Internal Labels

不要让普通用户直接面对：

- `IntentAlignmentGrader`
- `FailureTaxonomy`
- `TraceRecord`

而应转换成：

- “本次参考了 3 条记忆”
- “本次修改了 2 个文件”
- “这条建议基于你当前项目中的 2 份资料”
- “由于权限限制，我还没有真正发送”

### 4.4 Trust Through Reversible Boundaries

如果 agent 做了高风险动作，前端必须明确表达：

- 已经执行了什么
- 还没执行什么
- 是否需要确认
- 如何回看证据

### 4.5 Same Surface, Different Depth

同一套 UI 组件应该支持：

- 普通模式
- 高级模式
- 开发者模式

而不是为不同人群做三套完全不同的界面。

---

## 5. 建议的前端信息架构

建议把 harness 透明化信息放在 4 个前端层次：

1. `Session-Level Ambient Status`
2. `Message-Level Provenance`
3. `Turn-Level Inspector`
4. `Run / Failure Review Surface`

---

## 6. Layer 1: Session-Level Ambient Status

这是最轻的一层，不打断对话。

### 6.1 顶部状态胶囊

建议在 [ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx) 顶部标题区新增一组轻量胶囊：

- `已引用资料`
- `已使用记忆`
- `已修改文件`
- `待确认`
- `恢复中`

它们不需要每轮都出现，只在相关时出现。

示例：

- `已引用资料 3`
- `记忆 2`
- `改动 1 文件`
- `草稿未发送`
- `从中断恢复`

### 6.2 会话健康状态

在 session 维度增加一枚低干扰状态灯：

- 绿色：流程稳定
- 黄色：存在不确定性 / 部分完成
- 红色：失败或需用户确认

但不要写成“harness score 0.71”。  
普通用户看到的应是：

- `已完成`
- `部分完成`
- `需要确认`
- `未完成`

### 6.3 与现有遥测的关系

[TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx) 可以继续保留，但应转为：

- 开发者模式
- 高级用户可选

而不是唯一可见入口。

---

## 7. Layer 2: Message-Level Provenance

这是最关键的一层。

每条 assistant 消息下方，都建议挂一行轻量的 provenance chips。

### 7.1 建议新增的消息 footer chips

每条消息按需显示：

- `参考了 3 份资料`
- `引用了 2 条记忆`
- `调用了 4 个工具`
- `修改了 2 个文件`
- `未真正发送`
- `需要你确认`
- `从上次中断继续`

这类信息最适合成为消息级 footer，而不是右侧大面板。

### 7.2 为什么这是最重要的一层

因为它能把黑盒感从：

- “系统做了很多我不知道的事”

变成：

- “这条回答是基于哪些东西来的”

### 7.3 与现有组件的映射

可以复用或扩展：

- [ThinkingBlock.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/ThinkingBlock.tsx)
- [ToolCallMessage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/ToolCallMessage.tsx)
- [MemoryEvidencePanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryEvidencePanel.tsx)

建议新增：

- `ResponseProvenanceRow.tsx`
- `EvidenceChip.tsx`
- `ActionBoundaryChip.tsx`

### 7.4 普通用户文案建议

不要写：

- `EvidenceFidelity: 0.82`
- `MemoryAlignment: pass`

建议写：

- `本次基于 3 份资料`
- `本次参考了你的项目偏好`
- `我还没有真正发送，只生成了草稿`
- `这一步因为权限限制还未执行`

---

## 8. Layer 3: Turn-Level Inspector

当用户点击某条消息的 provenance row 时，打开更深一层的 inspector。

### 8.1 推荐形态

最适合 If2Ai 现有结构的，不是 modal，而是右侧 inspector / slide-over。

当前已有：

- [InspectorPanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ds/InspectorPanel.tsx)

建议把它扩展成可切换 tab 的 `Turn Inspector`：

- `依据`
- `动作`
- `记忆`
- `风险`
- `详情`

### 8.2 五个 tab 的含义

`依据`
- 本次读了哪些文件 / 来源
- 哪些来源是主要证据

`动作`
- 做了哪些工具调用
- 改了哪些文件
- 有没有外部动作

`记忆`
- 用了哪些记忆
- 为什么这些记忆相关

`风险`
- 哪些动作还未执行
- 哪些步骤需要确认
- 是否存在部分完成

`详情`
- 面向高级用户的 trace / ids / advanced metadata

### 8.3 普通模式与高级模式

普通模式默认只展示：

- 依据
- 动作
- 风险

高级模式再解锁：

- 记忆细节
- 详细 trace
- ids / diagnostics

---

## 9. Layer 4: Run / Failure Review Surface

这层用于：

- partial success
- failure
- recovery
- regression review

### 9.1 失败后不应只给 ErrorCard

当前如果只是普通错误卡片，用户会感觉“失败了，但为什么失败我还是不知道”。

建议失败卡片至少补充：

- 已成功完成了什么
- 哪一步没完成
- 没完成的原因是什么
- 我建议你下一步怎么做

这实际上就是把 harness 的 `OutcomeSnapshot + failure taxonomy` 转换成用户语言。

### 9.2 Partial Success Card

对事务型任务，这个比“失败”更常见。

例如：

- 总结已经写好，但消息未发送
- 资料已经比对，但缺一份关键来源
- 计划已经生成，但恢复后 continuity 不足

建议新增：

- `PartialSuccessCard`

文案结构：

1. 已完成
2. 未完成
3. 原因
4. 建议下一步

### 9.3 Recovery Timeline

如果任务经历了：

- compaction
- resume
- retry
- permission deny

前端应能以一条非常轻的 timeline 告诉用户：

- `检索资料`
- `写出草稿`
- `等待确认`
- `恢复继续`

不必展示所有内部事件，但要让恢复行为可见。

---

## 10. 建议的前端模式分层

建议给前端 transparency 提供 3 档模式：

### 10.1 `Minimal`

适合普通用户。

展示：

- 顶部状态胶囊
- 消息 footer provenance
- 失败/部分完成卡片

### 10.2 `Standard`

适合重度用户。

在 `Minimal` 基础上再展示：

- 可展开的 turn inspector
- memory/use-of-evidence 详情
- 文件改动摘要

### 10.3 `Expert / Developer`

适合开发者与评测人员。

在 `Standard` 基础上再展示：

- `TelemetryDrawer`
- trace ids / request ids / diag key
- recording / archive / export actions

---

## 11. 与现有前端组件的建议映射

### 11.1 第一批建议新增组件

- `src/components/chat/ResponseProvenanceRow.tsx`
- `src/components/chat/TransparencyChip.tsx`
- `src/components/chat/TurnInspectorDrawer.tsx`
- `src/components/chat/PartialSuccessCard.tsx`
- `src/components/chat/ActionBoundaryNotice.tsx`

### 11.2 第一批建议改造组件

- [TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx)
  从 dev-only 继续保留，但在 Expert 模式接入，而不是唯一入口
- [MemoryEvidencePanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/memory/MemoryEvidencePanel.tsx)
  从“只看记忆”扩展为“证据面板”的一种子形态
- `chat-ui.tsx`
  在 assistant message footer 挂 provenance row
- [InspectorPanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ds/InspectorPanel.tsx)
  扩展为 turn-level inspection 容器

---

## 12. 前端需要的最小数据模型

为了让 UI 不只是“猜测后端发生了什么”，建议前端最终接收一个面向 UI 的透明化模型，而不是直接拼 trace。

建议新增一个 view model：

```ts
interface TransparencySummary {
  turn_id: string
  status: 'completed' | 'partial_success' | 'needs_confirmation' | 'failed'
  used_sources_count: number
  used_memory_count: number
  tool_calls_count: number
  files_changed_count: number
  external_actions_count: number
  draft_only: boolean
  resumed: boolean
  requires_user_confirmation: boolean
  primary_risk?: string | null
}
```

以及一个更深层的 detail model：

```ts
interface TransparencyDetail {
  summary: TransparencySummary
  evidence_items: EvidenceItem[]
  memory_items: MemoryItem[]
  file_changes: FileChangeSummary[]
  action_boundaries: ActionBoundary[]
  recovery_events: RecoveryEvent[]
}
```

关键点：

- 前端不直接消费 raw grader names
- 后端先把 harness 结果整理成 UI model
- 这样普通模式和高级模式都能复用

---

## 13. 文案与可视化建议

### 13.1 用语风格

推荐：

- `基于这些资料`
- `参考了这些记忆`
- `我已完成`
- `我还没有执行`
- `需要你确认`

避免：

- `Grader failed`
- `Taxonomy hit`
- `trace mismatch`
- `policy violation score`

### 13.2 可视化风格

不要做成“监控仪表盘风”。

更适合 If2Ai 的是：

- 轻量 chips
- 折叠说明块
- 右侧 inspector
- 小型步骤 timeline
- 柔和状态标签

也就是说：

`像解释过程的生产力界面，而不是像运维后台`

---

## 14. 对“可检测性”的产品化定义

如果把“可检测性”只理解成系统能检测，那还不够。

前端里的可检测性至少应包含：

1. 用户能检测这次用了什么依据
2. 用户能检测是否用了长期记忆
3. 用户能检测是否做了真实改动
4. 用户能检测是否仍停留在草稿/未发送状态
5. 用户能检测任务是已完成、部分完成还是等待确认

换句话说：

`detectability = user can inspect the trust boundary of the turn`

---

## 15. 第一阶段前端落地建议

如果只做第一阶段 MVP，我建议前端只先落 4 个东西：

1. assistant message footer 的 `ResponseProvenanceRow`
2. `PartialSuccessCard`
3. 右侧 `TurnInspectorDrawer`
4. Expert 模式下接入现有 `TelemetryDrawer`

这 4 个已经足够把 harness 从“黑盒治理框架”推进到“用户可感知的透明层”。

---

## 16. 后续实施任务

### UI-1

在聊天消息模型里增加 `transparency_summary`

### UI-2

实现 `ResponseProvenanceRow`

### UI-3

实现 `TurnInspectorDrawer`

### UI-4

实现 `PartialSuccessCard`

### UI-5

把 `MemoryEvidencePanel` 升级为可复用 evidence viewer

### UI-6

增加 transparency mode 设置：

- Minimal
- Standard
- Expert

### UI-7

将 `TelemetryDrawer` 归位到 Expert 模式

---

## 17. 与其他文档的关系

- harness 总体策略： [harness-strategy-v2.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-strategy-v2.md)
- general tasks： [harness-general-task-optimization.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-general-task-optimization.md)
- grader 与可解释性： [grader-scoring-rubric-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/grader-scoring-rubric-spec.md)
- archive 与可回看能力： [archive-query-cli-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/archive-query-cli-spec.md)

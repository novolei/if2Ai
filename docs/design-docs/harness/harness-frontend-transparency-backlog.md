# If2Ai Harness Frontend Transparency Backlog

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 将 harness 前端透明化 UI/UX 方案拆成可执行 backlog，明确每个任务的用户问题、前端落点、后端依赖、数据契约、验收标准和 rollout 风险。

---

## 1. 为什么需要这份 backlog

[harness-frontend-transparency-uiux.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-frontend-transparency-uiux.md) 已经定义了透明化的体验原则和 UI 分层，但正式实现前还缺两件事：

1. 把设计目标翻译成可以排期的 backlog
2. 把 UI 工作和 `6H` 的后端/contract 依赖正式对齐

这份文档就是为这两件事准备的。

---

## 2. 总体原则

### 2.1 透明化不是开发者面板外溢

前端透明化的目标不是把 raw trace 或 grader 分数直接暴露给普通用户，而是让用户看见：

- 依据
- 动作
- 边界
- 风险
- 完成度

### 2.2 先建 UI View Model，再建组件

推荐执行顺序固定为：

1. `TransparencySummary`
2. `TransparencyDetail`
3. 轻量 message/session surfaces
4. 深层 inspector
5. Expert telemetry mode

如果不这样做，前端会直接绑定 raw backend schema，后续非常难演进。

### 2.3 普通模式优先，Expert 模式下沉

默认产品体验必须服务普通用户：

- 轻量、清楚、非技术化

Expert 模式才展示：

- trace ids
- session recording
- telemetry stats
- deeper diagnostics

---

## 3. Backlog 总览

建议分为 6 个 backlog 项：

1. `FT-1 Transparency View Models`
2. `FT-2 Session Ambient Transparency`
3. `FT-3 Message Provenance Row`
4. `FT-4 Partial Success / Needs Confirmation Surface`
5. `FT-5 Turn Inspector Drawer`
6. `FT-6 Expert Telemetry Mode`

这 6 项中，`FT-1` 是所有其他项的前置依赖。

---

## 4. FT-1 Transparency View Models

### 用户问题

当前前端能看到一些 telemetry 和 memory evidence，但没有一层稳定的“用户可理解透明化模型”，导致：

- UI 只能依赖临时字段
- 透明化信息无法跨组件复用
- 普通模式和 expert 模式无法统一

### 前端落点

- `src/modules/chat/types.ts`
- `src/lib/tauri.ts`
- `src/stores/conversation-slice.ts`
- `src/modules/chat/components/ChatWorkspace.tsx`

### 后端依赖

依赖 `6h.4`、`6h.5`、`6h.8` 产出可供整理的：

- outcome status
- user_experience / failure taxonomy
- evidence / memory / action boundary refs

### 建议数据契约

最少需要：

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

interface TransparencyDetail {
  summary: TransparencySummary
  evidence_items: EvidenceItem[]
  memory_items: MemoryItem[]
  file_changes: FileChangeSummary[]
  action_boundaries: ActionBoundary[]
  recovery_events: RecoveryEvent[]
}
```

### 验收标准

- 前端 message model 能挂接 `transparency_summary`
- UI 不直接依赖 raw grader ids
- 至少支持 `completed / partial_success / needs_confirmation / failed`

### 评审重点

- 文案是否是用户语言
- 类型是否足够稳定可扩展
- 是否能同时服务 message-level 和 inspector-level UI

---

## 5. FT-2 Session Ambient Transparency

### 用户问题

用户通常不知道当前会话是否：

- 正在恢复
- 已经改动文件
- 使用了外部依据
- 仍需要确认

### 前端落点

- [ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx)
- 标题区 / 顶部状态区

### UI 交付

新增轻量胶囊或状态标签：

- `已引用资料 N`
- `记忆 N`
- `改动 N 文件`
- `草稿未发送`
- `从中断恢复`
- `需要确认`

### 后端依赖

- `TransparencySummary`

### 验收标准

- 顶部状态不喧宾夺主
- 仅在相关时显示，不做常驻噪音
- 普通用户不需要理解内部技术名词

### 风险

- 信息太多会污染标题区

### 约束建议

- 同时最多显示 3 个胶囊
- 风险型状态优先于统计型状态

---

## 6. FT-3 Message Provenance Row

### 用户问题

用户最需要知道的是：

- 这条回答基于什么
- 做了什么动作
- 哪些事情还没真正执行

### 前端落点

- `chat-ui.tsx`
- 新增：
  - `src/components/chat/ResponseProvenanceRow.tsx`
  - `src/components/chat/TransparencyChip.tsx`

### UI 交付

在 assistant message footer 挂按需 chips：

- `参考了 3 份资料`
- `引用了 2 条记忆`
- `调用了 4 个工具`
- `修改了 2 个文件`
- `未真正发送`
- `需要你确认`

### 后端依赖

- `TransparencySummary`

### 验收标准

- message row 不应强迫用户打开开发者面板
- chips 点击后能进入更深层 inspector
- 普通模式下信息量控制在单行或双行以内

### 风险

- 若直接展示过多 chips，会把聊天界面变成日志流

### 约束建议

- 默认优先显示：
  1. 风险边界
  2. 依据
  3. 动作摘要

---

## 7. FT-4 Partial Success / Needs Confirmation Surface

### 用户问题

事务型任务经常不是简单成功/失败，而是：

- 已写好草稿，但未发送
- 已完成总结，但证据不足
- 已恢复一部分，但 continuity 不完整

当前如果只显示普通 error card，会让用户不清楚到底做成了多少。

### 前端落点

- 新增：
  - `src/components/chat/PartialSuccessCard.tsx`
  - `src/components/chat/ActionBoundaryNotice.tsx`

### UI 交付

统一展示结构：

1. 已完成什么
2. 未完成什么
3. 为什么
4. 下一步建议

### 后端依赖

- `TransparencySummary.status`
- `TransparencyDetail.action_boundaries`
- `failure taxonomy -> user-facing reason`

### 验收标准

- partial success 与 failed 有明显区分
- `needs_confirmation` 任务明确表达“未执行”的边界
- 不把内部 taxonomy 直接暴露给用户

### 风险

- 如果文案写成技术错误，会破坏信任

---

## 8. FT-5 Turn Inspector Drawer

### 用户问题

有些用户会想深挖单轮回答的依据、动作和风险，但不应该被迫看 raw trace。

### 前端落点

- 扩展 [InspectorPanel.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ds/InspectorPanel.tsx)
- 或新增：
  - `src/components/chat/TurnInspectorDrawer.tsx`

### UI 交付

推荐 5 个 tab：

- `依据`
- `动作`
- `记忆`
- `风险`
- `详情`

### 后端依赖

- `TransparencyDetail`

### 验收标准

- 普通模式下只需展示前三层重要信息
- 高级模式才显示更细 metadata
- inspector 和 message provenance row 有自然跳转关系

### 风险

- 做成开发工具感太强

### 约束建议

- 默认使用自然语言 summary
- 详细 ids / trace refs 放在最深一层

---

## 9. FT-6 Expert Telemetry Mode

### 用户问题

开发者和高级用户仍然需要：

- telemetry
- recording
- diagnostics
- debug correlation

但这些不应污染普通体验。

### 前端落点

- [TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx)
- Settings / developer toggle

### UI 交付

把现有 `TelemetryDrawer` 正式归位到：

- `Expert` 模式
- 或 Developer toggle

并与 message/session transparency 共存。

### 后端依赖

- 现有 harness IPC
- 后续 archive/trace query IPC 可进一步增强

### 验收标准

- 默认用户看不到开发者遥测
- Expert 模式可随时打开
- 不与普通透明层重复表达相同信息

### 风险

- 两套 UI 各讲各的，语义不一致

### 约束建议

- Expert 模式复用同一份 `TransparencySummary` / `TransparencyDetail`
- telemetry 作为 deeper diagnostics，而不是平行体系

---

## 10. 推荐依赖关系

建议依赖固定为：

```text
FT-1 Transparency View Models
  ├─ FT-2 Session Ambient Transparency
  ├─ FT-3 Message Provenance Row
  ├─ FT-4 Partial Success / Needs Confirmation
  ├─ FT-5 Turn Inspector Drawer
  └─ FT-6 Expert Telemetry Mode
```

实现优先级建议：

1. `FT-1`
2. `FT-3`
3. `FT-4`
4. `FT-5`
5. `FT-2`
6. `FT-6`

原因：

- 先解决每条消息“为什么可信/为什么没完成”
- 再补 session ambient 和 expert mode

---

## 11. 推荐插入 6H 的方式

建议在 `phase-6h-harness-v2-eval-control-plane.yaml` 中新增 3 个 UI slices，而不是把 6 个 FT backlog 全部直接平铺进去。

### `6h.10`

`Transparency View Models`

承接：

- `FT-1`

依赖：

- `6h.4`
- `6h.5`
- `6h.8`

### `6h.11`

`Message Provenance + Partial Success Surfaces`

承接：

- `FT-3`
- `FT-4`
- `FT-2` 的一部分轻量状态

依赖：

- `6h.10`

### `6h.12`

`Turn Inspector + Expert Telemetry Mode`

承接：

- `FT-5`
- `FT-6`

依赖：

- `6h.10`
- `6h.7`

---

## 12. Reviewer Checklist

每个透明化 UI PR reviewer 至少检查：

1. 是否使用用户语言，而不是 raw harness 名词
2. 是否优先展示依据、动作和边界，而不是统计
3. 是否控制了视觉噪音
4. 是否能清晰区分 `completed / partial_success / needs_confirmation / failed`
5. 是否避免直接绑定 raw backend/grader schema
6. 是否与现有 `MemoryEvidencePanel` / `TelemetryDrawer` 语义一致
7. 是否兼顾普通模式和 expert 模式

---

## 13. 常见反模式

### Anti-Pattern 1

直接把 grader 分数渲染给用户

### Anti-Pattern 2

把透明化做成另一个“监控仪表盘”

### Anti-Pattern 3

普通模式和 expert 模式使用两套不同数据模型

### Anti-Pattern 4

所有消息都强制显示满配 provenance chips

### Anti-Pattern 5

只显示“失败了”，不解释已完成部分

---

## 14. 与其他文档的关系

- 透明化设计： [harness-frontend-transparency-uiux.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-frontend-transparency-uiux.md)
- rubric 与风险含义： [grader-scoring-rubric-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/grader-scoring-rubric-spec.md)
- 6H 主执行计划： [phase-6h-harness-v2-eval-control-plane.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-6h-harness-v2-eval-control-plane.yaml)

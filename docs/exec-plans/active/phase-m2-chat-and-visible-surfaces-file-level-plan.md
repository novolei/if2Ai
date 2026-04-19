# Phase M2 Chat And Visible Surfaces File-Level Plan

> 将 `M2` 的 `m2.8 + m2.9` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m2.8` Refactor chat main path to consume projection stores
2. `m2.9` Project execution-mode and activation state into user-visible surfaces

目标是让聊天主路径真正开始消费 projection state，并把 execution mode / activation 从“隐藏平台事实”变成可见、可核查的产品表面。

## 2. 当前事实基线

### 2.1 聊天主路径现状

当前主聊天路径主要由：

1. [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
2. [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
3. [src/modules/chat/components/HomeScreen.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/HomeScreen.tsx:1)

直接消费大量 props 和局部状态。

这意味着：

1. chat 主路径对 runtime truth 还不是“投影消费”，而是“页面自己拼语义”
2. `chat-ui.tsx` 的责任仍然过重

### 2.2 用户可见平台真相现状

当前前端没有正式表面来稳定展示：

1. execution mode
2. reason codes / matched rules
3. activation / license status
4. revoke / expire / deactivate 回流状态

这会导致平台治理层虽然在文档里是一级能力，但产品上还是不可见。

### 2.3 settings 现状

当前 settings 已有独立窗口和成熟壳层：

1. [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
2. [src/modules/settings/pages/GeneralSettingsPage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/pages/GeneralSettingsPage.tsx:1)
3. [src/modules/settings/pages/RemoteSettingsPage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/pages/RemoteSettingsPage.tsx:1)

因此 `m2.9` 不需要重造 settings 框架，而是要把 activation/execution-mode 信息挂到现有表面。

## 3. 实施原则

1. Chat 主路径优先，调试/观测路径二级展开。
2. execution mode 和 activation 必须显示为平台真相，不再只是开发者脑内知识。
3. `chat-ui.tsx` 先降责，不要求这一轮拆到极致。
4. 不得让新的可见表面从临时 `invoke` 直接读数据，必须读 projection store。

## 4. 严格执行顺序

1. `V0` preflight inventory
2. `V1` 识别 `chat-ui.tsx` 可迁出的 projection consumption 面
3. `V2` 回写 `ChatWorkspace`
4. `V3` 回写 `chat-ui.tsx`
5. `V4` 建 execution-mode visible surfaces
6. `V5` 建 activation/license visible surfaces
7. `V6` 收 debug / telemetry / observability 分层
8. `V7` compile + manual verification

禁止并行：

1. `chat-ui.tsx` 主体重构
2. Settings 页入口扩展
3. activation gate 视觉细化

因为这三件事一起做时，最容易把主路径和次级路径重新耦合回去。

## 5. 文件级实施方案

## 5.1 `V0` Preflight Inventory

### 必查文件

- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)
- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
- [src/modules/settings/pages/GeneralSettingsPage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/pages/GeneralSettingsPage.tsx:1)
- [src/modules/settings/pages/RemoteSettingsPage.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/pages/RemoteSettingsPage.tsx:1)

### 必做动作

1. 标出 `chat-ui.tsx` 中所有直接处理消息运行态的局部逻辑。
2. 标出 execution mode 最自然的显示位置。
3. 标出 activation/license 状态最自然的 settings 落点。

## 5.2 `V1` 识别 `chat-ui.tsx` 可迁出的 projection consumption 面

### 修改文件

- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)

### 第一批优先迁出的输入

1. messages timeline
2. loading / streaming status
3. memory evidence panel inputs
4. permission mode display state

### 刻意不在这一步做的事

1. 不重写 markdown 渲染
2. 不重写 project rail preview 逻辑
3. 不重写 TTS/STT 局部能力

目标是先让主聊天面“读 projection”，不是先做大清理。

## 5.3 `V2` 回写 `ChatWorkspace`

### 修改文件

- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)

### 必须落实

1. `ChatWorkspace` 先从新 stores 读取主路径 projection。
2. 仍可保留一部分 props 作为过渡，但新的 session/run/memory/approval 相关数据优先走 store。
3. `ChatWorkspace` 负责将主路径数据整理后喂给 `ChatUI`，而不是在 `ChatUI` 内再猜一次运行态。

### 允许保留的 legacy props

1. layout-related props
2. side rail open/collapse props
3. font/density 偏好 props

## 5.4 `V3` 回写 `chat-ui.tsx`

### 修改文件

- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)

### 必须落实

1. `ChatUI` 变成 projection consumer，不再直接承载 runtime truth 解释。
2. 内部 Message 类型如仍需保留，只能是 UI view-model，而不是 transport contract 副本。
3. 与 MemoryChip / ContextBar / TodoPanel 的接线优先改成读 projection 结果。

### 第一阶段完成后，`chat-ui.tsx` 不应继续承担

1. runtime event shape compatibility
2. execution mode 推断
3. activation 状态判断

## 5.5 `V4` 建 execution-mode visible surfaces

### 新增文件

- `src/modules/chat/components/ExecutionModeBadge.tsx`
- `src/modules/chat/components/ExecutionModeInspectorCard.tsx`
- `src/modules/settings/pages/DeveloperRuntimePage.tsx`

### 修改文件

- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/data.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/data.ts:1)

### 第一版最少可见内容

1. 当前 `execution mode`
2. `risk/complexity`
3. `reason codes`
4. `matched rules`
5. `manual override` 状态

### 落点建议

1. 聊天页顶部或 inspector 侧显示 compact badge
2. Inspector 或 Developer Settings 显示完整 explainability card

### 约束

1. 不能只藏在开发模式 console。
2. 不能由前端临时调用命令现查。
3. 必须读 `execution-mode-store`

## 5.6 `V5` 建 activation/license visible surfaces

### 新增文件

- `src/modules/settings/pages/AccountActivationPage.tsx`
- `src/modules/settings/components/ActivationStatusCard.tsx`

### 修改文件

- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/data.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/data.ts:1)

### 第一版最少可见内容

1. activation status
2. license id / plan summary
3. last refresh / revoke check summary
4. deactivate entry
5. expired / revoked / deactivated explanation

### 约束

1. 不允许继续让 activation 只留在 onboarding step 6。
2. 不允许用假数据占位 license 状态。
3. 设置页中的状态必须能回流 gate。

## 5.7 `V6` 收 debug / telemetry / observability 分层

### 修改文件

- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)
- [src/modules/chat/components/ChatWorkspace.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/chat/components/ChatWorkspace.tsx:1)
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)

### 必须落实

1. telemetry / debug / observability 不再抢占主聊天路径的主要注意力。
2. 开发者面板通过二级入口展开。
3. 聊天主路径保持单一主任务动线。

### 这一步不要做的事

1. 不要删除 telemetry drawer。
2. 不要把观测能力重新塞回主内容区。

## 5.8 `V7` Compile + Manual Verification

### 必跑

- `npm run build:web`

### 必做人工检查

1. 进入聊天页后，主路径不会因 projection 改写而掉消息。
2. execution mode 在聊天或 inspector 中可见、可解释。
3. settings 中可以看到 activation/license 状态。
4. telemetry/debug 面板仍可打开，但不再主导主路径。

## 6. 完成定义

只有当 reviewer 可以清楚指出“聊天主路径现在消费的是 projection state，而且 execution mode 与 activation 已经成为可见平台真相”时，这一段才算完成。

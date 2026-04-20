# If2Ai Phase M0.6 — God-File Responsibility Inventory

> Phase M0 后半段产出。把当前 codebase 中四个 god-file 的责任拆成可被 M1 / M2 直接消费的清单。
>
> 最后更新: 2026-04-20
> 配套真相：[if2ai-canonical-domain-model.md](./if2ai-canonical-domain-model.md)、[if2ai-workflow-truth.md](./if2ai-workflow-truth.md)
> 配套 contracts：[src-tauri/src/modules/runtime/contracts/](../../src-tauri/src/modules/runtime/contracts/)、[src/transport/contracts.ts](../../src/transport/contracts.ts)
> 上位设计：[backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)、[runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)、[frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md)

## 1. 文档目的

M0 后半段不再写新概念，而是把"责任"从四个 god-file 中识别出来，让 M1 后端 service extraction 与 M2 前端 runtime projection 拿到可直接落刀的清单。

任何 inventory 条目都必须满足：

1. 对照真实代码（含行号 / 函数名 / 类型名），不写"文件太大"。
2. 每条责任都标明 destination module 与 target phase（M1 / M2 / M3+）。
3. 给出 safe extraction order：先抽什么、后抽什么、为什么不能颠倒。
4. 标记 risky entanglements：哪些代码段如果硬拆会立即破坏主路径。

本 inventory 只承诺"能用"。任何拆分动作的最终 PR 由对应 M1 / M2 slice 落实，inventory 只决定"能拆什么、按什么顺序拆、哪儿拆不动"。

## 2. 文件规模快照

| file                                                                     | line count (M0.6 时刻) | 主要 export                                                                                                   |
| ------------------------------------------------------------------------ | ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| [src-tauri/src/commands/agent.rs](../../src-tauri/src/commands/agent.rs) | 4060                   | `run_agent_turn` / `start_agent_stream` / `stop_agent_stream` / `respond_permission` / `RunAgentTurnResponse` |
| [src/components/ui/chat-ui.tsx](../../src/components/ui/chat-ui.tsx)     | 4736                   | `ChatUI`（单巨型导出），含 ~15 个内部子组件 + ~50 个内部 helper                                               |
| [src/App.tsx](../../src/App.tsx)                                         | 2472                   | `App`（单组件，41 个 hook 调用）                                                                              |
| [src/lib/tauri.ts](../../src/lib/tauri.ts)                               | 2319                   | 130+ 个 `export`：IPC bridge functions + payload interfaces + 类型 + 事件 listener helpers                    |

## 3. 共用 destination map

引用频次高的 destination module，先在这里登记，后面四节直接引用：

| destination                                            | 描述                                                                        | 在哪个 phase 落地    |
| ------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------- |
| `src-tauri/src/modules/services/conversation/`         | conversation orchestrator（取代 `start_agent_stream` 主循环）               | M1                   |
| `src-tauri/src/modules/services/provider/`             | provider 选择 / transport policy / API client 封装                          | M1                   |
| `src-tauri/src/modules/services/prompt/`               | system prompt 拼装 / sanitize / preflight / governor                        | M1                   |
| `src-tauri/src/modules/services/memory_injection/`     | memory recall + 注入 + trajectory 记录                                      | M1                   |
| `src-tauri/src/modules/services/stream/`               | runtime event emitter（产生 `RuntimeEventEnvelope`）                        | M1                   |
| `src-tauri/src/modules/services/permission/`           | permission prompt / decision pipeline                                       | M1                   |
| `src-tauri/src/modules/services/request_intelligence/` | execution-mode classifier                                                   | M1                   |
| `src-tauri/src/modules/services/activation/`           | activation lifecycle service                                                | M1                   |
| `src-tauri/src/modules/control_plane/`                 | control-plane seam（已存在 module，扩）                                     | M1 后期              |
| `src/transport/`                                       | thin IPC bridge（取代 `tauri.ts` 的 IPC + listener helpers）                | M2                   |
| `src/transport/contracts.ts`                           | canonical TS types（M0.3 已建）                                             | M0.3 起，M2 全量消费 |
| `src/runtime/translator/`                              | 把 `RuntimeEventEnvelope` 翻成 typed action                                 | M2                   |
| `src/runtime/reducers/`                                | conversation / memory / activation / execution-mode reducer                 | M2                   |
| `src/runtime/stores/`                                  | zustand / context store                                                     | M2                   |
| `src/modules/boot-shell/`                              | startup splash + activation gate overlay + main shell 切换                  | M2                   |
| `src/modules/chat/`                                    | 已有部分组件（`SttButton` 等）；chat 主路径子组件应按 feature 落入此 module | M2                   |
| `src/modules/markdown/`                                | markdown 渲染 / code highlighting / 文本规范化 helpers                      | M2                   |
| `src/modules/skills-report/`                           | slash skills report 解析 + 渲染                                             | M2                   |

## 4. `src-tauri/src/commands/agent.rs`（4060 行）

### 4.1 current responsibilities

按代码段实际承担的职责清单：

| 段落                                        | 行号区间（约）  | 职责                                                                                                                                                                              |
| ------------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Provider transport / control-plane switches | 218–386         | `load_provider_transport_policy` / `create_runtime_provider_client_from_config` / `ControlPlaneRuntimeSwitches` / `load_control_plane_switches`                                   |
| `RealApiClient`                             | 387–540         | 封装 provider 出站 API；handle retry / streaming chunk classification                                                                                                             |
| `ToolRegistryExecutor`                      | 541–706         | tool 调度执行体，handle tool result + permission prompt                                                                                                                           |
| Tool result heuristics                      | 707–805         | `contains_unverified_file_claim` / `is_mutating_tool_success` / `shell_command_likely_mutates_files` / `extract_skill_proposal_name`                                              |
| Memory recall + injection                   | 58–147、806–903 | `append_memory_injection_sections` / `RetrievedMemoryContext` / `map_scored_memory_to_payload` / `retrieve_memory_context`                                                        |
| Trajectory 记录                             | 904–943         | `record_trajectory_if_possible`（写 ShareGPT JSONL）                                                                                                                              |
| `run_agent_turn`（非流式）                  | 944–1489        | ~545 行的非流式 turn 编排：context 装配 + provider 调用 + tool loop + 持久化                                                                                                      |
| `start_agent_stream`（流式）                | 1490–3093       | ~1600 行的流式 turn 编排：tool loop / token stream / permission wait / governor / preflight / sanitization                                                                        |
| Stream telemetry helpers                    | 3094–3230       | `format_stream_error_reason` / `truncate_tool_result_for_model` / `summarize_tool_result_for_model` / `short_text_digest` / resume cursor 编解码 / `runtime_block_to_input_block` |
| Request preflight + governor                | 3258–3729       | `RequestPreflightStats` / `ContextGovernor` / `apply_request_preflight_limits` / token & char count estimators / message summarization / network error 判断                       |
| Sanitization                                | 3732–3905       | `SanitizationStats` / `sanitize_messages_for_provider` / `remove_tool_use_blocks` / `extend_sample_ids` / `parse_tool_input_json`                                                 |
| `stop_agent_stream`                         | 3906–3932       | 通过 `AppState.stream_cancel_senders` 取消进行中的 stream                                                                                                                         |
| Permission prompter                         | 3933–4001       | `TauriPermissionPrompter` —— bridge `agent loop` 与 frontend permission UI                                                                                                        |
| `respond_permission`                        | 4002–4060       | 处理前端发来的 permission 决策                                                                                                                                                    |

### 4.2 responsibilities that belong elsewhere

| 当前所在                                                                                                          | 应去                                                                                                                                 | 理由                                                            |
| ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `RealApiClient` + provider transport / control-plane switch loaders                                               | `services/provider/`                                                                                                                 | provider/transport/policy 是独立可复用对象，与 turn 编排无关    |
| `ToolRegistryExecutor`                                                                                            | 复用 `modules/tools/` 现有抽象，commands 层只做胶水                                                                                  | 工具执行已有 registry，不应在 commands 层再造一层 executor      |
| 文件改写 / shell 突变启发式（`is_mutating_tool_success` 等）                                                      | `modules/tools/` 内部或 `services/governance/`                                                                                       | 与 turn 编排无关，应靠近 tool 自身的 verifier                   |
| `append_memory_injection_sections` / `retrieve_memory_context` / `map_scored_memory_to_payload`                   | `services/memory_injection/` → 之后再被 M3 `MemoryCoordinator` 收拢                                                                  | 当前直接 reach 进 `AppState.memory_provider`，破坏 service 边界 |
| `record_trajectory_if_possible`                                                                                   | `services/trajectory/`（新模块）或 M5 trajectory pipeline                                                                            | trajectory 不属于 turn 编排，是观测产物                         |
| `run_agent_turn` / `start_agent_stream` 主体                                                                      | `services/conversation/` 编排器 + `services/stream/` emitter                                                                         | 这两个 command 是 thin entry，编排逻辑不应嵌在 commands 层      |
| `RequestPreflightStats` / `ContextGovernor` / `apply_request_preflight_limits` / `sanitize_messages_for_provider` | `services/prompt/`（preflight + sanitization）                                                                                       | 输入 message 整形与 turn 编排解耦                               |
| `TauriPermissionPrompter` + `respond_permission`                                                                  | `services/permission/`                                                                                                               | 这是 permission lifecycle 服务，不是 turn 编排子例程            |
| stream chunk → frontend event emit                                                                                | `services/stream/` emitting `RuntimeEventEnvelope`（[contracts/common.rs](../../src-tauri/src/modules/runtime/contracts/common.rs)） | 取代当前的散点 `app.emit(...)`                                  |

### 4.3 target phase

- **全部 M1**：本文件的所有真实抽离都由 Phase M1 承接，没有任何责任归属 M2 / M3 / M4 / M5。
- M3 `MemoryCoordinator` 在 M1 抽离 `services/memory_injection/` 之后再来收拢，但 M1 阶段不依赖 M3。
- 任何 trajectory 高级路径（scoring / candidate / promote）归 M5，不污染本轮拆分。

### 4.4 destination module

参见 §3 总表。本文件涉及的目标 module 全部在 `src-tauri/src/modules/services/` 下新建（M1 已规划该目录骨架）。

### 4.5 safe extraction order

固定顺序，不允许颠倒：

1. **先**：抽离 `services/provider/`（含 `RealApiClient` + transport policy）。原因：provider 是无状态对象，抽离对主路径风险最低；后续 service 都依赖它的稳定签名。
2. 抽离 `services/prompt/`（preflight + sanitization + governor）。原因：纯函数集合，单元测试覆盖容易，先剥可让后续 stream service 拿到稳定签名。
3. 抽离 `services/memory_injection/`（recall + 注入 + 当前 audit 事件源）。原因：reach 进 `AppState.memory_provider` 的代码集中在这里，提前剥离避免 conversation service 继续依赖 `AppState`。
4. 抽离 `services/permission/`（含 `TauriPermissionPrompter` + `respond_permission` + `permission_senders` map 所有权迁移）。原因：permission 是独立 lifecycle，不解耦的话 stream service 会继续直接跨写 `AppState`。
5. 抽离 `services/stream/`（envelope emitter）。原因：必须在 conversation service 之前完成，否则后者无 envelope 可发。
6. 抽离 `services/conversation/`（取代 `run_agent_turn` / `start_agent_stream` 主体）。原因：上面五个 service 都稳定后再来收 turn 编排，commands 层退化为 thin entry。
7. 抽离 `services/trajectory/`（最后）。原因：仅观测路径，不阻塞主路径。
8. 引入 `services/request_intelligence/` 与 `services/activation/`：这两块在 commands/agent.rs 中**当前没有真实代码**，由对应 commands 文件持有；可与上面 1–7 并行进行，互不阻塞。

### 4.6 risky entanglements

| 风险点                                                                                                                  | 描述                                                                                                                         | 应对                                                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `start_agent_stream` 函数体 ~1600 行                                                                                    | 同一函数同时 own provider client、tool loop、token stream、permission wait、preflight、sanitization、governor、resume cursor | 必须分两步：先把上面 §4.5 的 1–5 抽干净，最后才能切函数体；不要在依赖未剥离时切函数                                            |
| `AppState` 的多个 `Arc<Mutex<HashMap<...>>>`（`permission_senders` / `permission_overrides` / `stream_cancel_senders`） | 多个 service 共享 mutable 状态，所有权迁移容易撞                                                                             | 迁移到对应 service 时改成 service 内部 `Arc<RwLock>`，commands 层只通过 service handle 访问，禁止跨 service 直接 lock          |
| trajectory writer 失败默认静默                                                                                          | 当前 `record_trajectory_if_possible` 失败只记日志                                                                            | 抽离时不要顺手改语义，否则 trajectory pipeline 行为变化会被 M5 误判                                                            |
| ContextGovernor / preflight 与 provider 真实 token limit 强耦合                                                         | 估算逻辑依赖 message shape                                                                                                   | 抽 prompt service 时保留估算口径不变，让 conversation service 通过 stable trait 调用                                           |
| permission prompt 通过事件名约定与前端通信                                                                              | 当前事件名是字符串字面量散落                                                                                                 | 在 M0.3 `RuntimeEventEnvelope` 落地前，permission service 不要先改事件名；M1 抽离时统一切到 envelope `event_type = Permission` |
| resume cursor 字符串编解码                                                                                              | `build_resume_cursor` / `parse_resume_cursor` / `strip_resume_cursor_marker` 是隐式状态机                                    | 抽 stream service 时必须保留 cursor 测试；不要在抽离 PR 中顺手改 cursor 格式                                                   |

## 5. `src/components/ui/chat-ui.tsx`（4736 行）

### 5.1 current responsibilities

| 段落                                                             | 行号区间（约） | 职责                                                                                                                                                                        |
| ---------------------------------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 顶层 lazy import + 常量                                          | 1–170          | `MessageVoiceButtonLazy` / `SttButtonLazy` / 字体 + 密度 + project rail 各 storage key + tokens                                                                             |
| `FontSans/SerifIcon` / `DensityCompact/ComfortableIcon`          | 172–221        | 4 个内联 SVG icon 组件                                                                                                                                                      |
| `ChatUI` 主组件                                                  | 222–1300       | ~1080 行。100 个 hook 调用：state / refs / effects / callbacks，混合 chat 主流、project rail、density/font 偏好、stream 订阅、recovery 流程                                 |
| `ProjectFilesRail` + 子组件                                      | 1301–1788      | 文件树投影、文件 preview、autosave 节流、排序与树节点渲染                                                                                                                   |
| `ChatTranscript`                                                 | 1789–1914      | 消息列表渲染（含虚拟列表门槛 `VIRTUAL_LIST_THRESHOLD = 50`）                                                                                                                |
| `ComposerDock`                                                   | 1915–2432      | 输入框、stt button、模型选择器、发送按钮、permission prompt UI 触发                                                                                                         |
| `MemoryStoreToolCard` / `ToolCallMessage`                        | 2435–2666      | memory & tool call 卡片渲染                                                                                                                                                 |
| `SlashCommandSuggestions` / `AtFileSuggestions`                  | 2667–2909      | slash command + @file 自动补全                                                                                                                                              |
| `ChatMessage` + `MarkdownContent`                                | 2910–3244      | 单条消息容器 + markdown 渲染 + 缓存                                                                                                                                         |
| Skills slash report 解析 + 渲染                                  | 3245–3464      | `SkillsSlashReport` / `SkillMetaRow` / 一组 grouping / variant priority / status normalize / source key & label normalize / glyph 选择 / `parseSkillsSlashReport`           |
| Code & 文本规范化                                                | 3465–3720      | `CodeBlock` 渲染 + ASCII relationship box → markdown / decorative line strip / hash / narrative & keyword line normalization                                                |
| `MessageCopyButton` + 工具/状态 helpers                          | 3724–3954      | copy text / format short time / tool status / glyph / display / diagnostic copy text                                                                                        |
| Tool 摘要 helpers                                                | 3955–4286      | `pickToolHeadline` / `buildToolDetailLines` / `buildToolCommandResultLine` / `normalizeToolLabel` / `renderInlineToolSummary` / `summarizeToolResult` / json + 文本截断工具 |
| `EmptyState` / `LoadingIndicator` / `RecoveryCard` / `ErrorCard` | 4287+          | 顶层副屏                                                                                                                                                                    |

### 5.2 responsibilities that belong elsewhere

| 当前所在                                                               | 应去                                                                                      | 理由                                                    |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `ChatUI` 中所有 stream / memory / permission / recovery 订阅与状态合成 | M2 `runtime/translator` + `runtime/reducers/conversation` + `runtime/stores/conversation` | 视图组件不应自己合成业务事件；envelope 已在 M0.3 准备好 |
| 模型 / 字体 / 密度偏好等用户偏好 hook                                  | `src/modules/preferences/`（新 module，M2 引入）                                          | 偏好状态与 chat 主路径正交                              |
| `ProjectFilesRail` + 子组件                                            | `src/modules/project-rail/`（新 module，M2）                                              | 文件树投影是独立 surface                                |
| `ComposerDock`                                                         | `src/modules/chat/composer/`（M2）                                                        | 是 chat 子模块，但应能独立测试                          |
| `SlashCommandSuggestions` / `AtFileSuggestions`                        | `src/modules/chat/composer/suggestions/`（M2）                                            | 与 composer 强相关但应可拆                              |
| `MarkdownContent` + ASCII / narrative / keyword 规范化 helpers         | `src/modules/markdown/`（M2）                                                             | 渲染逻辑跨场景复用                                      |
| `SkillsSlashReport` 解析 + 渲染 + helpers                              | `src/modules/skills-report/`（M2）                                                        | 与 skills hub feature 同源                              |
| Tool call 摘要 + glyph + 状态判断 helpers                              | `src/modules/tool-projection/`（M2）                                                      | tool call UI 是独立投影                                 |
| `RecoveryCard` / `ErrorCard`                                           | `src/modules/boot-shell/recovery/` 或 `src/modules/system-feedback/`（M2）                | 与 chat 主流不强耦合                                    |

### 5.3 target phase

- **全部 M2**。本文件不会向 M1 / M3+ 抛出任何责任。
- 但 M2 的拆分必须以 M0.3 的 `RuntimeEventEnvelope` 与 M0.5 的 `ExecutionModeDecision` 已落 contracts 为前提，不得在 contracts 缺位时硬拆。

### 5.4 destination module

参见 §3 总表。所有目标 module 都在 `src/modules/` 下新建或扩展，`src/components/ui/` 退化为通用 UI primitive 容器。

### 5.5 safe extraction order

固定顺序：

1. **先抽 helpers（零 hook，零 React state）**：
   - `src/modules/markdown/`（含 `MarkdownContent` 与所有 ASCII / narrative / keyword 规范化、`CodeBlock`、`MessageCopyButton`、`extractCodeText`、`formatShortTime`、`copyTextToClipboard`、`normalizeCodeForDisplay` 等）
   - `src/modules/skills-report/`（`parseSkillsSlashReport` + `SkillsSlashReport` + 所有 normalize/glyph helpers）
   - `src/modules/tool-projection/`（tool 摘要 / glyph / 状态判断 / `summarizeToolResult` 系列）
   原因：纯函数 + 视图，无运行时状态依赖，抽离不会引入 race。

2. **再抽 surface 子组件**：`ProjectFilesRail`（与 `useProjectFiles` 等 hook 一起抽到 `src/modules/project-rail/`）、`EmptyState` / `LoadingIndicator` / `RecoveryCard` / `ErrorCard` 进入 `src/modules/boot-shell/recovery/`。
   原因：这些子组件已有相对清晰的 prop 边界，先抽出后 `ChatUI` 体量立刻下降。

3. **再抽 composer**：`ComposerDock` / `SlashCommandSuggestions` / `AtFileSuggestions` → `src/modules/chat/composer/`。
   原因：composer 与主 transcript 之间通过 prop callback 通信，抽离不破 stream。

4. **再抽 transcript**：`ChatTranscript` / `ChatMessage` / `MemoryStoreToolCard` / `ToolCallMessage` → `src/modules/chat/transcript/`。

5. **最后处理 `ChatUI` 主体**：把剩余 stream / memory / permission / recovery 订阅替换为 M2 stores 的 selector + reducer 派发，让 `ChatUI` 退化为 layout shell。
   原因：必须在 M2 stores / translator / reducers 已就位之后才能做。

### 5.6 risky entanglements

| 风险点                                                                     | 描述                                                               | 应对                                                                           |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| `ChatUI` 100 次 hook 调用集中在单组件                                      | 任何抽子组件如果忘记把 hook 一并搬走，会立即破坏渲染顺序           | 严格按 §5.5 顺序：先抽零状态 helpers，再抽自包含子组件，最后才碰 `ChatUI` 主体 |
| 模型 / 字体 / 密度偏好通过 localStorage key 直接耦合                       | `chatDensityModeV2` / `chatFontModeV2` / `projectRailWidthV1` 散落 | 抽离时统一通过 `src/modules/preferences/` 包一层，保留 key 名以兼容用户旧偏好  |
| `VIRTUAL_LIST_THRESHOLD = 50` 写死                                         | 列表虚拟化触发条件硬编码                                           | 抽到 `src/modules/chat/transcript/` 时不要顺手改阈值，否则会触发性能回归       |
| markdown 缓存 `markdownNormalizeCache = new Map<number, ...>` 是模块级单例 | 抽离时若变成多实例会失去缓存                                       | 在 `src/modules/markdown/` 内保持模块级单例，禁止变成 React state              |
| `SkillsSlashReport` 解析强依赖 backend slash 输出格式                      | 是字符串解析，对后端格式变化敏感                                   | 不在 M0.6 / M2 早期阶段同时改后端格式                                          |
| `RecoveryCard` 当前只在 chat-ui 显示 session 恢复                          | 与 boot-shell recovery 语义重叠                                    | 抽到 `boot-shell/recovery/` 时同步评估是否合并；不在 M2 早期 slice 处理        |

## 6. `src/App.tsx`（2472 行）

### 6.1 current responsibilities

`App` 是单一组件 + 41 次 hook 调用。按职责聚类：

| 聚类                          | 主要内容                                                                                                        |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Splash + onboarding gate      | `showSplash` / `showOnboarding` 状态 + `onboarding_get_state` 3s 超时检查 + `cross:onboarding-reset` 跨窗口监听 |
| Project + session 装载        | `listProjects` / `listSessions` / 当前选中项 + 切换路由                                                         |
| Active model / TTS / STT 偏好 | onboarding 完成后 sync active model + 询问下载 SenseVoice                                                       |
| Window 控件                   | `startWindowDrag`、设置窗口 / 浏览器 viewer 窗口管理                                                            |
| Top-level surface 切换        | chat / settings / browser / memory / specialized panels 之间路由                                                |
| Stream / event listener 订阅  | 通过 `tauri.ts` 的 listener helpers 订阅多源事件                                                                |
| 错误边界 / fallback           | 默认值与超时 fallback（`defaulting to no onboarding`）                                                          |

### 6.2 responsibilities that belong elsewhere

| 当前所在                                        | 应去                                                                     | 理由                                          |
| ----------------------------------------------- | ------------------------------------------------------------------------ | --------------------------------------------- |
| splash + onboarding 检查 + activation gate 切换 | `src/modules/boot-shell/`（M2）                                          | 这是 boot 顺序的视图，不应嵌在 root component |
| project / session 装载与切换                    | `src/runtime/stores/session`（M2） + `src/modules/session-router/`（M2） | 状态需进 store，路由需独立                    |
| active model + TTS / STT 偏好 sync              | `src/modules/preferences/` + `src/runtime/stores/preferences/`           | 偏好与启动顺序解耦                            |
| 设置窗口 / browser viewer 窗口 / 跨窗口事件     | `src/modules/window-bridge/`（M2，新建）                                 | 多窗口编排是独立关注点                        |
| 顶层 surface 切换                               | `src/modules/shell-router/`（M2）                                        | router 应独立可测                             |
| stream / event listener 订阅                    | `src/runtime/translator` + 各 reducer（M2）                              | App.tsx 不应直接监听 envelope                 |

### 6.3 target phase

**全部 M2**。本文件不向 M1 / M3+ 抛出责任，但其拆分依赖：

- M0.3 `RuntimeEventEnvelope`（已建）
- M0.4 `ActivationSnapshot`（已建）
- M2 stores / reducers / translator 已就位

### 6.4 destination module

参见 §3 总表。`App.tsx` 将退化为只组合 `<BootShell>` 与 `<ShellRouter>` 两个壳，目标行数 < 100。

### 6.5 safe extraction order

固定顺序：

1. **先建 `src/modules/boot-shell/`**：迁入 splash + onboarding check + activation gate overlay 的 view 层（无业务状态，状态来自 store）。
2. **再建 `src/runtime/stores/session` 与 `src/modules/session-router/`**：把 project / session 装载从 `App.tsx` 中移出。
3. **再建 `src/modules/preferences/`**：active model / TTS / STT / 字体 / 密度。
4. **再建 `src/modules/window-bridge/`**：跨窗口事件 + 设置窗口 + browser viewer 窗口。
5. **最后建 `src/modules/shell-router/`**：把顶层 surface 切换从 `App.tsx` 中移出。
6. **完成后** `App.tsx` 只剩 `<BootShell>{showShell ? <ShellRouter /> : null}</BootShell>` 这种结构。

### 6.6 risky entanglements

| 风险点                                          | 描述                                                         | 应对                                                                                                 |
| ----------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| splash 关闭与 main 数据装载有时序约定           | `if (!isOnboarding)` 分支里同步装载 projects 后才关闭 splash | 抽到 boot-shell 时必须保留这一时序，否则首屏闪烁                                                     |
| `cross:onboarding-reset` 跨窗口字符串约定       | 设置窗口与主窗口靠字符串事件名通信                           | 抽到 `window-bridge` 时同时登记 event 名常量；不要在抽离 PR 中改名                                   |
| 3s onboarding 超时 fallback                     | `defaulting to no onboarding`                                | 抽离时保留同一超时；显示出错时不要静默升级，应在 boot-shell 里显式渲染 `BootPhase::DegradedRecovery` |
| onboarding 完成后立即装载 projects              | 顺序耦合                                                     | 拆分时使用 store 触发器（"onboarding 完成 → projects 装载"）保持顺序                                 |
| top-level surface 切换通过 useState 而非 router | 浏览器后退 / 深链接缺位                                      | 不在 M2 早期阶段引入 react-router；先做内部 router 抽象，后续再决定是否上路由库                      |

## 7. `src/lib/tauri.ts`（2319 行）

### 7.1 current responsibilities

按代码注释分隔可识别的逻辑分区：

| 段落                       | 行号区间（约） | 职责                                                                                                                                                                                                                                        |
| -------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Memory 事件类型 + listener | 18–168         | `ContextBudgetUsage` / `MemoryEventPayload` + `listenMemoryEvent`                                                                                                                                                                           |
| Memory context item types  | 169–220        | `MemoryContextItem` / `StreamTokenPayload`                                                                                                                                                                                                  |
| Permission types           | 235–250        | `PermissionRequestPayload` / `PermissionMode`                                                                                                                                                                                               |
| Agent turn / stream IPC    | 251–420        | `AgentTurnResponse / ToolCall / TokenUsage / SessionMeta / Project / ProjectMeta / DirectoryEntryPreview / FilePreviewPayload` 等类型 + `runAgentTurn / startAgentStream / listenToStream / listenToPermissionRequests / respondPermission` |
| Session IPC                | 422–470        | `listSessions / deleteSession / setSessionPinned / memorySessionSetEnabled`                                                                                                                                                                 |
| Pinned IPC                 | 472–537        | `PinnedItemDto` + 4 commands                                                                                                                                                                                                                |
| Project IPC                | 538–700        | project commands + 路径 / 文件 preview 工具                                                                                                                                                                                                 |
| Window / 跨窗口 prefill    | 686–718        | settings window + chat prefill                                                                                                                                                                                                              |
| Session 详细类型 + IPC     | 719–870        | `Session / ConversationMessage / ContentBlock / ToolDefinition / ToolCallResult` + `executeTool / listTools / getToolDefinitions / listToolsets`                                                                                            |
| Slash + skills             | 871–1116       | slash 解析 + skills hub install / browse / search + sha256 + skill distribution                                                                                                                                                             |
| Web search 配置            | 1117–1170      | provider CRUD + validate                                                                                                                                                                                                                    |
| Memory 设置                | 1177–1260      | `MemoryRecallMode / MemoryPolicyEnforceMode / PromotionThresholds / MemoryConfig / MemoryConfigInput` + commands + onboarding helpers                                                                                                       |
| Browser 控制               | 1261–1450      | `BrowserStatusEvent` + 一系列 browser session / profile / settings / takeover IPC + `listenBrowserStatus`（约 1405 行）                                                                                                                     |
| Browser viewer 窗口        | 1428–1448      | viewer window navigation                                                                                                                                                                                                                    |
| Harness IPC                | 1448–end       | harness control / telemetry IPC                                                                                                                                                                                                             |

### 7.2 responsibilities that belong elsewhere

| 当前所在                                                                                                                               | 应去                                                                               | 理由                                                                     |
| -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| 所有 `interface` / `type` 声明（≈40+）                                                                                                 | `src/transport/contracts.ts`（M0.3 已建）+ 各 feature module 的 `types.ts`         | tauri.ts 不应同时是 transport bridge 与 type center                      |
| 所有 `listen*` helper（`listenMemoryEvent / listenToStream / listenToPermissionRequests / listenBrowserStatus / listenToChatPrefill`） | `src/runtime/translator/`（M2）                                                    | listener 必须经 translator 转 envelope-shaped action，不应直接消费裸事件 |
| Memory IPC + 类型                                                                                                                      | `src/transport/memory.ts`（thin invoke wrapper） + 类型回 `transport/contracts.ts` | feature 切片                                                             |
| Browser IPC + 类型                                                                                                                     | `src/transport/browser.ts` + types in `transport/contracts.ts`                     | feature 切片                                                             |
| Session / Project IPC + 类型                                                                                                           | `src/transport/session.ts` / `transport/project.ts` + types                        | feature 切片                                                             |
| Skills / Slash IPC                                                                                                                     | `src/transport/skills.ts` + types                                                  | feature 切片                                                             |
| Web search + memory config + onboarding helpers                                                                                        | `src/transport/settings.ts` + `transport/onboarding.ts`                            | feature 切片                                                             |
| Harness IPC                                                                                                                            | `src/transport/harness.ts` + types                                                 | feature 切片                                                             |
| `sha256Hex` / `installSkillFromDistribution` 中的非 IPC 业务逻辑                                                                       | `src/modules/skills/install/`（M2）                                                | tauri.ts 不应承载业务逻辑                                                |

### 7.3 target phase

- **类型层（M0.3 已开始）+ 全部其它责任 M2**。
- M0.3 已有 canonical types（`RuntimeEventEnvelope` / `ActivationSnapshot` / `ExecutionMode*` / `MemoryProjection` 等）。M2 切片要逐步把 `tauri.ts` 内类型迁入 `src/transport/contracts.ts` 或 feature module 内 `types.ts`。
- 不在 M1 阶段碰 `tauri.ts`（M1 只动后端）。

### 7.4 destination module

- 类型：`src/transport/contracts.ts`
- 各 feature 的 thin invoke wrapper：`src/transport/{memory,browser,session,project,skills,settings,onboarding,harness}.ts`
- 业务侧：`src/modules/<feature>/`
- 事件 listener 全部走 `src/runtime/translator/`

### 7.5 safe extraction order

固定顺序：

1. **先迁类型**：把 `tauri.ts` 内已被 `contracts.ts` 覆盖的类型（`MemoryProjection / ActivationSnapshot / ExecutionMode*`）逐个 `re-export from '@/transport/contracts'`，并在调用方逐步切换 import 来源。原因：先稳定类型，再切运行时。
2. **按 feature 切 thin invoke wrapper**：每次只切一个 feature（建议顺序：browser → session → project → skills → settings → harness → memory）。每切完一个 feature，对应 `tauri.ts` 区段直接删。原因：feature 之间不耦合，按 PR 切片小步走。
3. **listener helpers 最后切**：`listenMemoryEvent / listenToStream / listenToPermissionRequests / listenBrowserStatus / listenToChatPrefill` 必须在 M2 translator 落地后才迁，否则会出现"裸事件无 reducer 接住"的窗口期。原因：listener 的迁移即"运行时 truth 来源切换"，必须有承接方。
4. **`tauri.ts` 退化为仅剩 deprecated re-export**：当所有 feature 切完，`tauri.ts` 留一段过渡 re-export shim，标 `@deprecated`，由 M2 末尾 slice 决定何时删除。

### 7.6 risky entanglements

| 风险点                                                                                                                    | 描述                             | 应对                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| 调用方众多（`ChatUI` / `App.tsx` / 各 settings 页）                                                                       | 改 import 路径需要扫描全仓       | 每个 feature 切片单独 PR；不要在同一 PR 同时改多个 feature 的 import                                                            |
| 多个事件名（`memory_event / browser-status / agent stream / permission_prompt / chat_prefill`）通过字符串字面量与后端约定 | 耦合点散落                       | 在 listener 迁移到 translator 时建立事件名 const 注册表，禁止再出现裸字面量                                                     |
| `installSkillFromDistribution` 中的 `sha256Hex` + 解码 + 安装 编排逻辑                                                    | tauri.ts 当前不仅是 bridge       | 抽到 `src/modules/skills/install/` 并补单测；tauri.ts 端只保留 invoke                                                           |
| Tauri runtime detection（多处 `try/catch isTauri`）                                                                       | 容易在迁移过程被忘               | 把 isTauri 判断收敛到 `src/transport/runtime.ts` 单点                                                                           |
| Memory / Browser 事件 payload schema 与后端紧耦合                                                                         | M0.3 envelope 尚未在后端实际发射 | 迁移期间允许双轨：`tauri.ts` listener 临时把裸事件 wrap 成 envelope-shaped 对象供 translator 消费；正式 envelope 发射在 M1 完成 |

## 8. 跨文件汇总

| dimension                                           | 数据                                                                                                                            |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| 总行数                                              | 13587                                                                                                                           |
| Phase M1 接收（仅 `commands/agent.rs`）             | 4060 行                                                                                                                         |
| Phase M2 接收（`App.tsx + chat-ui.tsx + tauri.ts`） | 9527 行                                                                                                                         |
| 不在本轮拆分的原始文件                              | `src-tauri/src/commands/mod.rs`、`src-tauri/src/main.rs`（boot 装配，由 M1 / M2 中的对应 service 升级，不属于 god-file 主清单） |

## 9. 与 M0 其它产出的关系

- 命名 / 实体定义：见 [if2ai-canonical-domain-model.md](./if2ai-canonical-domain-model.md)
- workflow 真相：见 [if2ai-workflow-truth.md](./if2ai-workflow-truth.md)（特别是 §3.4 chat_prompt_dispatch、§3.5 stream_projection、§3.17 boot truth）
- contract 入口：[src-tauri/src/modules/runtime/contracts/](../../src-tauri/src/modules/runtime/contracts/)、[src/transport/contracts.ts](../../src/transport/contracts.ts)
- 设计依据：[backend-application-control-plane-refactor-design.md](./backend-application-control-plane-refactor-design.md)、[runtime-contracts-and-event-projection-design.md](./runtime-contracts-and-event-projection-design.md)、[frontend-information-architecture-ui-redesign.md](./frontend-information-architecture-ui-redesign.md)
- 上位执行计划：[phase-m1-backend-service-extraction.yaml](../exec-plans/active/phase-m1-backend-service-extraction.yaml)、[phase-m2-frontend-runtime-projection.yaml](../exec-plans/active/phase-m2-frontend-runtime-projection.yaml)

## 10. 维护规则

1. 任何 M1 / M2 slice 只要触及四大 god-file，PR 描述必须引用本文件对应章节并指出本 PR 抽走了哪些责任。
2. 当某文件行数下降明显时，PR 必须同步更新 §2 表格行数。
3. 每完成 §4.5 / §5.5 / §6.5 / §7.5 中的某一步，必须 PR 本文件，把对应步骤打 ✅ 并简述退场代码段。
4. 不允许在不更新本文件的情况下把责任抽到 §3 共用 destination map 之外的目录。

# If2Ai Desktop 前端/系统 QA 审计

日期: 2026-04-25
角色视角: Staff 系统架构师 + 前端 UI/UX 设计师
测试对象: `/Applications/If2Ai.app`，bundle id `dev.if2ai.desktop`

## 结论摘要

本轮测试确认已安装的 `/Applications/If2Ai.app` 是最新 debug build 的 0.4.0 bundle，基础聊天链路可正常完成一次无工具调用的模型回复；本轮未复现 `reasoning_content missing` / `request_validation_error` / `max_iterations_reached`。但当前桌面 App 仍存在几类会影响用户信任和配置闭环的问题：版本来源不一致、Provider 详情面板未回填已保存模型、thinking 原文默认展开暴露、打包启动路径/内存初始化路径出现双轨日志、以及中文产品界面中仍混入英文主标题。

## 测试环境与证据

- 已安装 App: `/Applications/If2Ai.app`
- 运行进程: `/Applications/If2Ai.app/Contents/MacOS/if2ai-backend`
- Bundle 元数据: `CFBundleIdentifier=dev.if2ai.desktop`, `CFBundleShortVersionString=0.4.0`, `CFBundleVersion=0.4.0`
- 前端水印实测: 右下角显示 `SF95-YA9D · v0.3.0`
- 代码版本源: `package.json` 为 `0.3.0`，`src-tauri/Cargo.toml` 为 `0.3.0`，`src-tauri/tauri.conf.json` 为 `0.4.0`
- 本轮聊天 run-log: `/Users/ryanliu/Library/Application Support/dev.if2ai.desktop/runtime/run-log/f79ae6f5-6347-4cc6-90dd-bfd4054c08b5/f1982e69-b29d-4c62-96a4-5e317e554f42.jsonl`
- 后台日志: `/Users/ryanliu/.if2ai/log/backend.log.2026-04-24`
- 模型配置文件: `/Users/ryanliu/.if2ai/models.json`

## 已验证正常

- 基础聊天发送成功: 输入 `OK` 后创建新线程，模型回复正常展示。
- run-log 收尾正常: `event_type=stream_complete`, `task_outcome=completed`, `resume_available=false`。
- 后台流诊断正常: `status='model_stop_no_tools'`, `degraded_reason='none'`, `tool_loop_iter=1`, `last_stream_error=none`。
- 当前 `models.json` 已写入 `supportsThinking` 字段，例如 `moonshot/kimi-k2.6`、`ollama/minimax-m2.7:cloud` 均有 `supportsThinking: true`。
- 聊天 composer 当前显示 `Moonshot (Kimi) / kimi-k2.6`，与当前主聊天模型配置一致。

## Findings

### P1: TTS idle eviction 后台线程触发 Tokio nested runtime panic

现象: 继续浏览设置和记忆页期间，后台日志出现 panic。进程仍存活，但 TTS idle evictor 线程已经 panic，属于稳定性问题；如果后续状态依赖该线程，TTS provider eviction / reload 状态可能失真。

证据:
- 日志时间: `2026-04-24T17:47:11.081031Z` 先出现 `TTS idle threshold reached; evicting provider (release RAM)`。
- 紧接着日志: `Cannot start a runtime from within a runtime. This happens because a function (like block_on) attempted to block the current thread while the thread is being used to drive asynchronous tasks.`
- `src-tauri/src/modules/tts/manager/eviction.rs:94` 在 evictor 自建 current-thread runtime 内 `rt.block_on(async move { ... })`。
- `src-tauri/src/modules/tts/manager/eviction.rs:124` 在该 async loop 内同步调用 `on_evict`。
- `src-tauri/src/commands/host_composition.rs:93` 的 `on_evict` 回调又调用 `tauri::async_runtime::block_on(...)` 写 `ProviderState::Evicted`，形成 nested runtime。

建议:
- 不要在 evictor runtime 内调用同步 `on_evict` 再 `block_on`。将回调改成 async callback 并在当前 evictor runtime 内 `await`，或把状态更新逻辑直接内联到 evictor async loop。
- 另一种低风险修法: `on_evict` 只做 lock-free 状态标记，避免任何 async lock；需要写 `RwLock` 时通过 `tokio::spawn`/channel 交给已有 runtime。
- 增加回归测试: TTS provider idle 超过阈值后，日志不得出现 nested runtime panic，状态应从 `Ready` 变为 `Evicted`。

### P1: Thinking 原文默认展开，暴露内部推理与记忆提示

现象: 简单输入 `OK` 后，聊天消息上方直接显示完整 “已完成思考” 内容，里面包含模型对用户偏好的引用和内部判断逻辑。对普通用户来说，这会显得嘈杂；对产品安全边界来说，也不应默认暴露完整 chain-of-thought 风格文本。

证据:
- UI 实测中 thinking block 展开显示完整文字。
- `src/components/ui/chat-ui.tsx:3193` 对 primary thinking message 渲染 `<ThinkingBlock ... defaultOpen />`。
- `src/components/ui/chat-ui.tsx:4833` 直接渲染 `{thinking}`。
- `src/components/chat/ThinkingBlock.tsx:37` 注释也将该字段定义为 raw chain-of-thought text。

建议:
- 默认只显示“已完成思考”摘要，不默认展开原文。
- 对用户可见层只展示 sanitized summary，例如“已完成上下文判断 / 已加载 1 条记忆 / 未使用工具”。
- 原始 thinking 若必须保留，应只进入调试面板或开发者模式，并明确区分 provider reasoning 与应用生成的观察摘要。

### P1: Provider 详情面板没有回填已保存模型，导致“左侧有模型，右侧显示 0”

现象: 设置页服务商列表显示 `Moonshot (Kimi) 3`、`Ollama (本地) 2`，但点击进入详情时右侧显示 `已添加的模型 0`，并提示“点击「读取模型」从供应商加载可用模型”。这会让用户误以为模型没有保存成功。

证据:
- `/Users/ryanliu/.if2ai/models.json` 中 `moonshot` 有 3 个模型，`ollama` 有 2 个模型，且都带 `supportsThinking`。
- `src/modules/settings/pages/ProvidersSettingsPage.tsx:252` 切换 provider 时清空 `availableModels`、`selectedModelIds`、`probeResults`。
- `src/modules/settings/pages/ProvidersSettingsPage.tsx:260` 只调用 `getProviderConfig(provider.id)` 回填 key/baseUrl。
- `src/modules/settings/pages/ProvidersSettingsPage.tsx:499` 右侧计数来自 `selectedModelIds.size`，而不是持久化模型列表。
- `src/modules/settings/pages/ProvidersSettingsPage.tsx:527` 在 `availableModels.length === 0` 时显示“读取模型”空态。

建议:
- 增加 `provider_get_configured_models(providerId)` 或扩展现有 provider config 返回 persisted models。
- 进入详情页时将 `models.json` 已保存模型回填为 `availableModels`，并将其 id 写入 `selectedModelIds`。
- UI 文案区分“已保存模型”和“可从服务商读取的远端模型”，不要把未读取远端列表误报为 0。

### P1: Memory Browser 浏览会污染 `access_count`，进而污染候选晋升

现象: 只是在记忆浏览器里查看/清除搜索，列表中的多条记忆访问次数从 6 增加到 7。随后“候选晋升”面板立即把这些条目推荐为 `session → project`。这会让人工浏览管理界面的行为影响 agent 真实记忆热度，进而放大重复/低质量记忆。

证据:
- UI 实测: 初始列表显示 `访问 6 次`，清除搜索返回列表后显示 `访问 7 次`。
- `src/components/memory/MemoryBrowser.tsx:138` 注释说明浏览器故意走 `memory_recall({ query: '' })`。
- `src/components/memory/MemoryBrowser.tsx:139` 注释说明 recall 路径会 bump 每条命中的 `access_count`。
- `src/components/memory/MemoryBrowser.tsx:146` 使用 `memoryRecall({ query: '', limit: 1000 })` 加载列表。
- `src/components/memory/MemoryBrowser.tsx:426` 候选晋升说明依据 `importance & 访问频次推荐`。

建议:
- Memory Browser 默认应走只读 `memory_export` 或新增 `memory_list_entries`，不要 bump `access_count`。
- 若产品确实想记录“人看过”，应单独建 `admin_view_count`，不要混入 agent recall 的 `access_count`。
- 候选晋升应基于 agent recall/use count，而不是 UI 管理浏览次数。

### P1: 打包 App 启动路径出现双轨初始化日志

现象: 同一天后台日志中出现两套启动形态：一套使用 `ChatProviderUtilityLlm` 和 `/Users/ryanliu/.if2ai`；另一套使用 `MockUtilityLlm placeholder` 和 `/Users/ryanliu/Library/Application Support/.if2ai`。这说明 packaged/dev/旧入口之间仍可能存在 storage root 或 bootstrap path 分叉。

证据:
- 正常路径: `[init] UtilityLlm bound to ChatProviderUtilityLlm (workdir="/Users/ryanliu/.if2ai")`
- 异常路径: `[init] UtilityLlm bound to MockUtilityLlm placeholder; real provider wiring deferred...`
- 异常路径同时写入 `/Users/ryanliu/Library/Application Support/.if2ai/memory/memory.db`

建议:
- 统一 packaged App、`npx tauri dev`、直接 backend 启动的 config root 解析。
- 启动日志增加 `build_profile`、`bundle_version`、`config_root`、`app_data_dir`、`argv[0]`，方便区分到底是哪条入口。
- 如果 `/Users/ryanliu/Library/Application Support/.if2ai` 是旧路径，应提供一次性迁移或明确废弃。

### P2: App 版本号三源不一致

现象: 已安装 bundle 是 `0.4.0`，但前端右下角显示 `v0.3.0`；设置页“关于我们”也显示 `v0.3.0`，因此不是单个 watermark 组件错误，而是前端编译期版本源整体落后。

证据:
- `src-tauri/tauri.conf.json:3` 为 `0.4.0`。
- `package.json:3` 为 `0.3.0`。
- `src-tauri/Cargo.toml:3` 为 `0.3.0`。
- `src/lib/appVersion.ts:3` 注释声称 `package.json`、`tauri.conf.json`、`Cargo.toml` 会被 release script 同步。
- `vite.config.ts:31` 用 `package.json` 的 version 注入 `__APP_VERSION__`。

建议:
- 短期修复: 将 `package.json` 和 `src-tauri/Cargo.toml` 同步到 `0.4.0`。
- 中期修复: CI 增加版本一致性检查，阻止三源漂移。
- 产品层建议: About/水印显示 `frontend/backend/bundle` 三个版本，调试时更容易定位“老 App”误会。

### P2: Vector memory 初始化在部分 packaged 启动中降级

现象: 部分启动日志中 `VectorMemoryProvider` 初始化失败，提示 `Failed to retrieve onnx/model.onnx` 并 fallback 到 SQLite。近期也有正常初始化成功的记录，因此这是环境/路径/资源解析相关的间歇性风险。

证据:
- 失败日志: `[memory] VectorMemoryProvider initialization failed: embedding failed: embedding model failed: Failed to retrieve onnx/model.onnx, falling back to SQLite`
- 成功日志: `[memory] VectorMemoryProvider initialized successfully`

建议:
- 启动时输出 embedding model resolved path 和存在性检查。
- UI 记忆页增加“向量检索已启用 / SQLite 降级”状态。
- 打包时加入资源完整性检查，避免 packaged App 与 dev App 表现不同。

### P2: `self_repair` watchdog 多次在无 Tokio runtime 处降级到 dedicated thread

现象: 日志反复出现 `no current Tokio runtime at watchdog spawn site; falling back to dedicated thread`。这不是用户立即可见 bug，但说明 runtime 边界仍不够整洁。

证据:
- `if2ai_backend::modules::scheduler: [self_repair] no current Tokio runtime at watchdog spawn site`
- `if2ai_backend::modules::runtime::self_repair: [self_repair] no current Tokio runtime at watchdog spawn site`

建议:
- 将 watchdog spawn 收敛到受控 runtime bootstrap 阶段。
- 把 fallback 从 warning 降噪为 debug，或修复调用点，避免真实 warning 被淹没。

### P2: 记忆候选晋升是高影响操作，但按钮缺少预览/确认

现象: 候选晋升面板内每条候选右侧直接提供“应用”按钮，按钮说明为“应用此晋升”。这会改变记忆 scope，例如 `session → project` 或 `project → global`，属于会影响后续所有会话上下文的高影响操作。

建议:
- 点击“应用”前展示确认弹层，说明将移动的 key、源 scope、目标 scope、影响范围和可回滚方式。
- 提供批量预览，但不要默认批量执行。
- 对重复候选先提示合并/去重，不应把重复记忆分别晋升。

### P2: Memory Browser 存在重复/同义记忆，且候选晋升会放大重复

现象: 列表中同时出现 `Episode:Session:food_preference_stir_fried_dough...` 和 `food_preference_stir_fried_dough`，内容都表达 “Ryan Liu 喜欢吃炒饼”；牛羊肉偏好也出现多个相近条目。

建议:
- 记忆写入或编译阶段增加 canonical key / semantic dedupe。
- 候选晋升前增加 duplicate cluster 展示，让用户先合并再晋升。
- trust score 为 `0.00` 且标记“存疑”的条目不应仅凭访问次数进入晋升推荐。

### P2: Usage 统计对未知模型使用默认 Sonnet 价格，成本展示可能误导

现象: 用量统计页显示今日成本 `$15.54`；后台日志多次出现 `no pricing for model; using default_sonnet_tier as fallback model=kimi-k2.6`。这说明 Kimi 模型没有真实价格表时，被按默认 Sonnet tier 估算。

建议:
- UI 成本处显示 `~估算` 或 `价格未知，按默认档估算`。
- Provider/Model 配置支持设置 per-1M input/output price。
- 未配置价格时，不应用高置信美元成本表达，应拆成 token usage 与 estimated cost。

### P3: 模型角色下拉包含大量空 Provider 分组，噪声偏高

现象: 模型配置页下拉能看到已保存模型，说明同步主链路当前正常；但菜单同时展示 `ANTHROPIC`、`BYTEPLUS`、`DEEPSEEK`、`GOOGLE` 等大量空分组，真实可选模型被夹在空分组之间。

建议:
- 默认隐藏无可用模型的 provider 分组。
- 需要展示生态全量 provider 时，放到“添加 Provider”入口，不放在模型选择菜单。
- 为支持 thinking 的模型展示统一 badge，但不要让 badge 挤占模型名。

### P3: 记忆搜索空态不显示“0 / 22”或当前过滤条件

现象: 记忆页搜索 `拓竹` 后显示“暂无记忆条目”，顶部原先的 `共 22 条记忆` 消失。用户无法判断是全库 0 条、当前 scope 0 条，还是当前 search 0 条。

建议:
- 搜索后展示 `0 / 22 条匹配`。
- 空态文案包含 query 和 scope，例如 `未找到包含“拓竹”的全部范围记忆`。
- 输入框内容变化时可提供 debounce live search，但显式搜索也可保留。

### P3: Prompt Diagnostics 的视觉选中与可访问焦点不同步

现象: Prompt Diagnostics 页面视觉上已选中，但 accessibility tree 中 focused element 仍显示为左侧 `用量统计` 按钮。这个问题不影响鼠标用户主路径，但会影响键盘导航、屏幕阅读器和自动化测试稳定性。

建议:
- 切换设置页时同步 focus 到页面标题或当前 nav item。
- nav item 使用 `aria-current="page"`，而不是仅靠视觉 class。
- 自动化测试可加断言: 视觉 active route 与 accessibility selected/current 一致。

### P3: 全局搜索范围目前只覆盖线程/项目

现象: 全局搜索输入 `openai` 能即时过滤线程，基础体验良好；但未看到统一检索记忆、设置项、工具、Provider 或命令入口。

建议:
- 保留当前轻量线程/项目搜索作为默认。
- 增加分组: 线程、项目、记忆、设置、命令。
- 对高风险命令只跳转到页面，不直接执行。

### P3: “检查更新”是 dead CTA

现象: 主导航底部和关于页都有“检查更新”按钮，但主导航按钮点击后没有任何可见反馈，也没有明显后台日志。代码层主导航按钮直接绑定空函数。

证据:
- `src/modules/app-shell/components/GlobalNavbar.tsx:84` 渲染 `检查更新`。
- `src/modules/app-shell/components/GlobalNavbar.tsx:88` 为 `onClick={() => {}}`。
- `src/modules/settings/pages/AboutSettingsPage.tsx:73` 也渲染 `检查更新` 按钮，但未看到实际 handler。

建议:
- 未实现 updater 前隐藏按钮，或 disabled 并标注“即将上线”。
- 如果保留按钮，至少 toast “当前版本 / 暂未接入自动更新”。
- 接入真实 updater 后，按钮需显示 checking / latest / update available / failed 四态。

### P3: 字母排序会让当前活跃项目离开顶部

现象: 线程列表从“最近活跃”切到“字母排序”后，当前活跃项目 `Work` 被移动到项目列表较下方，当前线程仍高亮但位置变远。功能本身成立，但容易让用户在大项目列表中迷路。

建议:
- 排序后保持当前项目吸顶，或提供“当前项目”固定区。
- 切换排序时添加轻量 toast 或过渡，解释列表顺序变化。
- 若有置顶项目，应明确置顶优先级高于排序策略。

### 已验证: TTS 线程 panic 后仍可 lazy reload，但仍需修 panic

现象: 触发 TTS idle eviction panic 后，点击聊天消息的“播放语音”，前端进入“合成中…”并恢复为“播放语音”；后台日志显示重新加载 `OnnxTtsProvider` 成功。这说明用户短期可恢复，但 panic 仍然会污染日志并破坏 evictor 线程健康。

证据:
- 后台日志: `TTS provider loaded (OnnxTtsProvider) model_dir=/Users/ryanliu/.if2ai/models/tts`
- 前端按钮状态: `播放语音` → `合成中…` → `播放语音`

建议:
- 保留 lazy reload 行为。
- 修复 evictor nested runtime panic 后，再补一条端到端测试: idle evict 后再次播放应成功，且日志没有 panic。

### P3: 中文 UI 中混入英文主标题

现象: 中文界面下首页主标题显示 `What should we build in Work？`，与侧栏、设置页、composer placeholder 的中文语言环境不一致。

建议:
- locale 为中文时改为“今天想在 Work 里完成什么？”或“我们今天在 Work 里构建什么？”。
- 项目名 `Work` 可保留英文，但句子主体应跟随应用语言。

### P1: 历史聊天恢复只显示用户消息，assistant/tool 没接回 projection store

现象: 重新打开历史会话时，用户消息会显示，但历史 assistant/tool 内容容易缺失或不稳定。当前代码并不是简单“没保存 assistant”，session JSON 里有 assistant 消息；问题出在 MIG-017/T-003 后，聊天渲染层只接受 user anchor，再从 runtime projection 派生 assistant/tool。

证据:
- `src/App.tsx:1180` 的 `activeMessages` 明确只保留 `msg.role === "user"`。
- `src/App.tsx:1458` 加载历史时调用 `getSession(sessionId)` 并把 `fullSession.messages` 转成 `convertedMessages`。
- `src/App.tsx:1579` 把 `convertedMessages` 写进 legacy conversation slice，但没有调用 `getSessionHistoryPage()`，也没有把 durable run-log replay 进 `runtimeProjectionStore`。
- `src/api/sessions.ts:119` 已有 `getSessionHistoryPage()`；`src/runtime-projection/history-replay.ts:10` 已有 `replayRunLogEntriesToMessages()`，但主会话加载路径未接入。
- 实测 session 文件 `/Users/ryanliu/.if2ai/projects/90951f44-e1f7-4b61-b7ef-44d3ed081381/sessions/f79ae6f5-6347-4cc6-90dd-bfd4054c08b5.json` 保存了 4 条消息，其中 2 条 assistant 均存在。

根因判断:
- 这是 projection cutover 后的主路径缺口，不只是 TODO 文档问题。
- 现有 TODO/注释覆盖了“assistant/tool 应来自 projection store”的方向，但没有完成“历史 run-log replay 到 projection store”的接线，因此用户可见问题仍未修。

建议:
- `handleSelectSession()` 优先调用 `getSessionHistoryPage(sessionId)`，把 event-log replay 成 projection events 或直接 hydrate projection snapshot。
- fallback 到 `getSession()` 只用于 event-log 为空或损坏时，并在 UI/日志中标记 fallback。
- 增加回归: 打开已有会话后，用户消息、assistant 回复、thinking 摘要、tool cards、turn cost/status 均能稳定复现。

### P1: Persona 快捷切换 UI 成功，但 session override 没有成为本轮有效身份

现象: 在已有会话 `OK` 中，点击左上角 Persona badge 可展开 `Switch Persona` 菜单；选择 `Staff Architect / 资深架构师` 后，头像更新并弹出“已切换 Persona”。随后发送“用一句话说明你当前的 Persona。”，模型仍回答当前是 `quitemood`，不是刚选的 Staff Architect。

证据:
- `src/modules/chat/components/SidebarTop.tsx:212` 只有存在 `activeSessionId` 时才允许切换；空白新对话中按钮禁用，已有会话中启用。
- `src/modules/chat/components/SidebarTop.tsx:225` 会先写 optimistic UI；`src/modules/chat/components/SidebarTop.tsx:239` 成功后显示 toast。
- 实测 toast: `当前会话现在以 Staff Architect / 资深架构师 模式协作`。
- 但 session 文件中 `soul_id: null`、`persona_id: null`，说明持久 override 没落到当前会话或被后续保存覆盖。
- 同一轮后台日志 `2026-04-24T18:04:50.841369Z` 输出: `persona_id 'quitemood' was not found; dropping persona`。
- 模型最终回复: `我是 Minami，当前以 quitemood Persona 模式与你协作。`

根因判断:
- 前端 optimistic badge/toast 与后端持久身份状态存在双重事实。
- 即使全局默认 `quitemood` 存在于 `~/.if2ai/prompt/identity-pack.json` 的 `custom_personas`，turn_service 的 identity resolution 仍使用 built-in registry。
- `src-tauri/src/modules/application/turn_service/mod.rs:387` 调用 `resolve_identity(&IdentityRegistry::builtin(), ...)`，没有合并 custom identity pack；而 `src-tauri/src/commands/session.rs:47` 的 session identity 命令已经使用 effective registry。这造成设置/切换路径与实际 turn path 不一致。

建议:
- turn_service identity resolution 改为 `apply_identity_customization_pack(&IdentityRegistry::builtin(), &read_identity_customization_pack())`，与 settings/session command 保持一致。
- Persona 切换 toast 必须以重新读取的 `SessionMeta.persona_id` 为准；若保存后 session 仍为空，应报错并回滚 optimistic UI。
- prompt 中不要把 raw `control-plane.json` 作为模型可读事实暴露给 LLM，至少应注入 resolved identity，而不是让模型从 `defaultPersonaId` 自行推断。
- 增加回归: 切换 session persona 后，session JSON 持久化为对应 `persona_id`，下一轮 prompt diagnostics 出现 `identity:persona@staff-architect`，模型自述与 UI 一致。

### P2: Persona 与 TTS Profile 在顶部同时出现，视觉语义容易混淆

现象: 顶部居中 `默认对话 1.00×` 是 TTS Profile 菜单，左上角头像 `Minami / 静心` 才是 Persona 菜单。两者都使用“对话/身份/模式”语义，用户很容易把语音 Profile 当成 Persona。

建议:
- TTS pill 文案改成“语音: 默认对话 1.00×”，Persona badge 增加显式 tooltip 或短标签“Persona”。
- 新对话空态下如果 Persona 无法切换，应提供 disabled reason，例如“发送第一条消息后可为当前会话切换 Persona”，或支持预设“下一条消息创建 session 时使用此 Persona”。

### P1: 复杂工具任务中 `file_write` 反复空参数失败，前端长时间卡在写文件

现象: 在编译版 `/Applications/If2Ai.app` 中发送“制作单文件 HTML 超级玛丽风格网页游戏”的任务，模型先成功调用 `TodoWrite`，随后连续尝试 `file_write`，但每次传入工具执行器的参数都是 `{}`。UI 显示多个“执行失败: 已写入文件”，但任务面板仍显示“2 个任务进行中 / 正在写入文件”，没有产生文件，也没有给用户可理解的失败总结。

实测输入:
- `请在当前工作目录制作一个单文件 HTML 网页游戏：超级玛丽风格横版跳跃小游戏。要求使用原生 HTML/CSS/JS，不依赖外部资源；包含玩家、平台、金币、敌人、碰撞、分数、胜利/失败状态、键盘控制；请实际创建文件并最后告诉我文件路径和如何打开测试。`

证据:
- session id: `08477032-10d7-4073-89ff-e6998422ef71`
- run id: `cd8bf816-a176-4af6-89dd-7a403a387a55`
- stream id: `bc74761a-bdb6-4a64-9151-902fd1ce2f68`
- run-log: `/Users/ryanliu/Library/Application Support/dev.if2ai.desktop/runtime/run-log/08477032-10d7-4073-89ff-e6998422ef71/cd8bf816-a176-4af6-89dd-7a403a387a55.jsonl`
- backend log: `/Users/ryanliu/.if2ai/log/backend.log.2026-04-24`
- 后台确认工作目录为 `/Users/ryanliu/Downloads/05_其他`，权限为 `danger-full-access`，不是权限拒绝。
- `TodoWrite:0` 正常完成；之后 `file_write:1`、`file_write:2`、`file_write:3` 均失败。
- run-log 中三次失败均为 `tool_args={}`，`tool_result=tool handler error: missing required parameter: path`。
- 手动停止后又出现第 4 次 `file_write:4` 空参数失败，说明停止信号没有中断正在等待的 provider stream。
- 文件系统检查 `/Users/ryanliu/Downloads/05_其他` 最近 10-15 分钟内无新文件产物。
- 后台在每次工具失败后继续 outer loop: `Tool execution done, continuing outer loop. session_messages len=5/7/9`。
- 每轮都会追加 `padding placeholder reasoning_content for assistant tool_call history`，但本次没有复现 `request_validation_error: reasoning_content is missing`。
- 手动停止链路: `18:24:26.682509Z` 后台记录 `Cancel signal sent`，但 `18:25:18.168446Z` 才记录 `Stream cancelled at loop iteration`，实际后台取消延迟约 51.5 秒；最终 `18:25:18.219887Z` 才输出 `stream_diag_summary status='cancelled_by_user'`。
- 历史样本: `bd4ff18f-a4fa-4b1a-a168-0b7e81762e43/d55d5d32-5a38-4ed7-b3cd-56e06250328d.jsonl` 中也出现 `file_write:13` 空参数失败，后台同段日志显示模型为 `kimi-k2.6`，随后该轮触发 `reasoning_content is missing` 400。
- 同一 session 中切换 composer 到 `Ollama (本地) / minimax-m2.7:cloud` 后复测，run id 为 `bd0ff284-938b-4678-8a71-8f4dfba8090f`，stream id 为 `70aa9df4-3a28-42ba-a9fe-9bdd9b4f11c0`，run-log 为 `/Users/ryanliu/Library/Application Support/dev.if2ai.desktop/runtime/run-log/08477032-10d7-4073-89ff-e6998422ef71/bd0ff284-938b-4678-8a71-8f4dfba8090f.jsonl`。
- 该复测中 `~/.if2ai/config.json` 的 `active_model` 已写为 `ollama/minimax-m2.7:cloud`，但 `role_models.chat` 仍是 `moonshot/kimi-k2.6`，后台用量归因仍输出 `model=kimi-k2.6`。因此这不是一次干净的 Ollama provider 复现，而是暴露出“composer 模型选择 / active_model / role model”三者真相源断裂。
- 同一复测中事件计数为 `TodoWrite` 成功 1 次、`file_write` 失败 1 次、`text_delta` 4089 次；失败仍是 `tool_args={}`，`tool_result=tool handler error: missing required parameter: path`。之后模型把 `{"path": ".../super-mario-game.html", "content": "..."}` 作为普通 assistant 文本输出，未再次通过工具创建文件。
- 同一复测最终后台诊断为 `status='model_stop_no_tools'`、`task_outcome='completed'`，但文件系统检查 `/Users/ryanliu/Downloads/05_其他` 没有新文件。因此当前成功判定只看模型 stop/text 完成，不会把“用户要求创建文件但 mutating tool 失败且无产物”降级为失败或部分完成。
- 在设置页把 `chat` role 明确改为 `ollama/minimax-m2.7:cloud` 后再次复测，run id 为 `b2b89f73-d1d6-4a84-9a40-ef0d2af6d731`，stream id 为 `81540fe4-c9b1-49c0-87dd-c28d5f4e4a19`，run-log 为 `/Users/ryanliu/Library/Application Support/dev.if2ai.desktop/runtime/run-log/08477032-10d7-4073-89ff-e6998422ef71/b2b89f73-d1d6-4a84-9a40-ef0d2af6d731.jsonl`。
- 这次后台最终用量归因明确为 `model=minimax-m2.7:cloud`，因此是有效的 Ollama 云模型路径复现，不再是 stale `role_models.chat` 污染。
- 这次事件计数为 `tool_call_failed=7`、`tool_call_completed=1`、`tool_call_queued=8`、`tool_loop_iter=9`；失败包括两次 `file_write {}` 缺 `path`、一次 `file_write {"path": ...}` 缺 `content`、四次 `bash {}` 缺 `command`。唯一成功工具是 `bash {"command":"echo \"test\" && pwd"}`，说明工具执行器本身可工作，失败集中在模型/adapter 传入参数为空或不完整。
- 后台在 `18:46:37.061428Z` 记录 `repeated tool batch detected; next iteration will force final summary`，第 9 轮进入 `force_final_response=true, finalization_reason=repeated_tool_batch_no_progress`。这避免了继续无限空转，但最终 `stream_diag_summary` 仍是 `status='model_stop_no_tools'`、`task_outcome='completed'`、`has_successful_mutating_tool=false`。
- 文件系统检查 `/Users/ryanliu/Downloads/05_其他` 最近 30 分钟内无新文件，说明“最终 completed”与用户目标不一致。

代码定位:
- `src-tauri/src/modules/tools/builtin/file_write.rs:32` 要求 `path`，`src-tauri/src/modules/tools/builtin/file_write.rs:38` 要求 `content`；工具 schema 在 `src-tauri/src/modules/tools/builtin/file_write.rs:153` 已声明 `required: ["path", "content"]`。
- `src-tauri/src/modules/api/providers/openai_compat.rs:561` 会累积 OpenAI-compatible streaming tool-call `function.arguments`。
- `src-tauri/src/modules/api/providers/openai_compat.rs:596` 把新增 arguments 作为 `InputJsonDelta` 交给 turn loop。
- `src-tauri/src/modules/application/turn_service/stream_task.rs:872` 收集 `InputJsonDelta`，`src-tauri/src/modules/application/turn_service/stream_task.rs:1281` 之后执行工具。
- 当前 run-log 显示 `file_write` 的完整入参就是 `{}`，因此至少在执行器边界可以确认问题不是文件系统写入失败，而是 tool-call 参数生成/解析/自修复链路失败。
- `src-tauri/src/modules/application/turn_service/stream_task.rs:368` 只在 outer loop 顶部检查 `cancel_rx.try_recv()`；当前代码没有在 `stream.next_event().await` 等待 provider 事件时 `select!` 取消信号。
- `src-tauri/src/modules/application/stream_cancel_service.rs:42` 只发送 oneshot，不会 abort provider HTTP/SSE 请求或后台 task。
- `src/api/streaming.ts:28` 的 `startAgentStream(sessionId, userMessage, permissionMode)` 没有把当前 composer 选择的 `providerId/modelId` 传给后端，`src/api/conversations.ts:103` 也只转发这三个字段。
- `src/components/ui/chat-ui.tsx:2451` 切换模型时只更新本地 `selectedModel` 并调用 `model_set_active`，但该调用不会覆盖 `role_models.chat`。
- `src-tauri/src/modules/config/model_resolver.rs:190` 的 `resolve_role_model("chat")` 优先使用显式 `role_models.chat`，只有没有 role assignment 时才 fallback 到 `active_model`。这解释了为什么 UI 和 `active_model` 已切到 Ollama，但实际 chat turn 仍可走 `kimi-k2.6`。

根因判断:
- 已确认的直接根因是 `file_write` 工具调用进入执行器时缺少 `path/content`，导致工具 handler 必然失败。
- 从证据看，Kimi `kimi-k2.6` 在工具失败自修复后仍持续输出空 `file_write`，turn loop 没有对“同一工具同一空参数非重试错误”做早停或强制最终总结。
- 当前证据中的空参数样本均来自 `kimi-k2.6`；不能据此证明 Ollama 或其他 OpenAI-compatible 模型不会发生，但这类问题理论上属于“provider tool-call argument streaming/模型工具调用质量 + app 缺少 schema guard”的组合风险。
- 同一 session 的“Ollama”复测不能证明真实 Ollama provider 一定也会空参数，因为当前后端解析显示 `chat` 角色配置仍覆盖了 `active_model`；在修复模型路由前，所有从聊天 composer 发出的跨模型对比都可能被 stale role assignment 污染。
- 设置页把 `role_models.chat` 同步为 `ollama/minimax-m2.7:cloud` 后，真实 Ollama 云模型路径也复现了工具参数为空/不完整问题。因此该问题不是 Kimi-only；至少影响 `minimax-m2.7:cloud` 这类 Ollama OpenAI-compatible 云模型。
- 真实 Ollama 复现进一步显示，问题不局限于 `file_write`：`bash` 也会收到 `{}` 并缺少 `command`。更准确的根因描述是“模型/adapter 输出的 tool-call arguments 在部分轮次为空或字段不完整，而 turn loop 未在执行前按 schema required 做拦截与结构化自修复”。
- 这类失败会被当前 `max_iterations` 放大：每次空参数失败都继续进入下一次 LLM 请求，用户前端长时间停留在“写入中”，但不会得到“无法继续”的清晰反馈。
- `reasoning_content` placeholder 在本轮起到了避免 400 的作用；本次稳定性问题不是 `reasoning_content missing`，而是 invalid tool args + 缺少循环熔断。
- 停止延迟的直接根因是协作式取消粒度太粗：stop IPC 只投递 cancel signal，stream task 要等当前 `stream.next_event().await` 返回并进入下一次 outer loop 才处理取消。因此 provider 迟迟不返回 MessageStop/错误时，用户会感到“点了停止但没立刻停”。

建议:
- 在 turn loop 增加 invalid tool args guard：若工具 schema required 字段缺失，不要直接执行工具；记录 `dropped_invalid_tool_use_inputs_total` 或专门 `invalid_tool_args_total`，并把结构化错误反馈给模型。
- 对同一 `tool_name + normalized_args + error_code` 连续失败设置小阈值，例如 2 次后强制 finalization pass，禁止继续调用同一无效工具。
- finalization pass 要明确告诉用户“未创建文件”和原因，而不是只留下工具卡片失败。
- 对 OpenAI-compatible provider 增加 tool-call args 观测: debug 日志应记录工具名、参数长度、JSON 是否有效、required 字段是否齐全，避免只能从 run-log 反推。
- 对大文件生成任务增加策略: 优先建议 `shell`/heredoc 或分块写入，或让 `file_write` schema/description 明确给出最小示例 `{ "path": "super-mario.html", "content": "..." }`。
- 将取消接入 provider event loop：`tokio::select! { _ = &mut cancel_rx => ..., event = stream.next_event() => ... }`，并确保取消时关闭/丢弃底层 HTTP stream，立即 emit `stream_complete(cancelled_by_user)`。
- 修复聊天模型路由闭环：要么 `start_agent_stream` 显式接收 composer 的 `providerId/modelId` 并作为本 turn 的最高优先级，要么 `model_set_active` 同步更新 `role_models.chat`；同时后台 stream start 日志必须打印 resolved provider/model，不能只在 usage fallback 中间接暴露。
- 成功判定需要结合目标和工具结果：当用户要求创建文件且 mutating tool 失败、目标路径不存在时，不能以 `model_stop_no_tools` 标记 `completed`，应降级为 `partial_success` 或 `failed` 并输出明确总结。
- 增加回归: 使用 `kimi-k2.6` 发送单文件 HTML 生成任务，若 provider 连续输出空 `file_write`，系统应在 2 次内熔断、展示明确失败总结，并且后台出现 `task_outcome='failed'` 或 `degraded_reason='invalid_tool_args_repeated'`。
- 增加回归: composer 从 `moonshot/kimi-k2.6` 切到 `ollama/minimax-m2.7:cloud` 后，下一轮 `start_agent_stream` 的 backend resolved model 必须是 `ollama/minimax-m2.7:cloud`，且不得被 `role_models.chat` 静默覆盖。
- 增加回归: 使用 `ollama/minimax-m2.7:cloud` 发送同一 HTML 文件生成任务时，若工具调用参数缺 required 字段，应在执行前计入 `dropped_invalid_tool_use_inputs_total` 或 `invalid_tool_args_total`，不得把缺参工具当作真实执行失败反复喂回模型。

### P3: 前端 bundle 体积和动态导入警告需要进入性能 backlog

现象: `npx tauri build --debug` 时 Vite 提示主 chunk 超过 500KB，且 `src/lib/tauri.ts`、`src/modules/git/api.ts` 同时静态和动态导入，动态导入无法拆包。

建议:
- 将设置页、Git Workbench、记忆调试页、Skills Market 拆成 route-level chunks。
- 清理同一模块的 static/dynamic 混用。
- 建立 `pnpm build` bundle size budget，避免桌面 App 冷启动继续变慢。

## 下一步修复顺序

1. 修工具循环稳定性: invalid tool args 熔断、重复失败强制 finalization、取消后写入明确终止诊断。
2. 先修 TTS idle eviction nested runtime panic，避免后台稳定性继续被破坏。
3. 修 `ThinkingBlock` 默认展开和原文暴露：这是最直接影响用户信任的问题。
4. 修历史聊天恢复: 将 durable run-log replay 接回 runtime projection store，避免只显示用户消息。
5. 修 Persona effective registry / session override / prompt 注入三处一致性。
6. 修 Memory Browser 只读列表路径，避免浏览污染 `access_count` 和晋升推荐。
7. 修 Provider 详情回填：让服务商配置、模型配置、聊天 composer 三处形成闭环。
8. 加版本一致性 gate：避免之后再次出现“运行的是不是老版本”的排查成本。
9. 统一 packaged/dev storage root 与 UtilityLlm bootstrap：这属于系统架构风险，适合单独做一次启动路径审计。
10. 将 memory/vector 降级、usage pricing fallback、self_repair warning 转成可观测、可解释的状态，而不是只在日志里出现。

## 回归检查清单

- 启动 `/Applications/If2Ai.app` 后右下角版本与 `Info.plist` 一致。
- 设置页 Provider 左侧模型数量与右侧“已保存模型”列表一致。
- 新增 Provider 模型后，`models.json` 立即写入 `supportsThinking`。
- 设置主聊天模型后，聊天 composer 模型选择立即同步。
- 简单聊天回复完成后，run-log 有 `stream_complete`，后台有 `task_outcome='completed'`。
- thinking block 默认折叠且不展示原始推理全文。
- packaged App 与 `npx tauri dev` 使用同一个 config root，启动日志明确打印该 root。
- TTS idle eviction 触发后无 Tokio nested runtime panic，TTS 状态正确变为 Evicted。
- TTS idle eviction 后再次点击“播放语音”可 lazy reload 并完成合成。
- 打开历史会话后 assistant/tool/thinking/cost/status 均从 projection 恢复，不只显示用户消息。
- 切换 Persona 后 session JSON 立即写入 `persona_id`，下一轮 prompt 使用同一个 resolved persona。
- 自定义 Persona（如 `quitemood`）可被 turn_service 正确解析，不再出现 `persona_id 'quitemood' was not found`。
- 复杂文件生成任务中，若 `file_write` 连续收到空参数，2 次内熔断并输出明确失败总结，不继续空转。
- 工具调用取消后，run-log 和 UI 都显示 `cancelled`/`interrupted` 的终态，而不是停留在“任务进行中”。
- 打开 Memory Browser 不增加 agent recall 的 `access_count`。
- 候选晋升点击前必须出现确认/预览，不允许单击直接跨 scope 写入。
- “检查更新”未实现时不应展示可点击空按钮。

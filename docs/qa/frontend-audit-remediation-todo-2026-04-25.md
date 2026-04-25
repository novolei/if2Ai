# Frontend Computer Use Audit Remediation TODO

日期: 2026-04-25
来源: `docs/qa/frontend-computer-use-audit-2026-04-25.md`
目标: 按 P1 / P2 / P3 分批修复全部 findings，并用代码验证关闭。

## 状态标记

- `[ ] Todo`: 尚未实施。
- `[~] In Progress`: 已定位并开始改代码。
- `[x] Done`: 代码已改并通过对应最小验证。
- `[!] Blocked`: 需要外部条件或更大架构拆分，必须写明原因和替代保护。

## P1 TODO

- `[x]` TTS idle eviction nested Tokio runtime panic
  - 目标文件: `src-tauri/src/commands/host_composition.rs`, `src-tauri/src/modules/tts/manager/eviction.rs`
  - 修复口径: eviction 回调不得在 evictor runtime 内再次 `block_on`；idle eviction 后 provider slot 释放且状态可观测。
  - 验收: `cargo test --manifest-path src-tauri/Cargo.toml tts --lib` 或相关 TTS 单测通过；运行日志不再出现 `Cannot start a runtime from within a runtime`。

- `[x]` Thinking 原文默认展开并暴露内部推理
  - 目标文件: `src/components/ui/chat-ui.tsx`, `src/components/chat/ThinkingBlock.tsx`
  - 修复口径: 默认折叠；非调试可见摘要不直接显示 raw thinking 首行；展开入口有清晰 aria。
  - 验收: 简单聊天完成后 thinking block 不自动展开，摘要不泄露完整内部提示。

- `[x]` Provider 详情面板未回填已保存模型
  - 目标文件: `src/modules/settings/pages/ProvidersSettingsPage.tsx`, `src/modules/onboarding/hooks/useOnboarding.ts`
  - 修复口径: 进入 provider 详情时从 `provider_get_configured_models` 和保存配置回填 `availableModels`、`selectedModelIds`、thinking badge。
  - 验收: 左侧模型计数和右侧“已添加的模型”一致；无需点“读取模型”也能看到已保存模型。

- `[x]` Memory Browser 浏览污染 `access_count`
  - 目标文件: `src/components/memory/MemoryBrowser.tsx`, `src/api/memory.ts`
  - 修复口径: 默认列表和清除搜索走 `memory_export` 只读路径；只有显式搜索才可用 recall 检索。
  - 验收: 打开/清除 Memory Browser 不增加 entry `access_count`。

- `[x]` 打包 App 启动路径双轨初始化
  - 目标文件: `src-tauri/src/bootstrap/*`, `src-tauri/src/modules/config/*`
  - 修复口径: 启动日志统一输出 `build_profile`、`bundle_version`、`config_root`、`app_data_dir`、`argv0`；避免 `.if2ai` 和 `Application Support/.if2ai` 双写。
  - 验收: packaged/dev/direct backend 启动日志能解释并统一 root；不再产生未知 MockUtilityLlm 路径。

- `[x]` 历史聊天恢复只显示用户消息
  - 目标文件: `src/App.tsx`, `src/runtime-projection/history-replay.ts`, `src/api/sessions.ts`
  - 修复口径: `handleSelectSession()` 优先读取 `getSessionHistoryPage()` 并 replay durable run-log；event-log 为空时 fallback 到 session JSON。
  - 验收: 重开历史会话后 user/assistant/tool/thinking/status/cost 都稳定显示。

- `[x]` Persona 快捷切换未成为本轮有效身份
  - 目标文件: `src-tauri/src/modules/application/turn_service/mod.rs`, `src-tauri/src/modules/identity/*`, `src/modules/chat/components/SidebarTop.tsx`
  - 修复口径: turn_service 使用合并 custom identity pack 的 effective registry；前端 toast 以后端返回 session meta 为准。
  - 验收: 切换 Persona 后 session JSON 写入 persona_id，下一轮 prompt diagnostics 和模型自述一致。

- `[x]` 工具空参数执行循环、停止延迟、模型路由断裂
  - 目标文件: `src-tauri/src/modules/application/turn_service/stream_task.rs`, `src-tauri/src/modules/tools/registry.rs`, `src-tauri/src/modules/runtime/stream_outcome.rs`, `src-tauri/src/modules/config/model_resolver.rs`, `src/api/streaming.ts`, `src/api/conversations.ts`, `src/App.tsx`, `src/lib/tauri.ts`
  - 已实施: schema required guard、invalid args 熔断、重复工具批次熔断、cancel select、composer selected model 透传、chat role 与 active model 同步。
  - 已验证: `cargo check --manifest-path src-tauri/Cargo.toml`; `cargo test --manifest-path src-tauri/Cargo.toml runtime::stream_outcome --lib`; `pnpm test -- --run src/api/client.test.ts src/api/conversations.test.ts`。

## P2 TODO

- `[x]` App 版本号三源不一致
  - 目标文件: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, 可选 `scripts/check-version-consistency.*`
  - 修复口径: 三源同步到 `0.4.0`；增加检查脚本或测试防漂移。
  - 验收: 前端水印/About/bundle Info.plist 一致。

- `[x]` Vector memory packaged 初始化降级不可观测
  - 目标文件: `src-tauri/src/bootstrap/memory.rs`, `src-tauri/src/modules/memory/providers/vector_provider.rs`, `src/modules/settings/pages/MemorySettingsPage.tsx`
  - 修复口径: 日志输出 embedding model resolved path 和存在性；UI 展示 Vector/SQLite fallback 状态。
  - 验收: 初始化失败时用户能看到降级原因，不只埋在 backend log。

- `[x]` `self_repair` watchdog 无 runtime warning 噪声
  - 目标文件: `src-tauri/src/modules/runtime/self_repair.rs`, `src-tauri/src/modules/scheduler*`
  - 修复口径: spawn 收敛到受控 runtime 或将 fallback 降为 debug 并保留一次性诊断。
  - 验收: 正常启动不反复刷 warning。

- `[x]` 记忆候选晋升缺少确认/预览
  - 目标文件: `src/components/memory/MemoryBrowser.tsx`
  - 修复口径: “应用”先弹确认，展示 key、源 scope、目标 scope、影响范围和可回滚说明。
  - 验收: 单击候选不会直接跨 scope 写入。

- `[x]` 重复/同义记忆会被晋升放大
  - 目标文件: `src/components/memory/MemoryBrowser.tsx`, 可选 `src-tauri/src/modules/memory/promotion.rs`
  - 修复口径: 前端先展示 duplicate cluster/相似 key 警示；低 trust 条目不直接给快速晋升。
  - 验收: 同义 key 候选会提示先合并/复核。

- `[x]` Usage 未知模型价格按 Sonnet 默认档误导
  - 目标文件: `src-tauri/src/modules/runtime/usage.rs`, `src/modules/settings/pages/UsageSettingsPage.tsx`, `src/components/chat/TurnCostChip.tsx`, `src/components/chat/ContextBar.tsx`
  - 修复口径: 未知价格标记 estimated/unknown；UI 显示“估算”而非高置信美元成本。
  - 验收: Kimi/Ollama unknown pricing 不再显示成确定成本。

- `[x]` Persona 与 TTS Profile 视觉语义混淆
  - 目标文件: `src/modules/chat/components/SidebarTop.tsx`, `src/components/ui/chat-ui.tsx`, TTS profile pill 所在组件
  - 修复口径: TTS pill 显示“语音: ...”；Persona badge 增加显式 label/tooltip。
  - 验收: 顶部两个入口一眼可区分“身份”和“语音”。

## P3 TODO

- `[x]` 模型角色下拉包含空 Provider 分组
  - 目标文件: `src/modules/settings/pages/ModelSettingsPage.tsx`
  - 修复口径: 默认隐藏 `models.length === 0` 的 provider group；空生态入口留给 Provider 添加页。
  - 验收: 角色下拉只展示有可选模型的分组。

- `[x]` 记忆搜索空态不显示 `0 / total` 和过滤条件
  - 目标文件: `src/components/memory/MemoryBrowser.tsx`
  - 修复口径: 搜索后展示 `0 / N 条匹配`，空态包含 query/category/scope。
  - 验收: 搜索无结果时能判断是过滤无结果，不是全库为空。

- `[x]` Prompt Diagnostics 视觉选中与可访问焦点不同步
  - 目标文件: `src/modules/settings/components/SettingsSidebarItem.tsx`, `src/modules/settings/pages/PromptDiagnosticsPage.tsx`
  - 修复口径: nav item 设置 `aria-current="page"`；切页后 focus 页面标题或当前 nav item。
  - 验收: accessibility 当前页和视觉 active 一致。

- `[x]` 全局搜索只覆盖线程/项目
  - 目标文件: `src/components/GlobalSearch.tsx`
  - 修复口径: 增加设置项/命令分组；高风险命令只跳转，不直接执行。
  - 验收: 搜 `provider` / `memory` / `prompt` 能打开对应设置入口。

- `[x]` “检查更新” dead CTA
  - 目标文件: `src/modules/app-shell/components/GlobalNavbar.tsx`, `src/modules/settings/pages/AboutSettingsPage.tsx`
  - 修复口径: 未接入 updater 前按钮显示 toast/disabled 状态，不允许空点击。
  - 验收: 点击有明确反馈。

- `[x]` 字母排序让当前活跃项目离开顶部
  - 目标文件: `src/components/ProjectRail.tsx`
  - 修复口径: 排序后 active project 固定在顶部或当前项目区。
  - 验收: 切换字母排序后 active project 仍可见。

- `[x]` 中文 UI 混入英文主标题
  - 目标文件: `src/modules/chat/components/HomeScreen.tsx`
  - 修复口径: 中文界面标题改为“今天想在 <project> 里完成什么？”。
  - 验收: 首页主标题语言一致。

- `[x]` bundle size / dynamic import warnings
  - 目标文件: `src/App.tsx`, `src/modules/settings/SettingsApp.tsx`, `src/lib/tauri.ts`, `src/modules/git/api.ts`
  - 修复口径: route-level lazy loading；清理同一模块 static/dynamic 混用；增加 bundle budget。
  - 验收: `pnpm build:web` 不再出现同类混用警告，主 chunk 明显下降或有预算门禁。

## 执行顺序

1. 完成所有 P1，并跑 Rust/前端最小回归。
2. 完成所有 P2，重点是用户信任和高影响操作确认。
3. 完成所有 P3，重点是 UX、a11y、性能和文案一致性。
4. 更新本 TODO 的状态，并在 audit 文档后补充 remediation summary。

## 全局验证命令

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml runtime::stream_outcome --lib
pnpm test -- --run src/api/client.test.ts src/api/conversations.test.ts
pnpm build:web
```

## Remediation Log

- 2026-04-25: 修复 TTS idle eviction nested runtime panic，将 eviction 状态更新改为非阻塞 `try_write`，避免在 evictor runtime 内再次 `block_on`。
- 2026-04-25: 修复 Thinking 默认展开与摘要泄露，primary thinking 默认折叠，摘要改为安全的上下文判断计数。
- 2026-04-25: 修复 Provider 详情页已保存模型回填，切换 provider 时读取 `provider_get_configured_models` 并同步 selected models。
- 2026-04-25: 修复 Memory Browser 浏览污染 `access_count`，默认列表、清除搜索、搜索均改用只读 `memory_export` 路径。
- 2026-04-25: 修复历史聊天恢复主路径，加载 session 时优先尝试 durable run-log replay，并在 projection run 不存在时允许 legacy hydrated assistant/tool 回显。
- 2026-04-25: 修复 turn_service Persona registry，只使用 built-in registry 的路径改为合并 `identity-pack.json` 的 effective registry。
- 2026-04-25: 增加启动 root 诊断日志，输出 build profile、bundle version、config root、memory root、app data dir、argv0。
- 2026-04-25: 同步版本到 `0.4.0`，新增 `scripts/check-version-consistency.mjs` 和 `npm run check:version`。
- 2026-04-25: 增加 Vector memory / FastEmbed 初始化路径与状态日志，降低 packaged fallback 排查成本。
- 2026-04-25: 将 self_repair watchdog 无 runtime fallback 从 warning 降为 debug，避免正常 bootstrap 噪声淹没真实问题。
- 2026-04-25: 记忆候选晋升增加确认；重复 key 候选显示复核/合并提示。
- 2026-04-25: Usage / TurnCost / ContextBar 成本展示改为“约”估算，并补充未知模型价格说明。
- 2026-04-25: TTS Profile pill 改为“语音: ...”，降低与 Persona 的语义混淆。
- 2026-04-25: 模型角色下拉隐藏空 provider 分组。
- 2026-04-25: 记忆搜索空态显示 query 和 `0 / total` 匹配语义。
- 2026-04-25: Settings sidebar 增加 `aria-current="page"`，切页后 focus 页面标题。
- 2026-04-25: 全局搜索增加设置/命令分组，匹配 provider/model/memory/prompt 等入口。
- 2026-04-25: “检查更新”按钮增加可见 toast，不再是空点击。
- 2026-04-25: ProjectRail 排序后保持当前项目吸顶。
- 2026-04-25: HomeScreen 主标题改为中文。
- 2026-04-25: Vite 增加手动 chunk，将 settings/git/memory/vendor 拆分，降低主 chunk 压力。

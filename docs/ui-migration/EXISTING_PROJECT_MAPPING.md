# 现有项目映射表

> 扫描时间: 2026-04-15
> 前端源码位置: `src/`
> 技术栈: Tauri 2 + React 19 + TypeScript + Tailwind CSS v4 + shadcn/ui + Radix UI

---

## 0. 技术栈一致性检查

| 检查项 | 当前项目 | Paico 产物 | 结论 | 处理策略 |
|---|---|---|---|---|
| 前端框架 | React 19.2.5 | React 19.2.0 | ✅ 一致（Patch 差异可忽略） | 直接复用组件，无需适配 |
| TypeScript | 5.0+ (vite devDeps) | ~5.9.3 | ⚠️ Paico 版本更新，但 API 向下兼容 | 保留现有 TS 版本，Paico 组件编译后无兼容风险 |
| 样式方案 | Tailwind CSS v4.1 + `@tailwindcss/vite` + `tw-animate-css` | Tailwind CSS v4.1.17 + `@tailwindcss/vite` + `tw-animate-css` | ✅ 一致（同为 v4 + Vite 插件模式） | CSS 变量和 utility class 可直接共享 |
| 组件库 | shadcn/ui v4.2.0 + Radix UI（New York 风格） | shadcn/ui（New York 风格）+ 完整 Radix 集合 | ✅ 一致（同风格同体系） | `components/ui/` 两边可互操作 |
| 图标库 | lucide-react ^1.8.0 | lucide-react ^0.555.0 | ⚠️ 版本跨度大，但 API 稳定 | 直接复用，无需适配；注意 Paico 中个别 icon 名可能在新版改名 |
| 状态管理 | 无（React useState/useEffect + localStorage 直写） | 无（React useState 直写） | ✅ 一致 | 两边都无全局状态库，组件间 props 传递模式一致 |
| 路由 | 无（Tauri 原生窗口，App.tsx 手动 section 切换） | react-router-dom v7（BrowserRouter + Routes） | ❌ 不同 | Paico 页面级路由组件**不可**直接复用，需剥离 Router 依赖后参考展示层 |
| 动画库 | 纯 CSS `@keyframes`（loading-caret / loading-glitch / shimmer / fadeInUp） | 纯 CSS 动画（`tw-animate-css`），无 Framer Motion | ✅ 一致 | CSS 动画可直接复用；现有 If2Ai 动画保留 |
| 目录结构 | `src/components/ui/` + `src/modules/*/components/` + `src/modules/*/pages/` + `src/components/ds/`（新增） | `src/components/ui/` + `src/components/ds/` + `src/components/dashboard/` + `src/pages/` | ⚠️ 组织逻辑相似但有差异 | Paico `components/ds/` 已复制到现有项目；Paico `pages/` 和 `dashboard/` 禁止直接接入 |

### 技术栈一致性总结

**高度一致（5/9）**：React 19、Tailwind v4、shadcn/ui New York、状态管理策略、动画方案均一致。这是**非常有利的迁移基础** — Paico 的组件代码可以几乎零适配地嵌入现有项目。

**轻微差异（2/9）**：TypeScript 版本和 Lucide 版本存在小版本差异，不影响组件级复用，但大规模迁移时需留意 breaking changes。

**根本差异（2/9）**：
- **路由**：Paico 使用 react-router，当前项目是 Tauri 原生窗口架构。所有 Paico `pages/` 都依赖 Router，禁止直接接入。
- **目录结构**：Paico 是独立 Web 应用的组织方式，当前项目是 Tauri 桌面应用的模块化架构。需要按组件粒度逐个提取，而非整体目录拷贝。

---

## 1. 基础组件映射

将"现有项目组件"与"Paico 对应组件"进行映射

| 现有组件名 | 现有文件路径 | Paico 对应组件 | Paico 文件路径 | 建议迁移方式 | 风险等级 |
|---|---|---|---|---|---|
| Button | `src/components/ui/button.tsx` | ds/Button | `docs/references/Paico UI/src/components/ds/Button.tsx` | 保留现有 API（children/cva），仅参考 Paico 的 primary/secondary/ghost 视觉风格做内部样式微调 | 🟡 中 |
| Input | `src/components/ui/input.tsx` | InputComposer（含 input 元素） | `docs/references/Paico UI/src/components/ds/InputComposer.tsx` | 保留现有 shadcn Input API；InputComposer 的 focus ring / shadow 样式可参考 | 🟢 低 |
| Badge | `src/components/ui/badge.tsx` | StatusTag | `docs/references/Paico UI/src/components/ds/StatusTag.tsx` | 已完成：Wave 2 新增 6 个 status-* 变体，保留 Badge 原有 API | 🟢 低 |
| Card | `src/components/ui/card.tsx` | （无直接对应） | — | 保留，Paico 无等价卡片组件 | 🟢 低 |
| Dialog | `src/components/ui/dialog.tsx` | （无直接对应） | — | 保留 Radix Dialog，Paico 页面无等价组件 | 🟢 低 |
| Tabs | `src/components/ui/tabs.tsx` | （无直接对应） | — | 保留 Radix Tabs | 🟢 低 |
| Switch | `src/components/ui/switch.tsx` | （无直接对应） | — | 保留 Radix Switch | 🟢 低 |
| Select | `src/components/ui/select.tsx` | （无直接对应） | — | 保留 Radix Select | 🟢 低 |
| Tooltip | `src/components/ui/tooltip.tsx` | （无直接对应） | — | 保留 Radix Tooltip | 🟢 低 |
| DropdownMenu | `src/components/ui/dropdown-menu.tsx` | （无直接对应） | — | 保留 Radix DropdownMenu | 🟢 低 |
| Separator | `src/components/ui/separator.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| Avatar | `src/components/ui/avatar.tsx` | （无直接对应） | — | 保留 Radix Avatar | 🟢 低 |
| ScrollArea | `src/components/ui/scroll-area.tsx` | （无直接对应） | — | 保留，但 Paico 的 `.scrollbar-thin` 可作为替代参考 | 🟢 低 |
| Textarea | `src/components/ui/textarea.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| Label | `src/components/ui/label.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| Collapsible | `src/components/ui/collapsible.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| ErrorBoundary | `src/components/ui/error-boundary.tsx` | （无对应） | — | 保留，If2Ai 专用组件 | 🟢 低 |
| cn() 工具 | `src/lib/utils.ts` | cn() 工具 | `docs/references/Paico UI/src/lib/utils.ts` | 功能一致，保留现有 | 🟢 低 |
| Design Token | `src/styles/globals.css` | index.css | `docs/references/Paico UI/src/index.css` | **已完成**：Wave 1 已合并 Paico Token 体系到 globals.css | 🟢 低 |

---

## 2. 业务壳组件映射

| 现有组件名 | 现有文件路径 | Paico 对应组件 | Paico 文件路径 | 建议迁移方式 | 风险等级 |
|---|---|---|---|---|---|
| GlobalNavbar | `src/modules/app-shell/components/GlobalNavbar.tsx` | Sidebar | `docs/references/Paico UI/src/components/ds/Sidebar.tsx` | 仅参考视觉风格（bg-sidebar、sidebar-accent 等 Token），**禁止替换组件结构** | 🟠 中高 |
| NavTooltipButton | `src/modules/app-shell/components/NavTooltipButton.tsx` | SidebarItem | `docs/references/Paico UI/src/components/ds/SidebarItem.tsx` | 参考 SidebarItem 的 hover/selected 态样式，保留现有的 Tooltip 逻辑 | 🟡 中 |
| SidebarTop | `src/modules/chat/components/SidebarTop.tsx` | （无直接对应） | — | 保留，Paico 侧边栏原型不含会话管理逻辑 | 🟢 低 |
| ProjectRail | `src/components/ProjectRail.tsx` | （无直接对应） | — | 保留，含真实 session/project 数据流，Paico 无等价组件 | 🟠 中高 |
| ChatUI | `src/components/ui/chat-ui.tsx` | ChatTimeline + MessageItem + InputComposer | `docs/references/Paico UI/src/components/ds/ChatTimeline.tsx` + `MessageItem.tsx` + `InputComposer.tsx` | **参考用**：Paico ds 组件提供展示层参考，但 ChatUI 含完整 Tauri 调用、Markdown 渲染、工具调用展示、权限决策展示等，禁止直接替换 | 🔴 高 |
| ChatWorkspace | `src/modules/chat/components/ChatWorkspace.tsx` | ChatWorkspace（页面级） | `docs/references/Paico UI/src/pages/ChatWorkspace.tsx` | 仅参考三面板布局（Sidebar/Chat/Inspector）和拖拽分割线视觉，**禁止替换组件结构** | 🔴 高 |
| AgentOrb | `src/components/AgentOrb.tsx` | （无对应） | — | 保留，If2Ai 专用加载指示器 | 🟢 低 |
| SessionStatus | `src/components/SessionStatus.tsx` | StatusTag | `docs/references/Paico UI/src/components/ds/StatusTag.tsx` | 可参考 StatusTag 的 6 状态色样式，保留现有 API | 🟡 中 |
| CreateProjectDialog | `src/components/CreateProjectDialog.tsx` | （无对应） | — | 保留，含 `@tauri-apps/plugin-dialog` 调用 | 🟠 中高 |
| WelcomeScreen | `src/components/WelcomeScreen.tsx` | （无对应） | — | 保留，含 Tauri 类型引用 | 🟠 中高 |
| SettingsShell | `src/modules/settings/components/SettingsShell.tsx` | （无对应） | — | 保留 | 🟢 低 |
| SettingsSidebar | `src/modules/settings/components/SettingsSidebar.tsx` | Sidebar | `docs/references/Paico UI/src/components/ds/Sidebar.tsx` | 参考 sidebar Token 样式，保留现有设置导航逻辑 | 🟡 中 |
| SettingsSidebarItem | `src/modules/settings/components/SettingsSidebarItem.tsx` | SidebarItem | `docs/references/Paico UI/src/components/ds/SidebarItem.tsx` | 参考 hover/selected 态样式，保留现有 Button + 路由逻辑 | 🟡 中 |
| SettingsSurface | `src/modules/settings/components/SettingsSurface.tsx` | Card | `docs/references/Paico UI/src/components/ui/card.tsx` | 参考 Card 样式，保留现有 API | 🟢 低 |
| SettingsRow | `src/modules/settings/components/SettingsRow.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| SettingsToggleRow | `src/modules/settings/components/SettingsToggleRow.tsx` | （无直接对应） | — | 保留 | 🟢 低 |
| SettingsMetricCard | `src/modules/settings/components/SettingsMetricCard.tsx` | StatCards | `docs/references/Paico UI/src/components/dashboard/StatCards.tsx` | 参考 StatCards 的视觉风格，但 StatCards 含 mock 数据，仅参考样式 | 🟡 中 |
| ArtifactEditor | `src/components/ui/ArtifactEditor.tsx` | （无对应） | — | 保留，CodeMirror 集成，If2Ai 专用 | 🟢 低 |
| TodoPanel | `src/components/ui/TodoPanel.tsx` | （无对应） | — | 保留，If2Ai 专用 | 🟢 低 |
| ProjectPreviewPanel | `src/components/ui/ProjectPreviewPanel.tsx` | InspectorPanel | `docs/references/Paico UI/src/components/ds/InspectorPanel.tsx` | 参考 InspectorPanel 的暖沙背景 + 文件树展示，保留现有 Tauri 类型引用 | 🟡 中 |
| SectionWorkspace | `src/modules/app-shell/components/SectionWorkspace.tsx` | （无对应） | — | 保留，路由段占位组件 | 🟢 低 |

---

## 3. 高耦合组件识别

| 组件名 | 文件路径 | 耦合类型 | 当前职责 | 迁移建议 | 风险等级 |
|---|---|---|---|---|---|
| App | `src/App.tsx` | invoke + listen + store | 主应用：agent 流、session 管理、project 管理、权限弹窗、自动恢复 | **禁止直接替换**，仅允许内部视觉重构（颜色/间距通过 Token 自动生效） | 🔴 高 |
| ChatUI | `src/components/ui/chat-ui.tsx` | Tauri 类型 + Message 渲染 | 完整聊天 UI：Markdown 渲染、工具调用卡片、权限决策、文件预览、Todo 面板 | **禁止直接替换**，可逐步参考 Paico ds/MessageItem 做局部展示层优化 | 🔴 高 |
| SettingsApp | `src/modules/settings/SettingsApp.tsx` | invoke | 设置窗口：skill 列表/启用/评审 | 保留逻辑，仅参考 Paico Card 样式做 Surface 视觉刷新 | 🟠 中高 |
| WebSearchSettingsPage | `src/modules/settings/pages/WebSearchSettingsPage.tsx` | invoke (5个API) | Web 搜索配置 | 保留逻辑，仅参考视觉 | 🟡 中 |
| SkillsSettingsPage | `src/modules/settings/pages/SkillsSettingsPage.tsx` | invoke (6个API) | 技能管理设置 | 保留逻辑，仅参考视觉 | 🟡 中 |
| ProjectRail | `src/components/ProjectRail.tsx` | Tauri 类型 | 项目/会话列表渲染 | 保留逻辑，参考 Paico 文件树样式做视觉优化 | 🟡 中 |
| ChatWorkspace | `src/modules/chat/components/ChatWorkspace.tsx` | 传递 props 到 ChatUI | 聊天工作区壳：模型选择、字体/密度切换 | 保留逻辑，参考 Paico 三面板布局优化视觉 | 🟠 中高 |
| GlobalNavbar | `src/modules/app-shell/components/GlobalNavbar.tsx` | Button 交互 | 左侧导航栏 | 参考 Paico Sidebar Token 做视觉刷新 | 🟡 中 |
| WelcomeScreen | `src/components/WelcomeScreen.tsx` | Tauri 类型 | 欢迎页（无项目时展示） | 保留逻辑，仅参考视觉 | 🟡 中 |
| CreateProjectDialog | `src/components/CreateProjectDialog.tsx` | `@tauri-apps/plugin-dialog` | 创建项目对话框 | 保留逻辑，仅参考视觉 | 🟡 中 |
| SidebarTop | `src/modules/chat/components/SidebarTop.tsx` | Button + Dropdown | 会话列表头部 | 保留逻辑，仅参考视觉 | 🟡 中 |
| main.tsx | `src/main.tsx` | invoke (closeSettingsWindow) | 入口：窗口类型检测 | 保留，不迁移 | 🟢 低 |

---

## 4. 推荐的首批迁移试点

| 推荐顺序 | 组件名 | 文件路径 | 推荐原因 | 风险等级 |
|---|---|---|---|---|
| 1 | **Design Token** | `src/styles/globals.css` | 已完成 Wave 1：Paico Token 已合并。这是所有后续视觉迁移的基础，全局生效，零耦合 | 🟢 低 |
| 2 | **Badge** | `src/components/ui/badge.tsx` | 已完成 Wave 2：新增 6 个 status-* 变体。纯组件文件，无 Tauri 耦合，14 个消费方自动受益 | 🟢 低 |
| 3 | **StatusTag** | `src/components/ds/StatusTag.tsx` (新建) | 已完成 Wave 3：全新组件，无现有引用。可立即在 SessionStatus、Skill 状态等处使用 | 🟢 低 |
| 4 | **SettingsSurface** | `src/modules/settings/components/SettingsSurface.tsx` | 纯展示容器组件，无 Tauri 耦合。可通过 Paico Card/shadow Token 做视觉刷新 | 🟡 中 |
| 5 | **SessionStatus** | `src/components/SessionStatus.tsx` | 小型状态徽章组件，可直接使用新增的 Badge status-* 变体或 StatusTag 组件 | 🟡 中 |

---

## 5. 总结

### 最适合先开始迁移的 3 个文件/组件

| # | 组件 | 原因 |
|---|---|---|
| 1 | `src/styles/globals.css` | ✅ 已完成。Paico Token 合并后，所有使用 `bg-primary`、`text-muted-foreground`、`border-border` 等 Tailwind 类的组件自动获得新配色 |
| 2 | `src/components/ui/badge.tsx` | ✅ 已完成。新增 6 个 status 变体，所有使用 `<Badge>` 的地方可选新样式 |
| 3 | `src/components/ds/StatusTag.tsx` | ✅ 已完成。全新组件，可立即在业务中使用，零风险 |

### 暂时不要碰的 3 个高风险文件/组件

| # | 组件 | 禁止原因 |
|---|---|---|
| 1 | `src/App.tsx` | 全应用核心：agent 流、session 管理、Tauri invoke/listen、权限系统。任何结构性改动都会影响全局 |
| 2 | `src/components/ui/chat-ui.tsx` | 完整聊天渲染引擎：Markdown、工具调用、权限决策、文件预览。Paico 的 MessageItem 仅能参考展示层 |
| 3 | `docs/references/Paico UI/src/pages/*.tsx` | 全部含 mock 数据，无 Tauri 连接，是设计原型而非生产代码 |

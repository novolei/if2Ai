# Paico 文件分类清单

> 扫描时间: 2026-04-15
> Paico 源码位置: `docs/references/Paico UI/`
> 技术栈: React 19 + TypeScript + Vite + Tailwind CSS v4 + Radix UI + shadcn/ui (New York)
>
> **技术栈一致性结论**：当前项目与 Paico 在 React 19、Tailwind v4、shadcn/ui New York、Radix UI 上完全一致。
> 唯一差异是路由（Tauri 原生窗口 vs react-router）和目录组织方式。
> 详细对比见 [EXISTING_PROJECT_MAPPING.md §0 技术栈一致性检查](./EXISTING_PROJECT_MAPPING.md#0-技术栈一致性检查)。

---

## 1. 可优先复用（低风险）

| 文件路径 | 文件类型 | 用途 | 推荐处理方式 | 风险等级 |
|---|---|---|---|---|
| `src/index.css` | Design Token System | 完整 CSS 变量体系（颜色、字体、间距、阴影、状态色、sidebar、inspector） | **优先接入**：合并到现有 `src/styles/globals.css`，保留已有动画类 | 🟢 低 |
| `components.json` | shadcn 配置 | New York 风格、CSS 变量启用、Lucide 图标 | 参考配置，无需迁移 | 🟢 低 |
| `src/lib/utils.ts` | 工具函数 | `cn()` 合并类（clsx + tailwind-merge） | 与现有 `@/lib/utils.ts` 功能一致，保留现有的即可 | 🟢 低 |
| `src/hooks/use-mobile.ts` | Hook | `useIsMobile()` 响应式检测 | 按需复制，不影响现有逻辑 | 🟢 低 |

---

## 2. 可局部参考（中风险）

### 2.1 基础 UI 组件（`components/ui/`）

| 文件路径 | 文件类型 | 用途 | 推荐处理方式 | 风险等级 |
|---|---|---|---|---|
| `src/components/ui/button.tsx` | 按钮（Radix Slot + CVA） | 6 变体 × 4 尺寸 | 保留现有 API 和 cva 变体，视觉微调参考 Paico ds/Button.tsx 的 `primary/secondary/ghost` 风格 | 🟡 中 |
| `src/components/ui/input.tsx` | 输入框 | 标准 input 表单 | 保留 API，token 颜色已由 globals.css Wave 1 自动生效 | 🟡 中 |
| `src/components/ui/badge.tsx` | 徽章 | 4 变体 | Wave 2 已新增 Paico 状态色变体，无需额外改动 | 🟡 低 |
| `src/components/ui/card.tsx` | 卡片容器 | Card/Header/Content/Footer | 保留，CSS 变量自动刷新颜色 | 🟢 低 |
| `src/components/ui/dialog.tsx` | 对话框（Radix） | Modal 对话框 | 保留 Radix 基础，仅视觉微调 | 🟡 中 |
| `src/components/ui/tooltip.tsx` | 提示框（Radix） | 悬浮提示 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/tabs.tsx` | 标签页（Radix） | Tab 切换 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/switch.tsx` | 开关（Radix） | Toggle 开关 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/select.tsx` | 下拉选择（Radix） | Select 表单 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/dropdown-menu.tsx` | 下拉菜单（Radix） | 菜单系统 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/separator.tsx` | 分隔线（Radix） | 水平/垂直分割 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/label.tsx` | 表单标签 | 可访问性 label | 保留，无需改动 | 🟢 低 |
| `src/components/ui/avatar.tsx` | 头像（Radix） | 用户头像 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/scroll-area.tsx` | 滚动区域（Radix） | 自定义滚动条 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/collapsible.tsx` | 折叠区域 | 手风琴 | 保留，无需改动 | 🟢 低 |
| `src/components/ui/textarea.tsx` | 多行文本 | textarea | 保留，无需改动 | 🟢 低 |

### 2.2 Paico ds 设计系统组件

| 文件路径 | 文件类型 | 用途 | 推荐处理方式 | 风险等级 |
|---|---|---|---|---|
| `src/components/ds/Button.tsx` | 设计系统按钮 | 3 变体（primary/secondary/ghost）× 3 尺寸 | **参考用**：与现有 shadcn button API 不同，不直接替换 | 🟡 中 |
| `src/components/ds/StatusTag.tsx` | 状态徽章 | 6 状态 + dot 指示器 | **可复用**：独立组件，无业务耦合，可直接使用 | 🟢 低 |
| `src/components/ds/SidebarItem.tsx` | 导航项 | 图标 + 标签 + 徽章 + 选中态 | **参考用**：与现有 SettingsSidebarItem / GlobalNavbar 逻辑不同，需适配 | 🟡 中 |
| `src/components/ds/FileListItem.tsx` | 文件树节点 | 递归文件夹展开 + 扩展名图标 | **可复用**：独立展示组件，可在 InspectorPanel 或 ProjectRail 中参考 | 🟢 低 |
| `src/components/ds/MessageItem.tsx` | 消息气泡 | assistant 思考块 + user 右对齐气泡 | **参考用**：与 `chat-ui.tsx` 中已有消息渲染逻辑并行，需选择整合策略 | 🟡 中 |
| `src/components/ds/ChatTimeline.tsx` | 消息容器 | 可滚动消息列 + thinking 状态管理 | **参考用**：与现有 `ChatUI` 组件架构不同，需渐进替换 | 🟡 中 |
| `src/components/ds/InputComposer.tsx` | 聊天输入框 | 含 attach/mic/send + meta bar | **参考用**：与 `ChatUI` 内置输入框并行，需选择整合 | 🟡 中 |
| `src/components/ds/InspectorPanel.tsx` | 右侧面板 | 项目头 + 技能徽章 + 文件树 | **参考用**：展示层参考，不含真实业务逻辑 | 🟡 中 |
| `src/components/ds/Sidebar.tsx` | 完整侧边栏 | 220px 导航 + 用户卡片 | **仅参考**：含 mock 导航项，与现有 GlobalNavbar + ProjectRail 架构不同 | 🟠 中高 |

---

## 3. 高风险文件（禁止直接替换）

| 文件路径 | 文件类型 | 用途 | 禁止原因 | 风险等级 |
|---|---|---|---|---|
| `src/pages/Index.tsx` | 页面壳 | Paico 主入口页 | 含完整 React Router 路由结构，与现有 App.tsx 完全不同架构 | 🔴 高 |
| `src/pages/ChatWorkspace.tsx` | 页面壳 | 三面板聊天（Sidebar/Chat/Inspector） | 含 mock 数据、拖拽分割线、静态假逻辑，不含 Tauri 调用 | 🔴 高 |
| `src/pages/Dashboard.tsx` | 仪表盘页 | 8 个统计组件组合 | 所有子组件都含 mock 数据，无真实业务 | 🔴 高 |
| `src/pages/TokenSystem.tsx` | Token 展示页 | 设计 Token 文档页 | 纯展示，与现有项目无关 | 🔴 高 |
| `src/pages/ComponentSystem.tsx` | 组件文档页 | 设计系统文档页 | 纯展示，含 SAMPLE 数据 | 🔴 高 |
| `src/pages/Moodboards.tsx` | 情绪板页 | 图片画廊 | 含 6 条 mock 数据 + 静态交互 | 🔴 高 |
| `src/pages/Materials.tsx` | 材料库页 | 材料列表 | 含 9 条 mock 数据 + 静态筛选 | 🔴 高 |
| `src/pages/Proposals.tsx` | 提案页 | 提案构建器 | 含 3 条 mock 数据 + 静态评论区 | 🔴 高 |
| `src/pages/Timeline.tsx` | 时间轴页 | Gantt 图 | 含 5 条 mock 项目数据 + 静态计算 | 🔴 高 |
| `src/pages/Clients.tsx` | 客户管理页 | 客户列表 + 详情 | 含 6 条 mock 数据 + 静态筛选 | 🔴 高 |
| `src/pages/Files.tsx` | 文件库页 | 文件列表 | 含 12 条 mock 数据 + 静态视图切换 | 🔴 高 |
| `src/pages/Projects.tsx` | 项目管理页 | 项目列表 | 含 6 条 mock 数据 + 静态筛选 | 🔴 高 |
| `src/pages/NotFound.tsx` | 404 页 | 未找到页面 | 与现有路由系统无关 | 🔴 高 |
| `src/components/Sidebar.tsx` | 侧边栏布局 | 导航 + Studio 信息 | 含 hardcoded 导航项 + Studio 名称，与现有 GlobalNavbar 架构冲突 | 🔴 高 |
| `src/components/Topbar.tsx` | 顶部栏 | 搜索 + 设置 + 帮助 | 含 hardcoded 按钮，与现有 App.tsx 布局冲突 | 🔴 高 |
| `src/components/dashboard/StatCards.tsx` | 统计卡片 | 4 项指标 | 含 4 条 mock 数据 | 🔴 高 |
| `src/components/dashboard/ProjectOverview.tsx` | 项目列表 | 项目表格 | 含 4 条 mock 项目数据 | 🔴 高 |
| `src/components/dashboard/RecentActivity.tsx` | 活动流 | 最近活动 | 含 6 条 mock 数据 | 🔴 高 |
| `src/components/dashboard/MilestonesPanel.tsx` | 里程碑面板 | 时间线 | 含 6 条 mock 数据 | 🔴 高 |
| `src/components/dashboard/TeamWorkload.tsx` | 团队负载 | 容量可视化 | 含 5 条 mock 数据 + Recharts | 🔴 高 |
| `src/components/dashboard/UpcomingReviews.tsx` | 待审列表 | 即将评审 | 含 3 条 mock 数据 | 🔴 高 |
| `src/components/dashboard/BudgetStatus.tsx` | 预算图表 | 柱状图 | 含 6 月 mock 数据 + Recharts | 🔴 高 |
| `src/components/dashboard/ApprovalsQueue.tsx` | 审批队列 | 待审批 | 含 3 条 mock 数据 | 🔴 高 |

---

## 4. 初步结论

### 最适合优先迁移的文件
1. **`src/index.css`** — Paico 设计 Token 体系，合并到 `src/styles/globals.css` 即可全局生效，零业务耦合
2. **`src/components/ds/StatusTag.tsx`** — 纯展示组件，6 种状态色，无外部依赖，可直接在业务中使用
3. **`src/components/ds/FileListItem.tsx`** — 文件树节点展示，可复用于 InspectorPanel 或文件浏览场景

### 只能参考不能直接替换的文件
- **`src/components/ds/Button.tsx`** — API 与现有 shadcn button 不同（`label` prop vs children），现有 button 已被 14+ 业务组件引用
- **`src/components/ds/MessageItem.tsx`** — 消息气泡展示层，但现有 `chat-ui.tsx` 已包含完整的 Markdown 渲染、工具调用展示、Tauri 类型等
- **`src/components/ds/InputComposer.tsx`** — 输入框 UI，但缺少与 `startAgentStream` 的真实连接
- **`src/components/ds/Sidebar.tsx`** — 导航 mock，与现有 `GlobalNavbar` + `ProjectRail` 的双栏架构不同

### 明确禁止直接接入的文件
- **所有 `src/pages/*.tsx`** — 全部为 Paico 原型页，含 mock 数据，无 Tauri 连接
- **所有 `src/components/dashboard/*.tsx`** — 全部含硬编码 mock 数据
- **`src/components/Sidebar.tsx`** 和 **`src/components/Topbar.tsx`** — 与现有 App.tsx 布局系统冲突

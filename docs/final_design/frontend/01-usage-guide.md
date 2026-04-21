# 前端使用指南

> 面向用户/设计师：If2Ai 前端组件库、设计系统与交互规范

## 🎨 组件库使用

If2Ai 基于 **ShadCN + Radix UI** 构建 23 个基础组件，位于 `src/components/ui/`：

### 布局组件

| 组件 | 文件 | 说明 |
|------|------|------|
| Card | `card.tsx` | 通用卡片容器 |
| Separator | `separator.tsx` | 分割线 |
| ScrollArea | `scroll-area.tsx` | 自定义滚动区域 |
| Collapsible | `collapsible.tsx` | 可折叠区域 |
| Tabs | `tabs.tsx` | 标签页切换 |

### 表单组件

| 组件 | 文件 | 说明 |
|------|------|------|
| Button | `button.tsx` | 按钮（多种变体） |
| Input | `input.tsx` | 文本输入框 |
| Textarea | `textarea.tsx` | 多行文本输入 |
| Select | `select.tsx` | 下拉选择器 |
| Switch | `switch.tsx` | 开关切换 |
| Label | `label.tsx` | 表单标签 |

### 反馈组件

| 组件 | 文件 | 说明 |
|------|------|------|
| Dialog | `dialog.tsx` | 模态对话框 |
| DropdownMenu | `dropdown-menu.tsx` | 下拉菜单 |
| Tooltip | `tooltip.tsx` | 工具提示 |
| Badge | `badge.tsx` | 徽标 |
| Sonner | `sonner.tsx` | Toast 通知 |

### 导航组件

| 组件 | 文件 | 说明 |
|------|------|------|
| Command | `command.tsx` | 命令面板 |
| Avatar | `avatar.tsx` | 头像 |

### 业务组件

| 组件 | 文件 | 说明 |
|------|------|------|
| ChatUI | `chat-ui.tsx` | 聊天主界面（176KB） |
| ArtifactEditor | `ArtifactEditor.tsx` | 产物编辑器 |
| TodoPanel | `TodoPanel.tsx` | 待办面板 |
| ProjectPreviewPanel | `ProjectPreviewPanel.tsx` | 项目预览 |
| ErrorBoundary | `error-boundary.tsx` | 错误边界 |

### 无障碍设计

所有 ShadCN + Radix 组件内置无障碍支持：
- 键盘导航（Tab / Enter / Escape）
- ARIA 属性自动管理
- 屏幕阅读器兼容
- 焦点管理（Focus Trap）

## 🎨 Paico 设计系统

### 色彩基调：翡翠薄雾

If2Ai 采用 **翡翠薄雾（Jade Mist）** 色调作为视觉基调：

| 层级 | 色调 | CSS 变量前缀 |
|------|------|-------------|
| 主色 | 翡翠绿 | `--paico-green-*` |
| 中性色 | 灰阶系 | `--paico-neutral-*` |
| 语义色 | 红/橙/蓝 | `--paico-red/orange/blue-*` |
| 表面色 | 半透明雾感 | `--paico-surface-*` |

### 80+ CSS 令牌

定义于 `src/styles/globals.css`（26.9KB），包含：

```css
:root {
  /* 主色调 */
  --paico-green-50: #f0fdf4;
  --paico-green-500: #22c55e;
  --paico-green-900: #14532d;
  
  /* 表面层 */
  --paico-surface: rgba(255, 255, 255, 0.8);
  --paico-surface-elevated: rgba(255, 255, 255, 0.95);
  
  /* 文字 */
  --paico-text-primary: #1a1a2e;
  --paico-text-secondary: #6b7280;
  
  /* 间距 */
  --paico-radius-sm: 6px;
  --paico-radius-md: 8px;
  --paico-radius-lg: 12px;
}
```

## 🌗 主题切换

If2Ai 支持深色/浅色主题切换：

```css
:root { /* 浅色主题 */ }
.dark { /* 深色主题 */ }
```

切换方式：设置界面选择，或跟随系统偏好。

### 深色模式适配

所有组件和 CSS 令牌均适配深色模式：
- 背景色自动切换
- 文字对比度自动调整
- 阴影/边框自动适配
- 图标颜色自动反色

## ⌨️ 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Cmd+N` | 新建会话 |
| `Cmd+,` | 打开设置 |
| `Cmd+K` | 命令面板 |
| `Cmd+Shift+M` | 切换记忆面板 |
| `Cmd+Enter` | 发送消息 |
| `Cmd+.` | 停止生成 |
| `Escape` | 关闭弹窗/取消操作 |

## 🪟 多窗口导航

| 操作 | 方式 |
|------|------|
| 打开设置 | `Cmd+,` 或 IPC `open_settings_window` |
| 打开浏览器查看器 | IPC `open_browser_viewer_window` |
| 返回主窗口 | 点击 Dock 图标或系统托盘 |
| 聚焦并预填 | IPC `focus_main_window_and_prefill_prompt` |

## 🔗 相关资源

- [实现深度解析](./02-implementation.md)
- [运行时投影系统](./03-runtime-projection.md)
- [Paico 设计规范](../../references/if_2_ai_动态视觉系统规范.md)

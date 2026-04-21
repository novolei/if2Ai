# 📚 Browser 使用指南

> 导航、点击、输入、截屏 —— AI 驱动的浏览器自动化操作手册。

## 🌐 浏览器控制命令

### 导航操作

| 命令 | 说明 | 参数 |
|------|------|------|
| `navigate` | 打开 URL | `url: String` |
| `go_back` | 后退 | — |
| `go_forward` | 前进 | — |
| `refresh` | 刷新页面 | — |

导航结果 `NavigateResult` 包含当前页面状态信息。

### 页面交互

| 命令 | 说明 | 参数 |
|------|------|------|
| `click` | 点击元素 | `ref: usize`（AXTree 引用编号） |
| `type` | 输入文本 | `ref: usize`, `text: String` |
| `scroll` | 滚动页面 | `direction: ScrollDir (Up/Down)` |
| `select` | 选择下拉选项 | `ref: usize`, `value: String` |

所有交互通过 AXTree 快照中的引用编号（`data-if2ai-ref`）定位元素。

### 截屏

| 命令 | 说明 |
|------|------|
| `screenshot` | 截取当前页面截图（PNG/JPEG） |

截屏图片通过 base64 编码返回给 AI 代理，用于视觉理解。

### 标签页管理

| 命令 | 说明 |
|------|------|
| `list_tabs` | 列出所有打开的标签页 |
| `switch_tab` | 切换到指定标签页 |
| `close_tab` | 关闭指定标签页 |

```rust
// 源码参考：src-tauri/src/modules/browser/session.rs
pub struct TabInfo {
    pub idx: usize,          // 标签页位置索引
    pub url: String,         // 当前 URL
    pub title: String,       // 页面标题
    pub active: bool,        // 是否为活动标签页
    pub target_id: String,   // CDP target ID
}
```

标签页数量硬性上限 10 个（`MAX_TRACKED_TABS`），防止恶意页面无限弹窗。

## 🔍 DOM 快照查看

### AXTree 快照

AXTree 快照是 LLM 理解页面的核心机制：

1. 注入 JavaScript 脚本到页面
2. 遍历可见 DOM 元素
3. 标注可交互元素（按钮、链接、输入框等）为 `[N]` 引用编号
4. 生成人类可读的无障碍树文本

快照输出格式：
```
[1] link "首页"
[2] button "搜索"
[3] textbox "输入关键词..."
[4] button "提交"
```

### 可交互元素识别

快照脚本自动识别以下交互元素：

| 类型 | HTML 标签 / 属性 |
|------|-----------------|
| 链接 | `<a>` |
| 按钮 | `<button>` |
| 输入框 | `<input>`, `<textarea>` |
| 下拉框 | `<select>` |
| 折叠 | `<details>`, `<summary>` |
| ARIA 角色 | `role="button"`, `role="link"` 等 |
| 可点击 | `cursor: pointer` + `onclick` |
| 可编辑 | `contentEditable="true"` |

### 快照限制

- 最大树大小 30,000 字符（`MAX_TREE`）
- 同构兄弟元素自动压缩（如列表项）
- 隐藏元素（`display: none`, `visibility: hidden`）跳过
- `<script>`, `<style>`, `<svg>` 等非视觉元素跳过

源码参考：[`src-tauri/src/modules/browser/snapshot.rs`](../../../src-tauri/src/modules/browser/snapshot.rs)

## 🔄 会话管理

### 创建会话

每个聊天会话拥有独立的浏览器实例：

- 首次调用浏览器操作时自动创建
- 无头 Chromium 进程（headless）
- 独立的 Cookie / LocalStorage / Session

### 切换与关闭

| 操作 | 说明 |
|------|------|
| 自动创建 | 首次浏览器操作触发 |
| 会话隔离 | 不同聊天窗口各自独立浏览器 |
| 关闭会话 | 聊天结束时自动清理 |
| 冷状态恢复 | 重启应用后恢复上次 URL |

### 用户接管模式

当用户需要手动操作浏览器时，启用接管模式：

- 所有 AI 工具调用收到 "paused" 错误
- 避免人与 AI 同时操作冲突
- 用户释放后恢复 AI 控制

```rust
// 源码参考：src-tauri/src/modules/browser/registry.rs
// takeover_flags: DashMap<String, bool>
// AI 工具调用前检查 → 如被接管则返回错误
```

## 🖥️ 浏览器查看器窗口

### 前端集成

浏览器状态通过 Tauri 事件推送至前端：

```rust
// 源码参考：src-tauri/src/modules/browser/events.rs
pub struct BrowserStatusEvent {
    pub session_id: String,
    pub running: bool,
    pub url: Option<String>,
}
```

前端 `BrowserViewerPage` 组件接收事件并渲染浏览器状态卡片。

### Chrome 二进制发现

`chrome_finder.rs` 自动查找系统 Chrome：

| 平台 | 搜索路径 |
|------|----------|
| macOS | `/Applications/Google Chrome.app/...` |
| Linux | `/usr/bin/google-chrome`, `/usr/bin/chromium` |
| Windows | `C:\Program Files\Google\Chrome\Application\chrome.exe` |

### Chrome 发现状态

```rust
// 源码参考：src-tauri/src/modules/browser/chrome_finder.rs
pub enum ChromeStatus {
    Found(PathBuf),      // 找到 Chrome 二进制
    NotFound,            // 未找到
    NotSupported,        // 当前平台不支持
}
```

### 浏览器配置模式

支持两种配置模式：

| 模式 | 说明 | 数据持久化 |
|------|------|-------------|
| Ephemeral（临时） | 每次启动创建新配置 | 应用退出即清除 |
| Persistent（持久化） | 使用固定配置目录 | Cookie/历史保留 |

持久化配置存储在 `~/.if2ai/browser-profiles/` 目录下。

源码参考：[`src-tauri/src/modules/browser/profile.rs`](../../../src-tauri/src/modules/browser/profile.rs)

## 📝 核心概念速查表

| 操作 | 命令 | 关键文件 |
|------|------|----------|
| 打开 URL | `navigate(url)` | `session.rs` |
| 点击元素 | `click(ref)` | `session.rs` |
| 输入文本 | `type(ref, text)` | `session.rs` |
| 截取屏幕 | `screenshot()` | `session.rs` |
| 获取快照 | `snapshot()` | `snapshot.rs` |
| 列出标签 | `list_tabs()` | `session.rs` |
| 查找 Chrome | `find_chrome_binary()` | `chrome_finder.rs` |
| 管理会话 | `BrowserRegistry` | `registry.rs` |

## 🔗 相关资源

- [Browser 实现文档](./02-implementation.md) — 开发者架构详解
- [BrowserSession 源码](../../../src-tauri/src/modules/browser/session.rs)
- [Snapshot 脚本源码](../../../src-tauri/src/modules/browser/snapshot.rs)

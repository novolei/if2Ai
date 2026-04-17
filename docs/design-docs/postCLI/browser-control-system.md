# Browser Control System — Design Doc

> Phase: 7B  
> Status: design  
> Author: Staff Architect  
> Created: 2026-04-17  
> Ref: ADR-014 (Browser Capability Gap vs openhanako)

---

## 1. 目标与范围

将 if2Ai 的浏览器能力从"静态 Web 访问"（`web_fetch` + `web_search`）提升至与 openhanako 同等的"可交互浏览器自动化"水平，让 AI Agent 能够：

- 导航至任意 URL 并读取页面结构
- 点击、输入文字、滚动、选择下拉项、按键
- 获取实时截图和 DOM AXTree 快照（带可操作 ref 编号）
- 执行任意页面 JavaScript
- 在后台运行，对用户展示实时缩略图
- 每个 chat session 拥有独立的 cookie/localStorage

---

## 2. 架构决策

### 2.1 核心引擎：chromiumoxide

选择 `chromiumoxide v0.9` 作为浏览器控制引擎，理由：

| 考量点       | chromiumoxide                 | 备选：Tauri WebviewWindow               |
| ------------ | ----------------------------- | --------------------------------------- |
| 跨平台一致性 | Chromium 引擎，行为一致       | WebKit (macOS) / WebView2 (Win)，差异大 |
| API 完整性   | 对标 Puppeteer，全部 CDP 操作 | eval_script() 有限制                    |
| Session 隔离 | incognito context 原生支持    | 需要 data_directory 绕过                |
| 截图质量     | 高质量 JPEG/PNG               | 系统截图 API 受限                       |
| 维护状态     | 活跃（v0.9.1, 2026-02）       | N/A                                     |

**运行模式：headless（默认）**，AI 在后台控制 Chromium，截图缩略图通过 Tauri Event 实时推送至前端 BrowserCard 展示。

### 2.2 进程架构

```
┌─────────────────────────────────────────┐
│           Tauri 主进程 (Rust)            │
│                                         │
│  ┌──────────────────────────────────┐   │
│  │     BrowserRegistry              │   │
│  │  ┌──────────────────────────┐    │   │
│  │  │  BrowserSession (session_id)  │   │
│  │  │   - chromiumoxide Browser │   │   │
│  │  │   - Page per tab          │   │   │
│  │  │   - incognito context     │   │   │
│  │  └──────────────────────────┘    │   │
│  └──────────────────────────────────┘   │
│                                         │
│  ┌──────────────────────────────────┐   │
│  │  browser Tool (ToolEntry)        │   │   ← LLM 通过工具调用
│  │  action: start/navigate/click... │   │
│  └──────────────────────────────────┘   │
│                                         │
│  Tauri Event Emitter                    │   ← 推送 browser-status
└─────────────────────────────────────────┘
              │ CDP WebSocket
              ▼
┌─────────────────────────────────────────┐
│  headless Chromium 进程                  │
│  (系统已安装 Chrome/Chromium)             │
└─────────────────────────────────────────┘

              │ Tauri Event
              ▼
┌─────────────────────────────────────────┐
│  前端 React (Svelte/TSX)                 │
│  ┌──────────────────────────────────┐   │
│  │  BrowserCard (浮动卡片)           │   │
│  │  - 截图缩略图                     │   │
│  │  - 当前 URL hostname              │   │
│  │  - 紧急停止按钮                   │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

---

## 3. 数据结构与接口

### 3.1 Rust 后端

#### 3.1.1 BrowserSession

```rust
// src-tauri/src/modules/browser/session.rs

/// 单个 chat session 对应的浏览器实例
pub struct BrowserSession {
    /// session 标识
    pub session_id: String,
    /// chromiumoxide Browser 实例（管理 Chromium 进程）
    browser: Browser,
    /// 后台任务句柄（chromiumoxide 要求必须 poll 的 handler）
    _handler: JoinHandle<()>,
    /// 活跃页面（每个 session 一个 tab）
    page: Page,
    /// 当前 URL（缓存，避免频繁 CDP 调用）
    pub current_url: Option<String>,
    /// 操作日志（供 AI 错误时回溯）
    action_log: Vec<ActionLogEntry>,
}

pub struct ActionLogEntry {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub params: serde_json::Value,
    pub result: String,  // "ok: <summary>" | "error: <msg>"
    pub url: Option<String>,
}
```

#### 3.1.2 BrowserRegistry

```rust
// src-tauri/src/modules/browser/registry.rs

/// 全局单例，通过 AppHandle state 注入
pub struct BrowserRegistry {
    /// session_id → BrowserSession
    sessions: DashMap<String, BrowserSession>,
    /// 冷保存路径
    cold_state_path: PathBuf,
}

impl BrowserRegistry {
    /// 为 session 启动浏览器（若已存在则返回现有实例）
    pub async fn launch(&self, session_id: &str) -> Result<(), BrowserError>;
    /// 关闭指定 session 的浏览器
    pub async fn close(&self, session_id: &str) -> Result<(), BrowserError>;
    /// 导航并返回 AXTree 快照
    pub async fn navigate(&self, session_id: &str, url: &str)
        -> Result<NavigateResult, BrowserError>;
    /// DOM AXTree 快照（注入 SNAPSHOT_SCRIPT）
    pub async fn snapshot(&self, session_id: &str) -> Result<String, BrowserError>;
    /// 截图，返回 base64 JPEG
    pub async fn screenshot(&self, session_id: &str) -> Result<String, BrowserError>;
    /// 缩略图（小尺寸截图，用于 BrowserCard）
    pub async fn thumbnail(&self, session_id: &str) -> Result<Option<String>, BrowserError>;
    /// 点击元素（by data-if2ai-ref 值）
    pub async fn click(&self, session_id: &str, ref_num: u32) -> Result<String, BrowserError>;
    /// 输入文字（可选指定 ref，否则聚焦元素）
    pub async fn type_text(&self, session_id: &str, text: &str, ref_num: Option<u32>, press_enter: bool)
        -> Result<String, BrowserError>;
    /// 滚动页面
    pub async fn scroll(&self, session_id: &str, direction: ScrollDir, amount: u32)
        -> Result<String, BrowserError>;
    /// 选择下拉项
    pub async fn select(&self, session_id: &str, ref_num: u32, value: &str)
        -> Result<String, BrowserError>;
    /// 按键
    pub async fn press_key(&self, session_id: &str, key: &str) -> Result<String, BrowserError>;
    /// 等待页面状态
    pub async fn wait(&self, session_id: &str, timeout_ms: u64, state: &str)
        -> Result<String, BrowserError>;
    /// 执行任意 JS，返回序列化结果
    pub async fn evaluate(&self, session_id: &str, expr: &str) -> Result<String, BrowserError>;
    /// 获取所有活跃 session 的浏览器状态
    pub fn get_all_status(&self) -> Vec<BrowserStatusEntry>;
    /// 冷保存：将 session URL 写入磁盘
    pub fn save_cold_state(&self) -> Result<(), BrowserError>;
    /// 冷恢复：从磁盘读取 URL，重新 launch + navigate
    pub async fn restore_cold_state(&self, session_id: &str) -> Result<bool, BrowserError>;
}
```

#### 3.1.3 Browser Tool Entry

```rust
// src-tauri/src/modules/tools/builtin/browser.rs

/// tool name: "browser"
/// toolset: "web"
/// actions: start | stop | navigate | snapshot | screenshot | click |
///          type | scroll | select | key | wait | evaluate
pub fn entry(app_handle: AppHandle) -> ToolEntry {
    // ...
}
```

#### 3.1.4 BrowserStatus Event Payload

```rust
// src-tauri/src/modules/browser/events.rs

#[derive(Clone, serde::Serialize)]
pub struct BrowserStatusEvent {
    pub session_id: String,
    pub running: bool,
    pub url: Option<String>,
    /// base64 JPEG 缩略图（~15KB）
    pub thumbnail: Option<String>,
}
```

Tauri Event 名称：`"browser-status"`

### 3.2 前端接口

#### 3.2.1 BrowserCard 组件

```tsx
// src/components/browser/BrowserCard.tsx

interface BrowserCardProps {
  sessionId: string;
}

// 监听 Tauri Event "browser-status"
// 显示：缩略图 + URL hostname + 运行状态 + 紧急停止按钮
export function BrowserCard({ sessionId }: BrowserCardProps)
```

#### 3.2.2 Zustand Store Slice

```ts
// src/stores/browser-slice.ts

interface BrowserEntry {
  running: boolean;
  url: string | null;
  thumbnail: string | null;
}

interface BrowserSlice {
  browserBySession: Record<string, BrowserEntry>;
  setBrowserStatus: (sessionId: string, entry: Partial<BrowserEntry>) => void;
  clearBrowserSession: (sessionId: string) => void;
}
```

---

## 4. DOM AXTree 快照算法

从 openhanako 移植 `SNAPSHOT_SCRIPT`，保存为 Rust `include_str!` 嵌入的 JS 文件：

```
src-tauri/src/modules/browser/snapshot.js
```

关键特性（保持与 openhanako 一致）：

1. **可见性过滤**：跳过 `display:none`、`visibility:hidden` 的元素
2. **交互元素检测**：A、BUTTON、INPUT、TEXTAREA、SELECT、role=button/link 等
3. **ref 注入**：给每个可交互元素打上 `data-if2ai-ref="N"` 属性
4. **同构兄弟压缩**：连续 ≥3 个结构相同兄弟节点压缩为单行（节省 token）
5. **字符上限**：30k 字符，头尾截断
6. **返回格式**：`{ text: string, currentUrl: string, title: string }`

通过 `page.evaluate(SNAPSHOT_SCRIPT).await?` 执行并取回结果。

---

## 5. 截图与缩略图

```rust
// 全页截图（供 LLM 视觉感知）
let screenshot_data = page
    .screenshot(ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Jpeg)
        .quality(85)
        .build())
    .await?;
let full_b64 = base64::encode(&screenshot_data);

// 缩略图（供 BrowserCard 显示，压缩至 ~200px 宽）
let thumb_data = page
    .screenshot(ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Jpeg)
        .quality(60)
        .clip(Some(Viewport { x: 0.0, y: 0.0, width: 1280.0, height: 800.0, scale: 0.15 }))
        .build())
    .await?;
```

每次操作（navigate/click/type 等）完成后，自动触发缩略图截图并通过 `browser-status` Event 推送至前端。

---

## 6. Chrome 二进制检测策略

```rust
// src-tauri/src/modules/browser/chrome_finder.rs

/// 按平台顺序查找已安装的 Chrome/Chromium
pub fn find_chrome_binary() -> Option<PathBuf> {
    // macOS: /Applications/Google Chrome.app/...
    //        /Applications/Chromium.app/...
    // Linux: /usr/bin/chromium-browser, /usr/bin/google-chrome, ...
    // Windows: %LOCALAPPDATA%/Google/Chrome/Application/chrome.exe
}

/// 检测结果，由 system_check 暴露给前端
pub enum ChromeStatus {
    Found(PathBuf),
    NotFound,
}
```

Chrome 未找到时，`browser` 工具返回结构化错误，前端在 BrowserCard 区域展示"需要安装 Chrome"的引导提示。

---

## 7. Session 隔离

每个 chat session 使用独立的 Chromium incognito context：

```rust
let context = browser.new_context(
    BrowserContextOptions::builder()
        // 无痕模式：隔离 cookie、storage、cache
        .build()
).await?;
let page = context.new_page().await?;
```

Session 切换时，对应的 `BrowserSession` 保留在 `BrowserRegistry` 的 DashMap 中（不销毁），切回来时直接复用。

---

## 8. 冷保存与恢复

```
~/.if2ai/user/browser-sessions.json

{
  "session-abc123": "https://example.com/some-page",
  "session-xyz789": "https://github.com/openhanako"
}
```

App 启动时，对每个有冷保存记录的 session 在首次访问时执行冷恢复：`launch → navigate(saved_url)`。

---

## 9. 安全约束

| 约束                          | 实现                                                                                   |
| ----------------------------- | -------------------------------------------------------------------------------------- |
| `evaluate` 危险操作需用户授权 | `PermissionMode::AskEveryTime` 模式下 evaluate 触发权限提示                            |
| SSRF 防护                     | browser 工具的 navigate 复用 `web_fetch` 的 SSRF 检测逻辑（blocked cloud metadata IP） |
| 截图泄露                      | 截图数据仅通过 Tauri Event 传输，不写磁盘（除非用户主动保存）                          |
| Chrome 沙箱                   | headless Chrome 默认启用沙箱，不添加 `--no-sandbox`                                    |

---

## 10. BrowserViewer 独立窗口（P1，非 P0）

P0 阶段仅做 headless + BrowserCard 截图流。P1 阶段实现可选的 BrowserViewer 独立窗口：

- 在 `commands/window.rs` 添加 `open_browser_viewer_window` 命令
- 创建新 `WebviewWindowBuilder` 窗口，加载 `index.html?window=browser-viewer`
- 窗口内嵌入 `<webview>` 标签，镜像当前 session 的 URL（非直接 embed chromiumoxide session，而是独立 webview 导航到相同 URL）

> 注：P1 方案不共享 chromiumoxide 的 session 状态（cookie/表单填写内容不同步），主要用于用户查看 AI 正在浏览的内容。

---

## 11. impl_targets 汇总

| Slice      | 新建/修改 文件                                                                                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 7B.1       | `src-tauri/Cargo.toml`（+chromiumoxide）                                                                                                                                                                                             |
| 7B.2       | `src-tauri/src/modules/browser/mod.rs`<br>`src-tauri/src/modules/browser/session.rs`<br>`src-tauri/src/modules/browser/registry.rs`<br>`src-tauri/src/modules/browser/errors.rs`<br>`src-tauri/src/modules/browser/chrome_finder.rs` |
| 7B.3       | `src-tauri/src/modules/browser/snapshot.js`<br>`src-tauri/src/modules/browser/snapshot.rs`                                                                                                                                           |
| 7B.4       | `src-tauri/src/modules/tools/builtin/browser.rs`<br>`src-tauri/src/modules/tools/builtin/mod.rs`（注册）                                                                                                                             |
| 7B.5       | `src-tauri/src/modules/browser/events.rs`<br>`src-tauri/src/commands/browser.rs`<br>`src-tauri/src/lib.rs`（注册 command）                                                                                                           |
| 7B.6       | `src/components/browser/BrowserCard.tsx`<br>`src/stores/browser-slice.ts`<br>`src/modules/chat/components/ChatWorkspace.tsx`（集成）                                                                                                 |
| 7B.7       | `src-tauri/src/modules/browser/cold_state.rs`（冷保存）<br>`src-tauri/src/modules/browser/registry.rs`（session 隔离完善）                                                                                                           |
| 7B.8（P1） | `src-tauri/src/commands/window.rs`（+browser viewer command）<br>`src/modules/browser-viewer/BrowserViewerPage.tsx`                                                                                                                  |

---

## 12. 验收标准

- `cargo test --workspace` 包含 `browser::` 模块测试全部通过
- AI 能完成"导航 → 快照 → 点击 → 输入 → 截图"完整链路
- DOM AXTree 快照中可交互元素携带正确 `data-if2ai-ref` 编号
- 前端 BrowserCard 在 AI 使用 browser 工具时实时显示缩略图
- 两个并发 session 使用 browser 工具时 cookie 完全隔离
- Chrome 未安装时给用户清晰错误提示，不崩溃

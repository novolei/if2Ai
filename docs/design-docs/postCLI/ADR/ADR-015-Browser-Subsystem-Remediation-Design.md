# ADR-015: Browser Subsystem Remediation Design — Phase 7C Gap Closure

**Status**: Proposed
**Date**: 2026-04-18
**Supersedes**: None (extends [Phase 7B browser-control-system.md](../browser-control-system.md))
**Based on**: 对 `openhanako-main`（`lib/browser/`、`lib/tools/browser-tool.js`、`desktop/main.cjs:990-1550`）与 `if2Ai` Phase 7B 当前实现 (`src-tauri/src/modules/browser/`、`src-tauri/src/modules/tools/builtin/browser_tool.rs`、`src-tauri/src/modules/tools/builtin/web_*.rs`) 的逐文件对照审计

---

## Context

Phase 7B 已经把 `chromiumoxide` v0.9 + DOM AXTree 快照管线接入了 if2Ai：12 项 must_implement 全部完成、712 个测试通过、`browser_tool_entry` / `web_search_entry` / `web_fetch_entry` 也都已注册到 `register_builtin_tools()` 并随 `tool_executor.get_definitions()` 喂给 LLM。

但是把 `openhanako-main` 的 Electron `WebContentsView` 路线和我们的 chromiumoxide CDP 路线做完整对照后，发现 12 个**真实存在并已影响 agent 体验**的能力差距 — 既包括 openhanako 已经做对而我们没做的部分（cookies 持久化、用户接管、headed 模式），也包括两边都没做但对真正 agent 上网必不可少的部分（vision pipeline、frame-aware snapshot、download 管线、多 tab 跟踪）。

本 ADR 把这 12 个差距全部转化为可执行的设计规范，按 P0/P1/P2 划入 13 个 slice，目标是把"AI 真的能用浏览器完成代理任务"从 demo 级提升到 production 级。

> **本 ADR 不替换 Phase 7B**：Phase 7B 的 8 个 slice 已 done，本设计只**补缺**和**升级**；不重写已有模块的接口，所有改造保持向后兼容（旧调用路径不破坏）。

---

## Gap Taxonomy

| 优先级 | ID  | 标题                                                                                    | 影响面                                                   |
| ------ | --- | --------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| P0     | G1  | Cookies / 用户登录态完全无法持久化                                                      | "帮我看 GitHub PR" 永远做不到                            |
| P0     | G2  | Vision blind — 截图被当字符串塞进 LLM context                                           | 浪费 30-100k token / 张，LLM 实际看不到图                |
| P0     | G3  | Headed mode 缺失 + viewer 是"假"浏览器                                                  | 用户无法接管、无法预登录                                 |
| P0     | G4  | System prompt 缺 web_search → web_fetch → browser routing 指引                          | LLM 无脑首选 browser，3-8s 冷启 + 触发 bot 检测          |
| P1     | G5  | `wait` state 机被忽略 + click/type 后等待时间过短                                       | SPA 路由没渲染完就 snapshot，AI 误判"按钮没响应"反复点击 |
| P1     | G6  | Multi-tab / popup 完全不跟踪                                                            | `target=_blank` 链接打开后 AI 完全不知道                 |
| P1     | G7  | Frame-aware snapshot 缺失（iframe 失明）                                                | YouTube embed / reCAPTCHA / Stripe Checkout 全部不可见   |
| P1     | G8  | Download / Upload 通道缺失                                                              | 下载发票 PDF、上传凭证图片等真实任务做不了               |
| P2     | G9  | Console / Network 错误未注入 snapshot                                                   | AI 无法自我诊断 "按钮点了为什么没反应"                   |
| P2     | G10 | `web_fetch` 用 regex 提取 HTML，嵌套元素抓不全                                          | 抓取质量差，token 利用率低 3-5×                          |
| P2     | G11 | 缺 `web_research` 复合工具（search → fetch → synth）                                    | "查资料"任务需要 3-5 轮 LLM 调用，体验差                 |
| P2     | G12 | 质量小坑集合（evaluate 无长度上限 / try_lock 死角 / `_state` 未用 / action_log 未对外） | 单点小问题，但累计影响可观测性与稳定性                   |

---

## P0 Critical Gaps

---

### G1: Browser Profile Persistence — 让 AI 真的"代我登录"

**对应代码位置**: `src-tauri/src/modules/browser/session.rs:117-131, 167-173`
**对应 openhanako 设计**: `desktop/main.cjs:1208` `session.fromPartition("persist:hana-browser")`

#### Purpose

当前每次 `BrowserSession::new()` 都用 `tempfile::Builder::tempdir()` 建一个临时 user-data-dir，再用 `start_incognito_context()` 套一层无痕 — Chromium 进程退出，cookies / localStorage / IndexedDB / cache **全部清空**。

后果：
- 所有"需要登录"的 agent 任务（"看下我 GitHub 通知""清掉我 Gmail 垃圾""帮我在 X 发推"）100% 失败
- AI 在 google.com / youtube.com 等 Cloudflare/Akamai-protected 站点上必然被识别为新匿名访客 → bot detection 触发
- 与 openhanako 体验差距巨大：openhanako 用 Electron `session.fromPartition("persist:...")` 直接落盘到 `~/Library/Application Support/openhanako/Partitions/hana-browser/`，cookies 会跟随用户**永久持久化**

#### Current State

```rust
// session.rs:117
let data_dir = tempfile::Builder::new()
    .prefix("if2ai-chrome-")
    .tempdir()
    .map_err(...)?;

// session.rs:134
let config = BrowserConfig::builder()
    .chrome_executable(chrome_path)
    .user_data_dir(data_dir.path())
    .no_sandbox()
    // ...
    .build()?;

// session.rs:167
let page = browser
    .start_incognito_context()  // ← 套一层无痕，cookies 二次隔离
    .await?
    .new_page("about:blank")
    .await?;

// session.rs:89: TempDir 字段，drop 时自动 rm -rf
_data_dir: TempDir,
```

#### Implementation Plan

**Step 1**: 引入 `BrowserProfileMode` 配置

新增 `src-tauri/src/modules/browser/profile.rs`：

```rust
//! Browser profile location & lifecycle policy.

use std::path::PathBuf;

/// 决定 Chromium user-data-dir 的位置和清理策略。
#[derive(Debug, Clone)]
pub enum BrowserProfileMode {
    /// 每个 chat session 独占一个持久化 profile。
    /// 路径：`<data_dir>/if2ai/browser-profiles/<session_id>/`
    /// Cookies / localStorage / cache 长期保留；Drop 不删除。
    PerSessionPersistent,

    /// 全局共享一个 profile，所有 session 复用。
    /// 路径：`<data_dir>/if2ai/browser-profiles/_shared/`
    /// 适合"个人助手"形态：AI 与你共享同一组登录态。
    Shared,

    /// 一次性 incognito（旧的 7B 行为，作为测试 fallback）。
    /// 路径：`tempfile::TempDir`，进程退出全部清空。
    Ephemeral,
}

impl BrowserProfileMode {
    /// 从环境变量 + ~/.if2ai/browser.toml 解析。
    /// 默认值：`PerSessionPersistent`
    pub fn from_env_or_config(if2ai_home: &PathBuf) -> Self { /* ... */ }

    /// 解析出真实 profile 目录，必要时创建。
    pub fn resolve(&self, if2ai_home: &PathBuf, session_id: &str)
        -> Result<ProfileHandle, BrowserError> { /* ... */ }
}

/// `ProfileHandle` 持有 user-data-dir 与可选的 `TempDir` 看守。
pub struct ProfileHandle {
    pub path: PathBuf,
    /// 仅 `Ephemeral` 模式下持有；`Drop` 时清理目录。
    pub _guard: Option<tempfile::TempDir>,
}
```

**Step 2**: 改写 `BrowserSession::new()` 签名

```rust
pub async fn new(
    session_id: String,
    profile_mode: BrowserProfileMode,    // NEW
    if2ai_home: &Path,                   // NEW
) -> Result<Self, BrowserError> {
    let profile = profile_mode.resolve(if2ai_home, &session_id)?;

    let config = BrowserConfig::builder()
        .chrome_executable(chrome_path)
        .user_data_dir(&profile.path)    // ← 已经是持久路径
        .no_sandbox()
        // ... 保留现有 stealth 参数
        .build()?;

    let (mut browser, mut handler) = Browser::launch(config).await?;
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    // 不再调用 start_incognito_context()
    let page = browser.new_page("about:blank").await?;

    page.enable_stealth_mode_with_agent(/* unchanged */).await?;

    Ok(Self {
        session_id,
        browser,
        _handler: handler_task,
        page,
        current_url: None,
        action_log: Vec::new(),
        _profile: profile,            // 替换 _data_dir
        // ...
    })
}
```

**Step 3**: `BrowserRegistry::new()` 接收 `profile_mode` + `if2ai_home`

```rust
pub fn new(
    cold_state_path: PathBuf,
    profile_mode: BrowserProfileMode,  // NEW
    if2ai_home: PathBuf,               // NEW
) -> Arc<Self> { /* ... */ }
```

**Step 4**: `main.rs` 接入

```rust
let profile_mode = modules::browser::profile::BrowserProfileMode::from_env_or_config(&if2ai_dir);
let browser_registry = modules::browser::BrowserRegistry::new(
    if2ai_dir.join("browser-cold-state.json"),
    profile_mode,
    if2ai_dir.clone(),
);
```

**Step 5**: 新增 Tauri 命令 `clear_browser_profile(session_id)` 和 `list_browser_profiles()`

`commands/browser.rs`：

```rust
/// 清空指定 session 的 profile（cookies / localStorage 全部抹除）。
/// 必须先确保 session 已 close，否则返回错误。
#[tauri::command]
pub async fn clear_browser_profile(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> { /* ... */ }

/// 列出磁盘上所有已建立的 browser profile（含磁盘占用）。
#[tauri::command]
pub async fn list_browser_profiles(/* ... */) -> Result<Vec<ProfileEntry>, String> { /* ... */ }
```

**Step 6**: `~/.if2ai/browser.toml` 默认配置

```toml
[browser]
profile_mode = "per_session_persistent"   # | "shared" | "ephemeral"
# 单个 profile 磁盘上限（超过后 LRU 清理最老 session 的 profile）
max_profile_disk_mb = 500
# 全局磁盘上限（防止失控）
max_total_disk_mb = 5000
```

#### Target Files

| File                                        | Change                                                                           |
| ------------------------------------------- | -------------------------------------------------------------------------------- |
| `src-tauri/src/modules/browser/profile.rs`  | **NEW** — `BrowserProfileMode` + `ProfileHandle`                                 |
| `src-tauri/src/modules/browser/session.rs`  | `new()` 签名 + 移除 `start_incognito_context()` + 替换 `_data_dir` 为 `_profile` |
| `src-tauri/src/modules/browser/registry.rs` | `new()` 签名 + 把 `profile_mode` 透传给 `BrowserSession::new()`                  |
| `src-tauri/src/modules/browser/mod.rs`      | `pub mod profile; pub use profile::BrowserProfileMode;`                          |
| `src-tauri/src/commands/browser.rs`         | `clear_browser_profile`、`list_browser_profiles` 两个新命令                      |
| `src-tauri/src/main.rs`                     | 解析 `browser.toml` 并构造 `BrowserProfileMode`                                  |
| `src-tauri/gen/schemas/capabilities.json`   | 注册新命令权限                                                                   |

#### Risks

- **磁盘膨胀**：每个 session 独立 profile 累积可能 GB 级。缓解：`browser.toml` 的 `max_total_disk_mb` 软上限 + LRU 清理（Step 6 配套）。
- **隐私顾虑**：Cookies 持久化意味着 AI session 之间会"互相认识"。缓解：`Shared` 模式必须用户主动开启；前端 BrowserCard 加"清空 profile"按钮。
- **测试稳定性**：`PerSessionPersistent` 让单元测试间状态泄漏。缓解：所有测试显式传 `BrowserProfileMode::Ephemeral`。

#### Review Standards

- [ ] `BrowserProfileMode` 枚举三种模式都有 doc comment 与单测
- [ ] `BrowserSession::new()` 不再调用 `start_incognito_context()`
- [ ] `ProfileHandle::Drop` 仅在 `Ephemeral` 时删目录
- [ ] `main.rs` 默认 `PerSessionPersistent`
- [ ] `clear_browser_profile` 在 session 仍 running 时返回明确错误
- [ ] 集成测试：launch → set cookie → close → relaunch → cookie 仍存在（PerSessionPersistent）
- [ ] 集成测试：launch → set cookie → close → relaunch → cookie 不存在（Ephemeral）

---

### G2: Multimodal Vision Pipeline — 让 LLM 真的"看见"截图

**对应代码位置**: `src-tauri/src/modules/tools/builtin/browser_tool.rs:367`、`src-tauri/src/modules/tools/registry.rs:209`
**对应 openhanako 设计**: `lib/tools/browser-tool.js:140-148` 把截图作为 `{type: "image", mimeType, data}` content part 返回

#### Purpose

`browser_tool` 的 `screenshot` action 当前返回值是：

```rust
// browser_tool.rs:367
Ok(format!("data:image/jpeg;base64,{b64}"))
```

这是**纯字符串**，被当成普通 tool 文本结果塞进 LLM 的 context。一张 1280×800 的 JPEG → 30-100k base64 token，**而 LLM 完全看不到任何视觉信息**：因为它收到的不是 OpenAI/Anthropic 的多模态 `image_url` content part，而是 `"data:image/jpeg;base64,/9j/..."` 一坨 ASCII。

后果：
- 一次 screenshot 浪费 30-100k token（~$0.5-2 / 调用）
- AI 看不见 → 后续推理只能基于已有的 AXTree 文本快照 → screenshot 工具实际是**负价值工具**（消耗 token 没收益）
- 视觉布局判断（"按钮在屏幕右下角""图表显示下降趋势"）100% 做不到 → vision-required 任务失败

#### Current State

`ToolHandler` 类型：
```rust
// registry.rs:209
pub type ToolHandler = Arc<
    dyn Fn(Value, SharedToolContext)
        -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send>>
        + Send + Sync,
>;
```

返回 `Result<String, ToolError>` — **只能返回字符串**。这是阻塞多模态的根本约束。

#### Implementation Plan

**Step 1**: 引入 `ToolOutput` 内容部分枚举

新增 `src-tauri/src/modules/tools/output.rs`：

```rust
//! Tool 多模态输出表示。

use serde::{Deserialize, Serialize};

/// 单条工具结果可包含多个 content part（文本 + 图片混排）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResultPart {
    Text { text: String },
    Image {
        /// e.g. "image/jpeg", "image/png"
        mime: String,
        /// base64-encoded raw bytes (no `data:` prefix)
        data: String,
        /// 可选的 alt 文本，描述截图内容（同时供低成本 LLM 使用）
        #[serde(skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
    },
}

/// 完整工具输出。`Display` 实现把所有 Image 折叠为占位文本，
/// 用于向后兼容 `Result<String, ToolError>` 路径。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub parts: Vec<ToolResultPart>,
}

impl ToolOutput {
    pub fn text(s: impl Into<String>) -> Self {
        Self { parts: vec![ToolResultPart::Text { text: s.into() }] }
    }

    pub fn image(mime: impl Into<String>, data: impl Into<String>) -> Self {
        Self { parts: vec![ToolResultPart::Image {
            mime: mime.into(), data: data.into(), alt: None,
        }] }
    }

    /// 折叠为纯文本（图片以 `[image: <mime> <bytes>B]` 占位）。
    pub fn to_legacy_string(&self) -> String { /* ... */ }

    /// 总字节数估算（图片按 base64 字节计入 max_result_size）。
    pub fn byte_size(&self) -> usize { /* ... */ }
}
```

**Step 2**: `ToolHandler` 改返回 `ToolOutput`，保留向后兼容

```rust
// registry.rs:209
pub type ToolHandler = Arc<
    dyn Fn(Value, SharedToolContext)
        -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send>>
        + Send + Sync,
>;

/// 让旧 handler（返回 String）依然能注册。
impl<F, Fut> From<F> for ToolHandler where /* ... */ {
    fn from(f: F) -> Self { /* wrap String → ToolOutput::text */ }
}
```

`dispatch_with_context` 同步改返回 `Result<ToolOutput, ToolError>`，旧调用方走 `.to_legacy_string()` 提取文本。

**Step 3**: `browser_tool::screenshot` 改成

```rust
"screenshot" => {
    ensure_running_or_restore(&registry, &session_id).await?;
    match registry.screenshot(&session_id).await {
        Ok(b64) => Ok(ToolOutput {
            parts: vec![
                ToolResultPart::Text { text: format!(
                    "Screenshot captured at {} — render attached for vision-capable models.",
                    registry.current_url(&session_id).unwrap_or_default()
                )},
                ToolResultPart::Image {
                    mime: "image/jpeg".into(),
                    data: b64,
                    alt: Some("Current browser viewport".into()),
                },
            ],
        }),
        Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(/* ... */).await),
        Err(e) => Err(ToolError::Handler(e.to_string())),
    }
}
```

**Step 4**: Provider adapter 转换层

`src-tauri/src/modules/api/`（已有 OpenAI/Anthropic/Gemini provider 适配层）：

- OpenAI：`ToolResultPart::Image { mime, data, .. }` → `{"type": "image_url", "image_url": {"url": "data:<mime>;base64,<data>"}}`
- Anthropic：→ `{"type": "image", "source": {"type": "base64", "media_type": mime, "data": data}}`
- Gemini：→ `{"inlineData": {"mimeType": mime, "data": data}}`
- 其他不支持 vision 的 provider：fallback 到 `to_legacy_string()`，但跳过 base64 内容（用 `[image not shown to non-vision model]` 占位）

每个 provider 增加一个 `supports_vision()` 静态方法 + 转换逻辑。

**Step 5**: `max_result_size` 语义调整

图像 base64 不计入 `max_result_size`（否则 64KB 限制会立刻拒绝任何截图）。改为：

```rust
pub struct ToolEntry {
    // ... existing
    /// 仅文本部分的字节上限。
    pub max_text_bytes: Option<usize>,
    /// 总图片字节上限（base64 计算后）。新增字段，None = 5MB 默认。
    pub max_image_bytes: Option<usize>,
}
```

`browser` tool：`max_text_bytes = 64KB`、`max_image_bytes = 5MB`。

#### Target Files

| File                                                  | Change                                                                                       |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `src-tauri/src/modules/tools/output.rs`               | **NEW** — `ToolResultPart` + `ToolOutput`                                                    |
| `src-tauri/src/modules/tools/registry.rs`             | `ToolHandler` 改返回 `ToolOutput`，加旧签名兼容 wrapper；`max_result_size` 拆分为 text/image |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | `screenshot` action 返回 `ToolOutput` 含 image part；其余 action 用 `ToolOutput::text`       |
| `src-tauri/src/modules/api/openai.rs`                 | 处理 image content part → `image_url` 块                                                     |
| `src-tauri/src/modules/api/anthropic.rs`              | 处理 image content part → `image` 块                                                         |
| `src-tauri/src/modules/api/gemini.rs`                 | 处理 image content part → `inlineData` 块                                                    |
| `src-tauri/src/modules/api/mod.rs`                    | 抽 `ProviderCapabilities { supports_vision: bool }`                                          |

#### Risks

- **Provider 协议差异**：Anthropic 限制 image ≤ 5MB、≤ 8000×8000 px；OpenAI 限制 ≤ 20MB；Gemini 区分 inline vs File API。缓解：在 provider adapter 内做尺寸校验，超限自动 resize（用 `image` crate）。
- **旧测试断言字符串**：现有针对 `screenshot` 返回 `"data:image/jpeg;base64,..."` 的测试会失败。缓解：迁移到 `ToolOutput::parts[1]` 断言，并保留 `to_legacy_string()` 的兼容快照测试。
- **流式工具结果**：现在的工具结果走文本流；image part 不能流式发。缓解：image part 始终在 final tool result 中一次性附带，不影响流式 text。

#### Review Standards

- [ ] `ToolOutput` 与 `ToolResultPart` 实现 `Serialize` / `Deserialize` / `Clone`
- [ ] `to_legacy_string()` 对所有 part 类型有覆盖
- [ ] OpenAI / Anthropic / Gemini 三个 adapter 各有单测验证 image part 转换
- [ ] `browser` tool screenshot action 返回的 `ToolOutput.parts` 长度为 2（Text + Image）
- [ ] `max_image_bytes` 默认值 5MB；超出时返回 `ToolError::OutputTooLarge`
- [ ] vision-incapable provider 收到 image part 时不 panic、不发垃圾 base64

---

### G3: Headed Mode + 真正的用户接管

**对应代码位置**: `src-tauri/src/modules/browser/session.rs:128-154`、`src/modules/browser-viewer/BrowserViewerPage.tsx`、`src-tauri/src/modules/viewer_registry.rs:23`
**对应 openhanako 设计**: `desktop/main.cjs:1209-1259` `WebContentsView` 直接挂在 BrowserViewerWindow 内 + `setWindowOpenHandler` + `view.webContents.focus()`

#### Purpose

当前的 `BrowserViewerPage` + `viewer_registry::sync_viewer_url` 是个**视觉幻觉**：

- AI 通过 chromiumoxide 操作的是一个 **headless Chromium 子进程**
- BrowserViewer 窗口里显示的是**另一个独立的 native WebView**（macOS WKWebView / Win WebView2）
- 两者**不共享 cookies、不共享 DOM、不共享 session storage**
- 用户在 viewer 里点击/滚动/输入，**对 AI 那个 Chromium 完全没影响**
- 用户想"我先登录一下，然后让 AI 接着用" — **做不到**

后果：
- "可见"承诺破产 — 用户其实看到的是个 placeholder
- 解决 Cloudflare/CAPTCHA 的最后退路（人机协同）也没有
- 与 openhanako"用户能直接接管浏览器"的体验差距巨大

#### Current State

```rust
// viewer_registry.rs:23
pub(crate) fn sync_viewer_url(session_id: &str, url: &str) {
    if let Some(webview) = VIEWER_CONTENT_WEBVIEWS.get(session_id) {
        if let Ok(parsed) = url.parse::<Url>() {
            let _ = webview.navigate(parsed);  // ← 让"另一个"WebView 也访问相同 URL
        }
    }
}
```

```rust
// session.rs:134
let config = BrowserConfig::builder()
    .chrome_executable(chrome_path)
    .user_data_dir(...)
    .no_sandbox()
    // 没有 .with_head() — 永远 headless
    .build()?;
```

#### Implementation Plan

**Step 1**: 在 `BrowserSession` 内增加 `headed: bool` 字段 + 启动时按需开窗

```rust
pub struct BrowserSession {
    // ... existing
    /// 当前是否以可见窗口运行；切换时需要重启浏览器（CDP 不支持运行时切换）。
    pub headed: bool,
}

impl BrowserSession {
    pub async fn new(
        session_id: String,
        profile_mode: BrowserProfileMode,
        if2ai_home: &Path,
        headed: bool,                       // NEW
    ) -> Result<Self, BrowserError> {
        let mut builder = BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .user_data_dir(&profile.path)
            .no_sandbox()
            // ... 既有参数
            ;

        if headed {
            builder = builder
                .with_head()
                // 窗口位置：靠右上角，避免遮挡 if2Ai 主窗口
                .arg("--window-position=900,80")
                .arg("--window-size=1024,768");
        }

        // ... 余下不变
    }
}
```

**Step 2**: `BrowserRegistry` 加 `relaunch_with_mode(session_id, headed)` 接口

```rust
/// 切换 headed/headless 模式。会保留 cookies（profile 持久），
/// 但当前页面状态（表单、滚动）会丢失（CDP 无法跨进程迁移 page state）。
pub async fn relaunch_with_mode(
    &self,
    session_id: &str,
    headed: bool,
) -> Result<NavigateResult, BrowserError> {
    let url_to_resume = self.current_url(session_id);
    self.close(session_id).await?;
    // launch 时使用同一个 profile_mode（持久化）+ 新的 headed
    let session = BrowserSession::new(
        session_id.to_owned(),
        self.profile_mode.clone(),
        &self.if2ai_home,
        headed,
    ).await?;
    self.sessions.insert(session_id.to_owned(), Arc::new(Mutex::new(session)));
    if let Some(url) = url_to_resume {
        self.navigate(session_id, &url).await
    } else {
        Ok(NavigateResult { url: "about:blank".into(), title: "".into(), snapshot: "".into() })
    }
}
```

**Step 3**: 新增 Tauri 命令 `request_browser_takeover` + `release_browser_takeover`

```rust
/// 用户点击 BrowserCard 上的"接管"按钮 → 把 headless Chrome 重启为 headed，
/// 把窗口 focus 到前台。AI 暂停所有 browser 操作，等用户做完后调 release_*。
#[tauri::command]
pub async fn request_browser_takeover(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<NavigateResult, String> {
    registry.set_takeover(&session_id, true);
    registry.relaunch_with_mode(&session_id, true).await
        .map_err(|e| e.to_string())
}

/// 用户做完，释放接管。可选 `back_to_headless = true` 把浏览器收回后台。
#[tauri::command]
pub async fn release_browser_takeover(
    session_id: String,
    back_to_headless: bool,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    registry.set_takeover(&session_id, false);
    if back_to_headless {
        let _ = registry.relaunch_with_mode(&session_id, false).await;
    }
    Ok(())
}
```

**Step 4**: Tool 层尊重 takeover 标志

`browser_tool.rs` 的 `execute_browser_action` 入口加守卫：

```rust
if registry.is_taken_over(&session_id) {
    return Err(ToolError::Handler(
        "User has taken over the browser; AI tools are paused. \
         Wait for the user to release control before retrying.".into()
    ));
}
```

**Step 5**: 替换"视觉幻觉" Viewer 为"实时缩略图 + 一键接管"

`BrowserViewerPage.tsx` 改造：
- 默认显示 `browser-status` event 推送的 thumbnail（每次 AI action 后更新，~200-400ms 刷新一次）
- "实时模式"按钮：调 `request_browser_takeover` → 系统弹出真正的 Chrome 窗口 → BrowserCard 上显示"用户接管中"徽章
- 移除掉那个会误导用户的"独立 native WebView mirror"代码（`navigate_viewer_window` 等命令保留作为兼容，但默认隐藏）

**Step 6**: `setWindowOpenHandler` 等价行为（chromiumoxide 中通过监听 `Target.targetCreated` 事件 — 与 G6 共享）

#### Target Files

| File                                                  | Change                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------- |
| `src-tauri/src/modules/browser/session.rs`            | `new()` 加 `headed` 参数；`with_head()` + 窗口位置参数                  |
| `src-tauri/src/modules/browser/registry.rs`           | `relaunch_with_mode`、`set_takeover`、`is_taken_over`                   |
| `src-tauri/src/commands/browser.rs`                   | `request_browser_takeover`、`release_browser_takeover`                  |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | 入口检查 `is_taken_over`                                                |
| `src/modules/browser-viewer/BrowserViewerPage.tsx`    | 移除独立 WebView mirror；改用 thumbnail polling + 接管按钮              |
| `src/components/browser/BrowserCard.tsx`              | 加"接管浏览器"按钮 + "用户接管中"徽章状态                               |
| `src/lib/tauri.ts`                                    | 增加 `requestBrowserTakeover` / `releaseBrowserTakeover` invoke wrapper |

#### Risks

- **重启丢失 page state**：headed/headless 切换需要 relaunch，未提交的表单数据会丢。缓解：`relaunch_with_mode` 在切换前 `evaluate("document.title + ' | ' + JSON.stringify(Array.from(document.forms).map(f=>f.outerHTML.length))')` 警告用户；UI 弹"切换前要保存表单吗？"
- **Headed 在无图形环境**：CI / SSH 远程必须是 headless。缓解：`request_browser_takeover` 在 `DISPLAY` 变量缺失（Linux）或前端报告"无可见显示器"时报错回退。
- **Tool 调用并发竞争**：用户接管期间 AI 还可能尝试发 tool call。缓解：Step 4 守卫；额外在 system prompt 中告知 LLM "若收到 'User has taken over' 错误，等下一轮用户消息再继续"。

#### Review Standards

- [ ] `BrowserConfig::with_head()` 仅在 `headed=true` 调用
- [ ] `relaunch_with_mode` 在重启前后 cookies 保留（依赖 G1 的 PerSessionPersistent）
- [ ] `is_taken_over` 在 `BrowserRegistry` 内用 `DashMap<String, bool>` 表示
- [ ] `BrowserViewerPage` 不再调用 `webview.navigate`
- [ ] BrowserCard "接管"按钮在 `running=false` 时禁用
- [ ] 集成测试：模拟用户 takeover → AI 工具调用返回 `paused` 错误 → release → AI 调用恢复

---

### G4: System Prompt Routing — 让 LLM 知道何时用哪个 web 工具

**对应代码位置**: `src-tauri/src/modules/runtime/prompt.rs:734`
**对应 openhanako 设计**: `core/agent.js:676-683` 三段式 routing 提示

#### Purpose

`web_search` / `web_fetch` / `browser` 三个工具都注册了，LLM 可见的 schema 是：

- `web_search`: "Search the web for information."
- `web_fetch`: "Fetch web page content"
- `browser`: "Control an interactive headless web browser. ... Actions: start | stop | navigate | snapshot | screenshot | click | type | scroll | select | key | wait | evaluate"

后果：从工具描述看，`browser` **看起来最强大**（11 actions vs 1 action），LLM 经常**首选 browser**：
- 冷启动 3-8s（chromiumoxide + Chromium 进程拉起）
- 触发 bot detection 概率高于 HTTP fetch
- 多轮 click/type/snapshot → 总 token 远高于一次 web_fetch
- 真实测试中（Phase 7B 完成后人工验证）"今天天气怎么样"这种问题 LLM 也会去 launch browser

openhanako 在 `core/agent.js:676` 给出明确路由：
> "**browser** — 只在以下情况使用：页面需要登录/身份验证、需要填表或点击交互、web_fetch 返回的内容为空或不完整（JS 动态渲染页面）、需要查看页面视觉布局。**Do not** launch the browser when web_search or web_fetch can do the job."

#### Current State

```rust
// prompt.rs:734
"You are an interactive agent that helps users {} Use the instructions
below and the tools available to you to assist the user.\n\nIMPORTANT: ..."
```

无任何 browser/web_fetch/web_search 之间的 routing 指引。

#### Implementation Plan

**Step 1**: 抽出 `tools_routing_guide()` 函数

新增 `src-tauri/src/modules/runtime/prompt_tools_guide.rs`：

```rust
//! Tool routing guidance injected into the system prompt.
//! Helps LLM choose the cheapest tool that can do the job.

/// Generate web-tools routing block.
/// Conditional: only included when at least 2 of {web_search, web_fetch, browser}
/// are registered, otherwise no advice needed.
pub fn web_tools_routing_block(registered: &[&str]) -> Option<String> {
    let has_search = registered.contains(&"web_search");
    let has_fetch  = registered.contains(&"web_fetch");
    let has_browser = registered.contains(&"browser");
    if [has_search, has_fetch, has_browser].iter().filter(|x| **x).count() < 2 {
        return None;
    }

    Some(format!(
        "## Web Access Tool Routing\n\
         You have multiple web access tools. ESCALATE FROM CHEAP TO EXPENSIVE:\n\
         \n\
         1. **web_search** — DEFAULT for finding information. Use first when you need to discover URLs or get a quick overview. Cost: 1 API call, ~1-2s.\n\
         2. **web_fetch** — When you have a SPECIFIC URL and want clean text content. Cost: 1 HTTP request, ~1-3s.\n\
         3. **browser** — LAST RESORT. Use ONLY when:\n   • The site requires login or session cookies (e.g., GitHub notifications, Gmail)\n   • You need to fill a form, click a button, or interact with UI\n   • web_fetch returned empty or incomplete content (SPA / JS-rendered pages like Twitter, Notion)\n   • You need to see the visual layout (then use action='screenshot')\n\
         \n\
         RULES:\n\
         • Browser cold-start is 3-8s and may trigger bot detection on Cloudflare/Akamai sites — never use it speculatively.\n\
         • If web_fetch returns substantial text, do NOT escalate to browser unless the user explicitly asks for visual / interactive work.\n\
         • If browser navigation hits a CAPTCHA / Cloudflare challenge, IMMEDIATELY fall back to web_search or web_fetch — do not retry click.\n\
         • If user takes over the browser, all browser tool calls return a 'paused' error — wait for the next user message before retrying.\n"
    ))
}
```

**Step 2**: 在 `prompt.rs` 拼接系统提示时调用

`prompt.rs::build_system_prompt(...)`（具体位置在文件 700-780 行附近）：

```rust
let registered_names: Vec<&str> = tool_registry.list_names_borrowed();
if let Some(block) = prompt_tools_guide::web_tools_routing_block(&registered_names) {
    sections.push(block);
}
```

**Step 3**: 类似地补 file 工具路由（顺手做）

```rust
pub fn file_tools_routing_block(registered: &[&str]) -> Option<String> {
    // 例：read_file → grep_search → glob_search 何时用哪个
}
```

**Step 4**: 单元测试覆盖

- 注册 web_search + web_fetch + browser → 应包含全部三段
- 仅注册 web_fetch → 返回 None
- 注册 web_fetch + browser（无 web_search）→ 仍包含说明（提示先 fetch 再 browser）

#### Target Files

| File                                                  | Change                              |
| ----------------------------------------------------- | ----------------------------------- |
| `src-tauri/src/modules/runtime/prompt_tools_guide.rs` | **NEW**                             |
| `src-tauri/src/modules/runtime/prompt.rs`             | 调用 `web_tools_routing_block` 拼接 |
| `src-tauri/src/modules/runtime/mod.rs`                | `pub mod prompt_tools_guide;`       |

#### Risks

- **太强的 routing 导致漏用 browser**：如果 routing 太严，LLM 会拒绝在该用 browser 时用。缓解：明确列举"何时必须用 browser"（登录、表单、SPA）。
- **prompt token 占用**：~250 token。缓解：仅在多个 web 工具同时注册时才插入。

#### Review Standards

- [ ] `web_tools_routing_block` 仅在 ≥2 web 工具注册时返回 Some
- [ ] 提示文本明确包含"DO NOT use browser if web_fetch can do the job"
- [ ] 提示文本提及 takeover paused 错误的处理
- [ ] 单测覆盖 4 种组合（单工具 / 双工具 / 全工具 / 零工具）

---

## P1 High Gaps

---

### G5: `wait` State Machine + 时序修正

**对应代码位置**: `src-tauri/src/modules/browser/session.rs:390, 441, 471, 526, 539-548`

#### Purpose

当前实现两个时序问题：

1. **`wait()` 完全忽略 `state` 参数**（line 539）：
   ```rust
   pub async fn wait(&mut self, timeout_ms: u64, _state: &str) -> Result<String, BrowserError> {
       let _ = tokio::time::timeout(
           Duration::from_millis(timeout_ms),
           self.page.wait_for_navigation(),  // ← 不管 state 是什么都走这个
       ).await;
       // ...
   }
   ```
   AI 想"等到 networkidle" 永远拿不到真实信号。

2. **click / type / scroll 后等待时间过短**：
   - `click` 后 `sleep(200ms)` (line 390)，openhanako 是 800ms
   - `type_text` 后 `sleep(150ms)` (line 441)，openhanako 是 300-800ms（按是否 pressEnter）
   - `scroll` 后 `sleep(150ms)` (line 471)，openhanako 是 500ms
   - `press_key` 后 `sleep(150ms)` (line 526)

   后果：SPA 路由切换通常需要 300-600ms，AI 收到的 snapshot 是**变更前**的页面 → 误判"按钮没响应"再点一次 → 双击 / 重复提交频发。

#### Current State

参见上方代码片段。

#### Implementation Plan

**Step 1**: 引入 `WaitState` 枚举

```rust
// session.rs
#[derive(Debug, Clone, Copy)]
pub enum WaitState {
    /// `Page.domContentEventFired` — DOM 树构建完毕。
    DomContentLoaded,
    /// `Page.loadEventFired` — load 事件触发（图片/资源加载完）。
    Load,
    /// 网络空闲：`Network.requestWillBeSent` 与 `responseReceived` 计数 0 持续 500ms。
    NetworkIdle,
}

impl WaitState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "load" => Self::Load,
            "networkidle" => Self::NetworkIdle,
            _ => Self::DomContentLoaded,
        }
    }
}
```

**Step 2**: 真实实现三种等待

```rust
pub async fn wait(&mut self, timeout_ms: u64, state: WaitState) -> Result<String, BrowserError> {
    use chromiumoxide::cdp::browser_protocol::page::*;
    use chromiumoxide::cdp::browser_protocol::network::*;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    match state {
        WaitState::DomContentLoaded => {
            let mut events = self.page
                .event_listener::<EventDomContentEventFired>()
                .await
                .map_err(|e| BrowserError::Cdp(e.to_string()))?;
            tokio::time::timeout_at(deadline, events.next()).await.ok();
        }
        WaitState::Load => {
            let mut events = self.page
                .event_listener::<EventLoadEventFired>()
                .await
                .map_err(|e| BrowserError::Cdp(e.to_string()))?;
            tokio::time::timeout_at(deadline, events.next()).await.ok();
        }
        WaitState::NetworkIdle => {
            self.wait_network_idle(deadline, Duration::from_millis(500)).await?;
        }
    }

    self.sync_url().await;
    self.run_snapshot().await
}

async fn wait_network_idle(
    &self,
    deadline: Instant,
    quiet_window: Duration,
) -> Result<(), BrowserError> {
    let mut started = self.page
        .event_listener::<EventRequestWillBeSent>()
        .await?;
    let mut finished = self.page
        .event_listener::<EventResponseReceived>()
        .await?;

    let mut inflight: i64 = 0;
    let mut last_zero_at = Instant::now();

    loop {
        if Instant::now() >= deadline { break; }
        tokio::select! {
            _ = started.next() => { inflight += 1; }
            _ = finished.next() => {
                inflight = (inflight - 1).max(0);
                if inflight == 0 { last_zero_at = Instant::now(); }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if inflight == 0 && last_zero_at.elapsed() >= quiet_window {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
```

**Step 3**: click / type / scroll / press_key 默认延迟提升 + 配置化

新增常量：

```rust
const DELAY_AFTER_CLICK_MS: u64 = 600;
const DELAY_AFTER_TYPE_MS: u64 = 300;
const DELAY_AFTER_TYPE_ENTER_MS: u64 = 800;
const DELAY_AFTER_SCROLL_MS: u64 = 400;
const DELAY_AFTER_KEY_MS: u64 = 300;
```

并把硬 sleep 替换为 "短 sleep + wait_for_load(state, 1500ms) 双保险"：

```rust
pub async fn click(&mut self, ref_num: u32) -> Result<String, BrowserError> {
    // ... 既有 click 逻辑 ...

    tokio::time::sleep(Duration::from_millis(DELAY_AFTER_CLICK_MS)).await;
    // 弱等待：如果有导航发生就跟，没有就立即 snapshot
    let _ = tokio::time::timeout(
        Duration::from_millis(1500),
        self.page.wait_for_navigation(),
    ).await;

    self.sync_url().await;
    let snapshot = self.run_snapshot().await?;
    // ...
}
```

**Step 4**: tool 层 `wait` action 把字符串映射到 `WaitState`

```rust
"wait" => {
    let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("domcontentloaded");
    let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
    let wait_state = WaitState::from_str(state);
    registry.wait(&session_id, timeout_ms, wait_state).await
        .map(ToolOutput::text)
        .map_err(/* ... */)
}
```

**Step 5**: input_schema 把 state enum 暴露给 LLM

```json
"state": {
    "type": "string",
    "enum": ["load", "domcontentloaded", "networkidle"],
    "description": "What to wait for. Default: domcontentloaded.",
    "default": "domcontentloaded"
}
```

#### Target Files

| File                                                  | Change                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------- |
| `src-tauri/src/modules/browser/session.rs`            | `WaitState` 枚举 + `wait_network_idle` + click/type/scroll/key 延迟提升 |
| `src-tauri/src/modules/browser/registry.rs`           | `wait()` 签名改 `WaitState`                                             |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | `wait` action 映射 + input_schema 加 enum                               |

#### Risks

- **NetworkIdle 永不触发**：长连接（WebSocket / Server-Sent Events）会让 inflight 永远 > 0。缓解：`quiet_window=500ms` 内有任何完成就重置；超 timeout 优雅返回。
- **延迟变长降低吞吐**：每个 click 多等 400ms。缓解：tool schema 加 `quick: bool` 选项（AI 显式跳过等待，用于已知快速响应的页面）。

#### Review Standards

- [ ] `WaitState` 枚举三个变体都有 doc comment
- [ ] `wait_network_idle` 被 `tokio::select!` 实现，不阻塞 runtime
- [ ] click / type / scroll / press_key 默认延迟从 200/150/150/150 提升到 600/300/400/300
- [ ] tool schema 中 `state` 字段是 enum 不是自由字符串
- [ ] 集成测试：navigate → wait(networkidle, 5000) 在 example.com 上 < 2s 返回

---

### G6: Multi-Tab / Popup Tracking

**对应代码位置**: `src-tauri/src/modules/browser/session.rs:74` (`page: Page` 单页字段)

#### Purpose

`BrowserSession` 持有一个 `page: Page` 字段。当页面调用 `window.open(...)` 或用户点击 `<a target="_blank">` 时，chromiumoxide 会在 `Browser` 上创建一个**新 Page**，但 `BrowserSession.page` 仍指向旧的，AI 完全看不见新 tab。

后果：
- "在新 tab 里打开商品详情" → AI 看到原页没变，误以为点击失败
- 银行/电商常见的"新窗口确认支付" → AI 完全无法跟随

#### Implementation Plan

**Step 1**: `BrowserSession` 改持有 `pages: Vec<Page>` + `active_idx: usize`

```rust
pub struct BrowserSession {
    // ... existing 但移除 page 字段
    pages: Vec<Page>,
    active_idx: usize,
}

impl BrowserSession {
    fn active_page(&self) -> &Page { &self.pages[self.active_idx] }
}
```

**Step 2**: 启动时监听 `Target.targetCreated` / `targetDestroyed`

```rust
pub async fn new(/* ... */) -> Result<Self, BrowserError> {
    let (mut browser, mut handler) = Browser::launch(config).await?;

    // 接收 target 变化
    let mut target_events = browser
        .event_listener::<EventTargetCreated>()
        .await?;

    let pages = Arc::new(tokio::sync::Mutex::new(Vec::<Page>::new()));
    let initial = browser.new_page("about:blank").await?;
    pages.lock().await.push(initial);

    let pages_clone = Arc::clone(&pages);
    let browser_handle = /* obtain shareable handle */;
    let watcher = tokio::spawn(async move {
        while let Some(evt) = target_events.next().await {
            if evt.target_info.r#type == "page" {
                if let Ok(p) = browser_handle.attach_to_page(&evt.target_info.target_id).await {
                    pages_clone.lock().await.push(p);
                    // emit Tauri event browser-tab-opened
                }
            }
        }
    });
    // ...
}
```

**Step 3**: 新增 `tabs` / `switch_tab` / `close_tab` actions

`browser_tool.rs` schema 加：

```json
"action": { "enum": [..., "tabs", "switch_tab", "close_tab"] }
```

```rust
"tabs" => {
    let list = registry.list_tabs(&session_id).await?;
    Ok(ToolOutput::text(
        list.iter().enumerate().map(|(i, t)| {
            format!("[{}]{} {} — {}", i,
                if t.active { " *" } else { "  " },
                t.url, t.title)
        }).collect::<Vec<_>>().join("\n")
    ))
}
"switch_tab" => {
    let idx = args.get("tab_index").and_then(|v| v.as_u64()).ok_or(...)? as usize;
    let snapshot = registry.switch_tab(&session_id, idx).await?;
    Ok(ToolOutput::text(snapshot))
}
```

**Step 4**: 自动 active 新打开的 tab（可配置）

```rust
// browser.toml
[browser.tabs]
auto_focus_new = true     # 新 tab 自动成为 active，旧 tab 后台保留
```

**Step 5**: snapshot 末尾追加"Other tabs"提示

```
## Other tabs (use action='switch_tab' with tab_index to view):
[1] https://example.com/cart — Shopping Cart
[2] https://example.com/payment — Confirm Payment (NEW)
```

#### Target Files

| File                                                  | Change                                           |
| ----------------------------------------------------- | ------------------------------------------------ |
| `src-tauri/src/modules/browser/session.rs`            | `pages: Vec<Page>` + 监听 `Target.targetCreated` |
| `src-tauri/src/modules/browser/registry.rs`           | `list_tabs`、`switch_tab`、`close_tab` 委托      |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | 新 actions + snapshot 末尾追加 tabs 列表         |
| `src-tauri/src/modules/browser/events.rs`             | 新增 `BrowserTabsEvent`                          |

#### Risks

- **Tab 数量爆炸**：恶意脚本 `for(;;) window.open()` → OOM。缓解：硬上限 max_tabs=10，超出自动关闭最旧的。
- **跨 page snapshot ref 混淆**：每个 page 各有自己的 `data-if2ai-ref` 序号。缓解：snapshot 内显示当前 active tab + 序号，AI 在不同 tab 操作前必须先 switch_tab。

#### Review Standards

- [ ] `Target.targetCreated` 监听器在 session close 时正确取消
- [ ] `list_tabs` 返回结构 `[{url, title, active, idx}]`
- [ ] `switch_tab(idx)` 越界返回 `BrowserError::TabIndexOutOfRange`
- [ ] tool schema 显式声明 `tab_index` 参数

---

### G7: Frame-Aware Snapshot — iframe 不再失明

**对应代码位置**: `src-tauri/src/modules/browser/snapshot.rs:181` (`var tree = walk(document.body, 0);`)

#### Purpose

`SNAPSHOT_SCRIPT` 只 walk `document.body` — 任何 `<iframe>` 内的内容（YouTube embed、reCAPTCHA、Stripe Checkout、嵌入广告）AI 完全看不到。同源 iframe 可以同步访问 `iframe.contentDocument`，跨域 iframe 必须通过 CDP 的 `Frame Tree` API 访问。

#### Implementation Plan

**Step 1**: snapshot.js 内对同源 iframe 递归 walk

```js
// snapshot.js
function walkIframes(out, depth) {
  document.querySelectorAll('iframe').forEach(function(frame) {
    var doc = null;
    try { doc = frame.contentDocument; } catch(e) {}
    if (doc && doc.body) {
      out += '\n--- iframe (same-origin): ' + (frame.src || '<inline>') + ' ---\n';
      out += walk(doc.body, depth + 1);
    } else if (frame.src) {
      out += '\n--- iframe (cross-origin): ' + frame.src + ' [content not directly accessible] ---\n';
    }
  });
  return out;
}

// 在主 walk 完后
tree += walkIframes('', 0);
```

**Step 2**: Rust 侧对跨域 iframe 用 chromiumoxide `Frame Tree` 单独 evaluate

```rust
// session.rs
pub async fn run_snapshot(&self) -> Result<String, BrowserError> {
    let main = self.eval_snapshot_in_frame(self.active_page().main_frame_id()).await?;

    let frames = self.active_page().frame_ids().await?;
    let mut extra = String::new();
    for frame_id in frames {
        if frame_id == self.active_page().main_frame_id() { continue; }
        match self.eval_snapshot_in_frame(frame_id).await {
            Ok(t) if !t.is_empty() => {
                extra.push_str(&format!(
                    "\n--- iframe (CDP): {} ---\n{}", frame_id, t
                ));
            }
            _ => {}
        }
    }
    Ok(main + &extra)
}

async fn eval_snapshot_in_frame(&self, frame_id: FrameId) -> Result<String, BrowserError> {
    // 使用 Page.createIsolatedWorld + Runtime.evaluate(contextId=...)
    // 在指定 frame 上下文跑 SNAPSHOT_SCRIPT
}
```

**Step 3**: ref 编号在所有 frame 间全局唯一

snapshot.js 内 `ref` 改为模块级变量（Rust 侧通过 `Runtime.evaluate` 的 `executionContextId` 在不同 frame 复用同一个 contextID 的全局对象）— 简化方案：每个 frame 跑独立 SNAPSHOT_SCRIPT 但 ref 加 frame 前缀，例如 `[F1.3]` 表示 "frame #1 的第 3 个交互元素"。

```rust
// 在 snapshot 顶部
out.push_str(&format!("\n[F{}] iframe: {}\n", frame_idx, frame_url));
```

AI 用 `click` 时传 `[F1.3]` → tool 层先解析出 frame_id 再 querySelector。

#### Target Files

| File                                                  | Change                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------- |
| `src-tauri/src/modules/browser/snapshot.rs`           | snapshot.js 加 same-origin iframe walk                                  |
| `src-tauri/src/modules/browser/session.rs`            | `run_snapshot` 遍历 frame tree；`click` / `type` 解析 `F<n>.<ref>` 前缀 |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | `ref` 参数 schema 改 `string`（接 `"3"` 或 `"F1.3"`）                   |

#### Risks

- **跨域 iframe 无法 evaluate** 的合法情况（CSP 严格的 iframe）：fallback 到"显示 src 但内容不可见"提示。
- **ref 序号变得复杂**：AI 可能在主 frame 用 `F0.3` 而出错。缓解：snapshot 输出顶部明确说明"主 frame 用 [N]，iframe 用 [F<n>.<m>]"。

#### Review Standards

- [ ] same-origin iframe 内容出现在 snapshot 中
- [ ] cross-origin iframe 通过 CDP frame tree 抓取
- [ ] `[F1.3]` 形式的 ref 在 click / type 中被正确解析
- [ ] iframe 无法访问时降级为 "[content not accessible]" 而不是 panic

---

### G8: Download Manager + 简易 Upload

**对应代码位置**: `src-tauri/src/modules/browser/session.rs` (无 download 处理)

#### Purpose

真实代理任务必然涉及下载附件（"帮我下载这张发票""把这个 CSV 给我"），以及上传文件（"把我桌面这张图传到 ChatGPT"）。当前两边代码都缺。

#### Implementation Plan

**Step 1**: 启用 CDP 下载行为

```rust
// session.rs 内 new() 末尾
use chromiumoxide::cdp::browser_protocol::browser::{
    SetDownloadBehaviorParams, SetDownloadBehaviorBehavior,
};

let download_dir = if2ai_home
    .join("desk")
    .join(&session_id)
    .join("downloads");
std::fs::create_dir_all(&download_dir)?;

browser.execute(SetDownloadBehaviorParams {
    behavior: SetDownloadBehaviorBehavior::AllowAndName,
    browser_context_id: None,
    download_path: Some(download_dir.to_string_lossy().to_string()),
    events_enabled: Some(true),
}).await?;
```

**Step 2**: 监听 `Browser.downloadWillBegin` / `downloadProgress`

```rust
let downloads = Arc::new(DashMap::<String, DownloadEntry>::new());
let dl_clone = Arc::clone(&downloads);
tokio::spawn(async move {
    let mut evts = browser_handle.event_listener::<EventDownloadWillBegin>().await?;
    while let Some(e) = evts.next().await {
        dl_clone.insert(e.guid.clone(), DownloadEntry {
            guid: e.guid.clone(),
            url: e.url.clone(),
            suggested_filename: e.suggested_filename.clone(),
            state: DownloadState::InProgress,
            saved_path: None,
        });
        // emit Tauri event
    }
});
```

**Step 3**: 新增 `downloads` action

```rust
"downloads" => {
    let list = registry.list_downloads(&session_id).await;
    Ok(ToolOutput::text(format_downloads(&list)))
}
```

snapshot 末尾追加：
```
## Recent downloads (saved to ~/.if2ai/desk/<session>/downloads/):
- invoice-2026-04.pdf (243 KB) — completed
- report.csv (12 KB) — in progress (47%)
```

**Step 4**: Upload 通过 CDP `Page.handleFileChooser`

```rust
pub async fn upload_file(&mut self, ref_num: u32, file_path: &Path)
    -> Result<String, BrowserError>
{
    use chromiumoxide::cdp::browser_protocol::page::HandleFileChooserParams;

    // 触发 click 让 file chooser 弹出
    self.click(ref_num).await?;
    // 立即注入文件路径（CDP 截获 file chooser）
    self.active_page().execute(HandleFileChooserParams {
        action: HandleFileChooserAction::Accept,
        files: Some(vec![file_path.to_string_lossy().to_string()]),
    }).await?;

    self.run_snapshot().await
}
```

新增 `upload` action：

```json
{
  "action": "upload",
  "ref": 5,
  "file_path": "/Users/x/Desktop/photo.jpg"
}
```

注意：`file_path` 必须经过 `tool_context.workspace_root` 校验（防越权访问任意路径）。

**Step 5**: 前端 BrowserCard 显示下载列表 + "在 Finder 中显示" 按钮

#### Target Files

| File                                                  | Change                                                           |
| ----------------------------------------------------- | ---------------------------------------------------------------- |
| `src-tauri/src/modules/browser/session.rs`            | `SetDownloadBehavior` + `downloadWillBegin` 监听 + `upload_file` |
| `src-tauri/src/modules/browser/registry.rs`           | `list_downloads`、`upload_file` 委托                             |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | `downloads` / `upload` actions                                   |
| `src/components/browser/BrowserCard.tsx`              | Downloads 区域                                                   |

#### Risks

- **Upload 路径越权**：AI 可能传 `/etc/passwd`。缓解：必须在 `tool_context.workspace_root` 或 `~/.if2ai/desk/` 之内。
- **下载文件占盘**：缓解：每 session 单独目录，UI 加"清空下载"按钮。

#### Review Standards

- [ ] download 落在 `~/.if2ai/desk/<session>/downloads/`
- [ ] download 状态变化 emit Tauri event
- [ ] upload 路径必须通过 path canonicalize + workspace 包含性检查
- [ ] snapshot 末尾的下载列表只显示最近 10 个

---

## P2 Medium Gaps

---

### G9: Console / Network 错误注入 Snapshot

**对应代码位置**: `src-tauri/src/modules/browser/session.rs` (无 console / network event 收集)

#### Purpose

AI 调 `click` 后页面没变 → 现在 AI 没办法知道是"按钮被禁用""onClick 报 JS 错误""API 返回 401"。`SNAPSHOT_SCRIPT` 只看 DOM，console / network 事件不在 DOM 里。

#### Implementation Plan

**Step 1**: 启动时监听 `Console.messageAdded` / `Network.responseReceived`

```rust
pub struct BrowserSession {
    // ... existing
    recent_console: Arc<Mutex<RingBuffer<ConsoleEvent>>>,    // 最近 50 条
    recent_network_errors: Arc<Mutex<RingBuffer<NetEvent>>>, // 最近 30 条 4xx/5xx
}
```

**Step 2**: snapshot 输出末尾追加（仅当有错误时）

```
## Recent console errors (last 5):
[error] Uncaught TypeError: Cannot read properties of null (reading 'foo') at app.js:42
[warn] React: Each child in a list should have a unique "key" prop.

## Recent network errors (last 5):
POST https://api.example.com/cart → 401 Unauthorized (3.2 KB body, took 124ms)
GET https://cdn.example.com/icons.svg → 404 Not Found
```

**Step 3**: 新增 `console` / `network` actions（可选，主要让 AI 显式拉取详细日志）

```rust
"console" => {
    Ok(ToolOutput::text(registry.dump_console(&session_id).await))
}
```

#### Target Files

| File                                                  | Change                                 |
| ----------------------------------------------------- | -------------------------------------- |
| `src-tauri/src/modules/browser/session.rs`            | console / network 监听器 + ring buffer |
| `src-tauri/src/modules/browser/snapshot.rs`           | snapshot 输出末尾拼接                  |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | `console` / `network` actions          |

#### Risks

- **隐私**：console 可能含敏感数据（auth token in 日志）。缓解：在 BrowserCard 提供"显示/隐藏 console"开关。

#### Review Standards

- [ ] ring buffer 上限固定（50 console / 30 network），不内存泄漏
- [ ] snapshot 仅在 buffer 非空时追加 console / network 段
- [ ] `console` action 返回完整 buffer

---

### G10: web_fetch 用 `scraper` + Readability 提取主要内容

**对应代码位置**: `src-tauri/src/modules/tools/builtin/web_fetch.rs:196-247`

#### Purpose

当前 `web_fetch` selector 实现是 regex 解析 HTML（line 199-218），嵌套元素抓不全；无 selector 时直接 strip-tag 然后 split_whitespace，把整页噪音都喂给 LLM。

openhanako 的 `lib/tools/web-fetch.js:39-71` 是手写 regex+entity decode 的简化版，质量也不高。

我们可以做得更好：用 `scraper` (CSS selector) + 移植 Readability 算法。

#### Implementation Plan

**Step 1**: 新增 `readability` 子模块

```rust
// modules/tools/builtin/web_fetch/readability.rs
//! Mozilla Readability 算法的 Rust 简化移植：
//! - 评分 candidate 节点（class/id 含 article/content/main 加分；含 nav/footer/sidebar 减分）
//! - 选最高分节点的 outerHTML 作为正文
//! - 输出 markdown（含标题、段落、链接、图片 alt、列表）

pub fn extract_article(html: &str, base_url: &Url) -> ExtractedArticle {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    // 1. strip script/style/iframe/nav/footer/aside
    // 2. score 候选节点
    // 3. 选最佳容器
    // 4. 转 markdown：<h1-h6> → #*N、<p> → 段落、<a> → [text](url)、<img> → ![alt](src)
    // 5. 标题：<h1> 或 <title>
    // 6. 摘要：前 200 字符
    // 7. 估算 reading time
    // ...
}

pub struct ExtractedArticle {
    pub title: String,
    pub byline: Option<String>,
    pub markdown: String,
    pub excerpt: String,
    pub estimated_reading_time_min: u32,
}
```

**Step 2**: web_fetch 新参数 `mode`

```json
{
  "url": "...",
  "mode": "auto" | "text" | "article" | "selector",
  "selector": "...",
  "max_length": 12000
}
```

- `auto`（默认）：HTML → readability article，markdown 输出；非 HTML 走原 text 路径
- `article`：强制 readability
- `text`：原全文 strip
- `selector`：用 `scraper` 真正的 CSS selector，不再 regex

**Step 3**: scraper 实现 selector 模式

```rust
let document = Html::parse_document(&html);
let selector = Selector::parse(sel)
    .map_err(|e| ToolError::Handler(format!("invalid CSS selector: {e:?}")))?;
let mut out = Vec::new();
for el in document.select(&selector).take(50) {
    out.push(el.text().collect::<Vec<_>>().join(" ").trim().to_owned());
}
```

**Step 4**: Cargo.toml

```toml
scraper = "0.20"
# 不再需要新 crate；readability 自实现避免引入维护活跃度低的 crate
```

#### Target Files

| File                                                           | Change                 |
| -------------------------------------------------------------- | ---------------------- |
| `src-tauri/src/modules/tools/builtin/web_fetch/mod.rs`         | 拆分原文件为 mod       |
| `src-tauri/src/modules/tools/builtin/web_fetch/readability.rs` | **NEW**                |
| `src-tauri/src/modules/tools/builtin/web_fetch/extract.rs`     | scraper-based selector |
| `src-tauri/Cargo.toml`                                         | + `scraper = "0.20"`   |

#### Risks

- **Readability 算法不完美**：复杂 SPA 页面（Twitter）效果差。缓解：fallback 到 raw text；提示 LLM "若内容不全请试 browser tool"。
- **scraper 编译时长**：`scraper` 依赖 `html5ever`，首次编译 ~30s。可接受。

#### Review Standards

- [ ] `mode=article` 在 wikipedia.org 上输出干净 markdown
- [ ] `mode=selector` 接受标准 CSS selector（含 `:nth-child`、属性选择器）
- [ ] `mode=auto` 在 JSON API 上自动跳过 readability
- [ ] readability 输出 token 数 ≤ raw text 的 50%

---

### G11: `web_research` 复合工具 — 一次调用完成"查资料"

**对应代码位置**: 新工具
**与 G4 协同**：当 LLM 需要"调研一个话题"时，prompt routing 优先指向 `web_research`

#### Purpose

"查资料"任务的典型 LLM 调用序列：
1. `web_search(query)` → 收到 5-10 个 link
2. 选 top 3
3. `web_fetch(url1)` × 3
4. 自己拼摘要

→ 4 轮 tool call，每轮一次完整 LLM round-trip。`web_research` 把这串变成 1 次：

```json
{
  "name": "web_research",
  "description": "Search → pick top-k → fetch in parallel → return synthesized markdown.",
  "input": {
    "query": "What's new in Tauri 2.0?",
    "max_pages": 3,
    "language_hint": "en"
  }
}
```

#### Implementation Plan

**Step 1**: `web_research.rs` 新工具

```rust
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args, ctx| Box::pin(async move {
        let query = args["query"].as_str().ok_or(...)?;
        let max_pages = args.get("max_pages").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

        // Step A: 调 web_search 内部函数（不走 tool dispatch，避免循环）
        let search_results = crate::modules::tools::builtin::web_search::search_internal(
            query, max_pages * 2
        ).await?;

        // Step B: 并发 fetch top max_pages URLs
        let urls: Vec<_> = search_results.iter().take(max_pages).map(|r| r.url.clone()).collect();
        let fetches = urls.iter().map(|url| {
            crate::modules::tools::builtin::web_fetch::fetch_internal(
                url, FetchMode::Article, 8000
            )
        });
        let articles = futures::future::join_all(fetches).await;

        // Step C: 拼接 markdown
        let mut out = String::new();
        out.push_str(&format!("# Research: {query}\n\n"));
        out.push_str(&format!("## Sources ({} fetched)\n", articles.len()));
        for (i, (url, article)) in urls.iter().zip(articles.iter()).enumerate() {
            match article {
                Ok(a) => {
                    out.push_str(&format!("\n### [{i}] {} — {url}\n\n{}\n",
                        a.title, a.markdown.chars().take(2000).collect::<String>()));
                }
                Err(e) => {
                    out.push_str(&format!("\n### [{i}] {url} — FETCH FAILED: {e}\n"));
                }
            }
        }

        Ok(ToolOutput::text(out))
    }));

    ToolEntry {
        name: "web_research".into(),
        toolset: "web".into(),
        description: "Cheaper alternative to manual web_search + web_fetch chains. \
                      Use when you need to research a topic and synthesize info from \
                      multiple pages. Returns markdown with all sources included.".into(),
        // ...
        max_text_bytes: Some(48 * 1024),
        timeout_secs: Some(45),
    }
}
```

**Step 2**: 把 `search_*` / `fetch_*` 内部逻辑抽出为 `pub(crate) fn ... async`，让 `web_research` 直接调（避免重复构造 reqwest client）。

**Step 3**: G4 routing block 增加 `web_research` 推荐：

```
4. **web_research** — Compound: when researching a TOPIC (not a single fact).
    One call replaces 1 web_search + N web_fetch chains.
```

#### Target Files

| File                                                   | Change                                     |
| ------------------------------------------------------ | ------------------------------------------ |
| `src-tauri/src/modules/tools/builtin/web_research.rs`  | **NEW**                                    |
| `src-tauri/src/modules/tools/builtin/web_search.rs`    | 抽 `pub(crate) async fn search_internal()` |
| `src-tauri/src/modules/tools/builtin/web_fetch/mod.rs` | 抽 `pub(crate) async fn fetch_internal()`  |
| `src-tauri/src/modules/tools/mod.rs`                   | `register_builtin_tools` 注册              |
| `src-tauri/src/modules/runtime/prompt_tools_guide.rs`  | routing 块加 web_research 条               |

#### Risks

- **失败级联**：3 个 fetch 中 1 个超时拖慢整个 tool。缓解：每个 fetch 独立 timeout 10s，超时返回 "FETCH FAILED" 占位，不阻塞其他。
- **结果尺寸**：3×2000 字符 = 6000 字符 ≈ 8KB，安全。

#### Review Standards

- [ ] `web_research` 注册成功
- [ ] 单测：mock provider，3 个 mock 页面，并发 fetch 总耗时 ≈ max(单页耗时)
- [ ] 一个失败不影响另两个返回

---

### G12: Quality 小坑集合（5 个独立小修复）

#### Purpose

清理审计中发现的所有非阻塞但累计影响可观测性 / 稳定性的小问题。

#### Implementation Plan

| #   | 文件                                                 | 问题                                                                  | 修复                                                                                   |
| --- | ---------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| 12a | `session.rs:552 evaluate`                            | 表达式无长度上限，恶意 / 失控可塞任意大脚本                           | 限制 `expression.len() <= 10000`，超限返回 `BrowserError::EvaluateTooLong`             |
| 12b | `registry.rs:317 current_url`                        | 在 async 路径用 `try_lock`，session 忙时永远返回 `None`               | 改为 `arc.lock().await`（既然函数已经 sync 调用就在 emit 路径，不在 hot loop）         |
| 12c | `registry.rs:332 get_all_status`                     | 同上，`r.try_lock()` 在导航中 session 上拿不到 url                    | 改为单独缓存 `last_known_url: ArcSwap<Option<String>>` 在每次 navigate 后更新          |
| 12d | `session.rs:79 action_log`                           | 字段 `pub(crate)`，但 `take_action_log` 只在 stop 时取 → 无法实时观测 | 新增 Tauri 命令 `get_browser_action_log(session_id)` 与 BrowserCard "操作日志"抽屉     |
| 12e | `browser_tool.rs:218 max_result_size: Some(64*1024)` | 与 G2 多模态拆分冲突                                                  | 在 G2 落地后改为 `max_text_bytes: Some(64*1024)`、`max_image_bytes: Some(5*1024*1024)` |

每条修复都是 < 30 行变更，集中到一个 slice 完成。

#### Target Files

| File                                                  | Change                            |
| ----------------------------------------------------- | --------------------------------- |
| `src-tauri/src/modules/browser/session.rs`            | 12a evaluate 长度上限             |
| `src-tauri/src/modules/browser/registry.rs`           | 12b/12c 锁与 url 缓存             |
| `src-tauri/src/commands/browser.rs`                   | 12d `get_browser_action_log` 命令 |
| `src-tauri/src/modules/tools/builtin/browser_tool.rs` | 12e max_result_size 拆分          |
| `src-tauri/gen/schemas/capabilities.json`             | 注册 12d 新命令                   |

#### Review Standards

- [ ] 12a：evaluate 收到 11000 字符表达式时返回结构化错误，不 panic
- [ ] 12b/12c：`current_url` 在 navigate 进行中也能返回上一次的 url
- [ ] 12d：BrowserCard 抽屉显示完整 action_log 时间线
- [ ] 12e：G2 落地后 screenshot 不会因 max_text_bytes 被截断

---

## Cross-Cutting Concerns

### Tool Schema 升级一致性

12 个 gap 中 G2/G3/G5/G6/G7/G8/G9/G11 都涉及 `browser` 工具 schema 变更。最终的 `browser` tool action 列表：

| Action                                                                                                   | 既有 | 7B 后           |
| -------------------------------------------------------------------------------------------------------- | ---- | --------------- |
| start / stop / navigate / snapshot / screenshot / click / type / scroll / select / key / wait / evaluate | ✅    | ✅               |
| `tabs` / `switch_tab` / `close_tab` (G6)                                                                 | ❌    | ✅               |
| `downloads` / `upload` (G8)                                                                              | ❌    | ✅               |
| `console` / `network` (G9)                                                                               | ❌    | ✅               |
| `takeover_request` / `takeover_release` (G3，可选 — 优先走 Tauri 命令而非 tool)                          | ❌    | ⚠️ 走 Tauri 命令 |

最终 schema 17 actions，需要在 `browser_tool.rs` description 中分组列出（避免 LLM 看到一坨 enum 选错）：

```
Lifecycle: start, stop, navigate
Perception: snapshot, screenshot, console, network
Interaction: click, type, scroll, select, key, wait
Tabs: tabs, switch_tab, close_tab
Files: downloads, upload
Advanced: evaluate
```

### 测试基线

每个 P0 / P1 slice 必须新增至少 1 个集成测试（chromium 真启动）。CI 上用 `harness/suites/browser_*.yaml` 覆盖：

- `browser_login_persistence.yaml`：A 登录 → close → relaunch → 仍登录
- `browser_vision.yaml`：截图返回 ImageContent；mock provider 验证 `image_url` 正确生成
- `browser_takeover.yaml`：headless → request takeover → headed window opens → release → headless
- `browser_multitab.yaml`：navigate google.com → click result with target=_blank → tabs action 见到 2 个 tab → switch_tab 1
- `browser_iframe.yaml`：访问含 iframe 的测试页 → snapshot 含 iframe 内容
- `browser_download.yaml`：navigate 一个 .csv URL → 文件出现在 desk/<session>/downloads

### 兼容性策略

| 既有调用方                                                | 兼容方案                                                             |
| --------------------------------------------------------- | -------------------------------------------------------------------- |
| 旧 `BrowserSession::new(session_id)` 1 参形式             | 加 `BrowserSession::new_for_test(id)` 包装，使用 `Ephemeral` profile |
| 旧 `BrowserRegistry::new(path)` 1 参形式                  | 加 `BrowserRegistry::for_test(path)` 包装                            |
| 旧测试断言 screenshot 返回 `"data:image/jpeg;base64,..."` | 迁移到 `ToolOutput::parts` 断言                                      |
| 旧 `ToolHandler` 返回 `Result<String, ToolError>`         | `From<F>` impl 自动 wrap → `ToolOutput::text(s)`                     |
| 旧 `wait(timeout, "domcontentloaded")` 字符串 state       | `WaitState::from_str` 兼容                                           |

---

## Implementation Sequence

13 个 slice 的依赖关系：

```
G1 (profile)             ──┐
G2 (vision)              ──┤── 7C.1-7C.4 P0 并行可启动
G3 (headed)  → 依赖 G1   ──┤
G4 (routing)             ──┘
                            │
                            ▼ Human Checkpoint #1: 7C.4 完成 → 人工验证
                              "AI 真能用 browser 完成登录任务"
                            │
G5 (wait)                ──┐
G6 (tabs)    → 依赖 G5   ──┤── 7C.5-7C.8 P1
G7 (frames)              ──┤
G8 (downloads)           ──┘
                            │
                            ▼ Human Checkpoint #2: 7C.8 完成
                            │
G9 (console)             ──┐
G10 (web_fetch)          ──┤── 7C.9-7C.13 P2
G11 (web_research) → G10 ──┤
G12 (quality)            ──┤
7C.13 docs + harness     ──┘
```

总预计：~25-30 工程日。

---

## Decision Log

### 决策 1：保留 chromiumoxide，不切换 Tauri WebView

- **背景**：openhanako 的"用户可见 + 可接管"是用 Electron WebContentsView 实现的，Tauri 2 的 webview 看起来相似
- **选项**：A) 保留 chromiumoxide（headless + 按需 headed） B) 切换 Tauri webview C) 双栈并存
- **决定**：A
- **原因**：Tauri webview 缺 `executeJavaScript` 返回值、缺 `wait_for_navigation`、缺 CDP，AI 自动化能力会损失 90%；chromiumoxide 加 `with_head()` 后已经能做到"用户可见"
- **日期**：2026-04-18

### 决策 2：Profile 默认 PerSessionPersistent，不是 Shared

- **背景**：openhanako 是单 profile 全用户共享（Hanako 是个人助手）；我们的 if2Ai 一个 app 内可能有多个 chat session
- **选项**：A) PerSessionPersistent B) Shared C) 让用户首次启动选
- **决定**：A 默认，UI 提供 toggle 切到 B
- **原因**：A 更安全（session 之间不串扰）；B 更省心但有隐私风险
- **日期**：2026-04-18

### 决策 3：vision pipeline 通过扩展 ToolHandler 类型，不通过新工具

- **背景**：可以选择"加一个 `browser_vision` 工具单独返回 image" 或者"全面升级 ToolHandler 支持 multimodal"
- **选项**：A) 升级 ToolHandler B) 新工具
- **决定**：A
- **原因**：未来肯定有更多工具需要返回图（图表生成、文件预览、视频帧）— 一次升级一劳永逸
- **日期**：2026-04-18

### 决策 4：移除"独立 native WebView mirror"

- **背景**：现有 `BrowserViewerPage` 用 Tauri webview 假装显示 AI 浏览器，是技术幻觉
- **选项**：A) 完全移除，只显示 thumbnail B) 保留作为"快速预览"，加明显标识 C) 保留并努力同步 cookies
- **决定**：A，但保留 `navigate_viewer_window` 命令向后兼容（hidden by default）
- **原因**：C 几乎做不到（Tauri webview 用 system webview，跟 chromiumoxide 的 Chromium 不可能共享 cookies）；B 会持续误导用户
- **日期**：2026-04-18

---

## Risks & Mitigations (Aggregate)

| 风险                                              | 可能性 | 影响 | 缓解                                                               |
| ------------------------------------------------- | ------ | ---- | ------------------------------------------------------------------ |
| Profile 持久化导致磁盘失控                        | M      | M    | 软上限 + LRU 清理                                                  |
| Vision pipeline 把 token 成本拉高（image 也要钱） | H      | M    | LLM 主动选择，非每次 navigate 都截图；prompt 中明确说明 image cost |
| Headed 模式在 CI 失败                             | H      | L    | `headed=false` 默认；CI env detection 自动 fallback                |
| Multi-tab 让 ref 编号混乱误操作                   | M      | M    | snapshot 顶部明确说明 `[F<n>.<m>]` 语法；集成测试覆盖              |
| Readability 算法对部分站点效果差                  | H      | L    | mode=auto 失败 fallback 到 text；prompt 提示可重试 browser         |
| Tool schema 一次性增加 5+ actions LLM 学不会      | L      | M    | description 内分组列出；提供使用示例                               |

---

## Milestones

- ⏳ 7C.1-7C.4 (P0) — 2026-05-02
- ⏳ Human Checkpoint #1 — 验证 "AI 用 browser 真完成登录任务"
- ⏳ 7C.5-7C.8 (P1) — 2026-05-12
- ⏳ Human Checkpoint #2 — 验证 "frame / multi-tab / download 完整链路"
- ⏳ 7C.9-7C.12 (P2) — 2026-05-22
- ⏳ 7C.13 文档 + harness suite — 2026-05-26
- ⏳ Phase 7C Final Sign-off — 2026-05-30

---

## Related Documents

- [browser-control-system.md](../browser-control-system.md) — Phase 7B 原始设计
- [phase-7b-browser-control-system.yaml](../../../exec-plans/active/phase-7b-browser-control-system.yaml) — 已完成的 Phase 7B
- [phase-7c-browser-subsystem-remediation.yaml](../../../exec-plans/active/phase-7c-browser-subsystem-remediation.yaml) — 本 ADR 对应的 exec-plan（13 slices）
- [openhanako-main 源码参考](file:///Users/ryanliu/Downloads/openhanako-main/) — `lib/browser/` + `desktop/main.cjs:990-1550` + `lib/tools/browser-tool.js`

---

**版本**: 1.0 | **最后更新**: 2026-04-18 | **状态**: Proposed

# 七、MCP / 插件系统偏差

> 本章节对比 MCP (Model Context Protocol) 和插件系统在 CLAW-CLI Baseline 和 If2Ai 桌面端的实现差异。

---

## 7.1 MCP 协议支持对比

### CLAW-CLI Baseline

```rust
// runtime/mcp_stdio.rs
pub struct McpServerManager {
    servers: BTreeMap<String, ManagedMcpServer>,     // Stdio 服务器进程
    unsupported_servers: Vec<UnsupportedMcpServer>, // HTTP/WS 等
    tool_index: BTreeMap<String, ToolRoute>,         // qualified_name → route
}

impl McpServerManager {
    pub async fn discover_tools(&self) { ... }
    pub async fn call_tool(&self, qualified_name: &str, params: Value) -> Value { ... }
}

// 工具名称格式: mcp__<server>__<tool>
// 示例: mcp__claude_ai_Example_Server__weather_tool
```

**功能**：
- Stdio 传输的 MCP 服务器生命周期管理
- 按需启动（lazy spawn）
- 工具发现（`tools/list` 分页）
- JSON-RPC over Content-Length 帧协议
- 进程复用（discovery 和 call 之间保持运行）

**不支持**：HTTP/SSE/WebSocket 传输的 MCP 服务器（标记为 unsupported）

### If2Ai 桌面端

**完全缺失**：无 MCP 相关模块。

`modules/tools/builtin/` 中的所有工具都是内置的，没有 MCP 工具发现或调用能力。

---

## 7.2 插件系统对比

### CLAW-CLI Baseline

```rust
// plugins/src/lib.rs
pub trait Plugin {
    fn metadata(&self) -> &PluginMetadata
    fn hooks(&self) -> &PluginHooks
    fn lifecycle(&self) -> &PluginLifecycle
    fn tools(&self) -> &[PluginTool]
    fn validate(&self) -> Result<(), PluginError>
    fn initialize(&self) -> Result<(), PluginError>
    fn shutdown(&self) -> Result<(), PluginError>
}

pub struct PluginManager {
    plugins: Vec<RegisteredPlugin>,
}

impl PluginManager {
    pub fn discover_plugins(&self) -> Result<(), PluginError>
    pub fn aggregated_tools(&self) -> Result<Vec<PluginTool>, PluginError>
    pub fn aggregated_hooks(&self) -> Result<PluginHooks, PluginError>
}
```

**插件发现机制**：
1. `sync_bundled_plugins()` — 同步 bundled 插件
2. `builtin_plugins()` — 内置脚手架插件
3. `discover_installed_plugins()` — 从安装目录扫描
4. `discover_external_directory_plugins()` — 从外部目录扫描

### If2Ai 桌面端

**完全缺失**：无插件系统。

`register_builtin_tools()` 是硬编码注册，无动态加载能力。

---

## 7.3 Hook 系统对比

### CLAW-CLI Baseline（两套实现）

**runtime 内置 Hook**（`runtime/hooks.rs`）：
```rust
pub enum HookEvent { PreToolUse, PostToolUse }
pub struct HookRunResult { denied: bool, messages: Vec<String> }
```

**插件 Hook**（`plugins/hooks.rs`）：
- 与 runtime hooks 模型一致
- 通过 `HookRunner::from_registry()` 聚合多个插件的钩子

### If2Ai 桌面端

```rust
// modules/runtime/hooks.rs — HookRunner 存在
pub struct HookRunner { ... }
impl HookRunner {
    pub fn run_pre_tool_use(&mut self, tool_name: &str, input: &str) -> HookRunResult { ... }
    pub fn run_post_tool_use(&mut self, tool_name: &str, input: &str, output: &str, is_error: bool) -> HookRunResult { ... }
}
```

**但未接入**：
- `run_agent_turn` 创建了 `HookRunner`（通过 `RuntimeFeatureConfig`）
- 但 `ConversationRuntime::run_turn()` 没有实际调用 Hook
- 工具执行时 Hook 系统从未触发

---

## 7.4 OAuth 登录对比

### CLAW-CLI Baseline

```rust
// runtime/oauth.rs
pub async fn load_oauth_credentials(config: &OAuthConfig) -> Result<OAuthTokenSet, Error>
pub async fn refresh_oauth_token(config: &OAuthConfig) -> Result<OAuthTokenSet, Error>
```

支持 OAuth token 获取和自动刷新。

### If2Ai 桌面端

**完全缺失**。API 认证目前仅支持：
- Bearer Token（从 `~/.claude/settings.json` 读取）
- API Key

无 OAuth 登录 UI 或 token 刷新机制。

---

## 7.5 小结

| 问题 | 严重度 |
|------|--------|
| MCP 协议完全缺失 | 🟢 低（Phase 5+） |
| 插件系统完全缺失 | 🟢 低（Phase 5+） |
| Hook 系统未接入 | 🟡 中 |
| OAuth 登录 UI 缺失 | 🟢 低 |

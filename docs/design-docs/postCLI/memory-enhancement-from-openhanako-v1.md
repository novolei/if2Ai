# Memory Enhancement Plan — Adopting Openhanako Pipeline (v2, Calibrated)

> **版本**：v2 · 2026-04-18（基于 v1，按 system-architect + UI/UX designer 角色对 openhanako 与本仓 codebase 做颗粒度核对后重写）
> **变更摘要**：保留 v1 的 Sprint × Task 结构与产品意图，**修正 30 个与现状不一致的接口/路径/依赖**（详见 §0.5 Calibration Δ）。所有“伪码示例”改为可直接落地的真实模块/类型签名；新增 **§0.5、§0.6、§0.7** 三节，分别说明 v2 的接口对齐、依赖增量、跨模块约束。
> **关联文档**：
> - `docs/design-docs/postCLI/memory-control-plane-v1.md`（既有 Memory Control Plane）
> - `docs/design-docs/postCLI/ADR/ADR-002-Active-Retrieval-vs-Passive-Invocation.md`
> - `docs/design-docs/postCLI/ADR/ADR-004-Token-Budget-Allocation.md`
> - `docs/design-docs/postCLI/ADR/if2Ai-Memory-Autonomous-Learning-Architecture-Report.md`
> - 参考实现：`/Users/ryanliu/Downloads/openhanako-main/lib/memory/`
> - **本仓真实接口（已核对）**：
>   - `src-tauri/src/modules/memory/mod.rs` — `MemoryProvider` trait（已含 `clear_all` / `promote_scope` / `demote_scope` / `apply_importance_decay`）
>   - `src-tauri/src/modules/memory/audit.rs` — `MemoryAuditEmitter`（**unit 类型，纯关联函数**）+ `AuditContext<'a>` + `MemoryEventPayload`（**字段固定**，需扩展策略）
>   - `src-tauri/src/modules/memory/security.rs` — `ThreatScanner`（已有 8 类 pattern，**只标记不脱敏**）
>   - `src-tauri/src/modules/runtime/conversation.rs` — `ApiClient` trait（runtime 内部）
>   - `src-tauri/src/modules/api/providers/{mod.rs,manager.rs}` — `Provider` trait + `ProviderManager`
>   - `src-tauri/src/modules/runtime/prompt.rs` — `SystemPromptBuilder`（builder 模式，`build()` **同步**）
>   - `src-tauri/src/modules/runtime/config.rs` — `MemoryFeatureConfig`（**`f64` 字段不能从 settings.json 走 `JsonValue` 反序列化**，必须经 `~/.if2ai/memory_config.json` 的 `serde_json` 路径）
>   - `src-tauri/src/main.rs::create_memory_provider` — 数据落盘根目录是 `dirs::data_local_dir().join(".if2ai/memory")`，**不是** `~/.if2ai/memory`
>   - `src-tauri/src/main.rs::invoke_handler!` + `gen/schemas/capabilities.json` — 每个新 Tauri command **必须双注册**

---

## 0. 文档目的

本文将《openhanako vs if2Ai 记忆模块差异分析》中的 **18 项 Gap（G1-G18）** 落到**可执行的 Sprint × Task 规格**，每个 Task 给出：
- 文件路径（新增 / 修改）
- 数据结构 / SQL / API 签名（具体字段）
- 实现步骤（伪码或要点列表，可直接转为 slice）
- 验收标准（可被 harness gate 检查）
- 前置依赖

**不是**重写整个 Memory Control Plane；而是在既有的"三层 scope + 向量检索 + promotion engine + audit emitter"之上，**正交叠加** openhanako 的"LLM 编译流水线 + 静态注入 + 写入硬约束"。两套系统各自负责的事：

| 既有体系（保留）                                     | 新增体系（引入）                              |
| ---------------------------------------------------- | --------------------------------------------- |
| 向量检索（FastEmbed + LanceDB）+ HRR                 | 滚动摘要持久化（`session-summary.js` 模型）   |
| Importance / access_count / Weibull decay            | LLM 编译流水线（today/week/longterm/facts）   |
| 三层 scope（session/project/global）+ promote/demote | 元事实 + Tag + FTS5 检索                      |
| Policy enforce mode（allow/deny/prompt）             | Pinned 静态注入 system prompt                 |
| Promotion engine + audit emitter                     | Experience extractor + Diary writer           |
| Intent classifier + retrieval planner                | PII 写入侧硬规则 + 逻辑日 + Turn-based ticker |

---

---

## 0.5 Calibration Δ（v1 → v2 全量修正清单）

> **使用方式**：每条 Δ 都对应 v1 文档中一处或多处“伪码示例”。**实现 slice 时以本节为最高优先级权威**；下文 Sprint 章节内的伪码若与本节冲突，**以本节为准**（v1 伪码保留是为了人读的连贯性，未逐字回写）。

### Δ-1 LLM 调用层：不存在 `LlmClient` trait，需要新增 utility shim

| 项                   | v1 写的  | 现状                                                                                                                        | v2 修正                                                                                                                                                                                                                                                                                                               |
| -------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Arc<dyn LlmClient>` | 多处使用 | **不存在**。运行时只有 `runtime::conversation::ApiClient`（流式 + 工具）和 `api::providers::Provider`（请求级 send/stream） | 新增 `src-tauri/src/modules/memory/llm.rs`：定义 **`UtilityLlm` trait** 作为“一次性 system+user → text”的薄壳，背后默认实现 `ProviderUtilityLlm` 调用 `ProviderManager::create_message_default` 构造无工具的 `MessageRequest`。所有 memory 子模块（rolling / compiler / extractor / diary）依赖 `Arc<dyn UtilityLlm>` |

```rust
// src-tauri/src/modules/memory/llm.rs（新增）
#[async_trait]
pub trait UtilityLlm: Send + Sync {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, MemoryError>;
}

pub struct ProviderUtilityLlm {
    manager: Arc<api::providers::manager::ProviderManager>,
    /// 默认 utility 模型（settings 里 `memory.utility_model`，回落到 default provider）
    model: Option<String>,
}
```

> **理由**：保持 `ConversationRuntime` 不被 memory 子系统污染；`ProviderManager` 的 `Provider` trait 已具备 send/stream 能力，包一层即可。

---

### Δ-2 PII 脱敏：扩展 `ThreatScanner`，不要新建 `scrub_for_storage`

| 项                                              | v1 写的               | 现状                                                                                                                              | v2 修正                                                                                                                                                                                                                                                       |
| ----------------------------------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pub fn scrub_for_storage(text) -> ScrubResult` | 在 `security.rs` 新增 | `security::ThreatScanner` 已有 8 类 pattern（`api_key×4 + private_key + password + jwt + xss`），**只返回首条命中报告，不做替换** | **扩展现有 ThreatScanner**：(a) 追加 3 类 pattern (`credit_card`, `id_card_cn`, `ssn_us`)；(b) 新增方法 `scan_and_redact(&self, key: &str, content: &str) -> ScrubResult`，返回所有命中区间 + 已替换字符串；(c) 既有 `scan()` 保持向后兼容（policy 路径仍用） |

```rust
// security.rs 增量
impl ThreatScanner {
    pub fn scan_and_redact(&self, key: &str, content: &str) -> ScrubResult { /* 多次扫描 + 区间替换为 [REDACTED:<category>] */ }
}

pub struct ScrubResult {
    pub cleaned: String,
    pub detected: Vec<DetectedPii>,   // 命中类别 + 摘要片段（前 8 字 + 后 4 字）
    pub flagged: bool,
}
```

> **写入路径接入顺序**（v1 §T-A1 验收标准不变，但调用方式改为 `scanner.scan_and_redact(...)`）：`commands::memory::memory_store` → `pinned::store::add` → `summary::rolling::save` → `facts::store::add` → `experience::store::add` → `diary::writer::write_diary`。

---

### Δ-3 `MemoryAuditEmitter` 是 unit 类型，**所有调用都是关联函数**

| 项                                                                       | v1 写的  | 现状                                                                                                                                                             | v2 修正                                                                                                                                                                                           |
| ------------------------------------------------------------------------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `audit: Arc<MemoryAuditEmitter>` 字段、`self.audit.memory_compiled(...)` | 多处出现 | `MemoryAuditEmitter` 是 `pub struct MemoryAuditEmitter;`，所有方法是 `pub fn xxx(ctx: &AuditContext<'_>, ...)`。事件分发用全局 `OnceLock<AppHandle>` + `tracing` | **删除所有 `Arc<MemoryAuditEmitter>` 字段**，改为 `MemoryAuditEmitter::memory_xxx(&AuditContext::from_scope(scope), ...)` 直接调用。`AuditContext` 用 `from_scope` / `from_scope_with_trace` 构造 |

---

### Δ-4 `MemoryEventPayload` 字段是固定结构 — 必须扩展为可携带可变 metadata

| 项                                     | v1 写的                     | 现状                                                                                                                                             | v2 修正                                                                                                                                                                                                                                                                                                                                          |
| -------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 11 个新事件直接 emit，每个有自定义字段 | 隐含假设 payload 可任意扩展 | `MemoryEventPayload` 字段是固定枚举（约 14 个 Option<&str>/Option<usize>），新事件类型不能塞 `detected: Vec<...>` / `recovered: Vec<...>` 等结构 | **新增 `extra: Option<serde_json::Value>` 字段**到 `MemoryEventPayload`（向后兼容：默认 `None`，TS 端用 `extra?: Record<string, unknown>`）。复杂 metadata 走 `extra`，简单 string 字段沿用现有 Option。同时把 `event` 字段类型从 `&'static str` 升级为 `&'static str`（仍保持），并在 TS 侧 `MemoryEventPayload.event` 字面量联合追加 11 个新值 |

```rust
// audit.rs 增量
struct MemoryEventPayload<'a> {
    event: &'static str,
    // ... 既有字段保留 ...
    /// New in v2: 任意结构化 metadata（用于新事件 detected/recovered/sections 等数组类字段）
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
    timestamp: String,
}
```

> 配套：`src/lib/tauri.ts` 的 `MemoryEventPayload` interface 同步追加 `extra?: Record<string, unknown>` + 11 新枚举值。

---

### Δ-5 `MemoryProvider` trait 已具备的方法不需要重写

| 已存在（v1 当作 TODO）        | 真实状态                                                    |
| ----------------------------- | ----------------------------------------------------------- |
| `clear_all`                   | ✅ trait 默认实现 + SQLite/Vector 已 override                |
| `promote_scope`               | ✅ trait 默认 Err，SQLite/Vector 已 override                 |
| `demote_scope`                | ✅ trait 默认 delegate 到 promote，SQLite/Vector 已 override |
| `apply_importance_decay`      | ✅ trait 默认 0，等 Phase H 启用                             |
| `entry_matches_scope_default` | ✅ helper 已存在                                             |

> v2 不重新定义这些；新增的 `SessionSummaryStore` / `FactStore` / `PinnedStore` / `ExperienceStore` / `DiaryStore` 走 **独立的 trait**（不挂在 `MemoryProvider` 上），保持核心 trait 单一职责。

---

### Δ-6 数据落盘根目录：`dirs::data_local_dir()/.if2ai/memory/`，**不是** `~/.if2ai/memory/`

v1 §2 写的路径要全部理解为“相对 `dirs::data_local_dir()`”。macOS = `~/Library/Application Support/.if2ai/memory/`，Linux = `~/.local/share/.if2ai/memory/`。配置文件 `~/.if2ai/memory_config.json` 的 home-dir 路径是另一回事，仅用于配置覆盖（看 Δ-9）。

```text
<data_local>/.if2ai/memory/                 ← 用户级（global scope）
  ├─ memory.db                              ← SQLite，新增 facts/summaries/pinned_items/experience 表
  ├─ vector_db/                             ← LanceDB（既有）
  ├─ jobs.db                                ← S1 新增（job_attempts 表）
  └─ global/                                ← 用户级 markdown 镜像
     ├─ pinned.md / memory.md
     ├─ today.md / week.md / longterm.md / facts.md
     ├─ *.fingerprint                       ← S2 指纹边车
     └─ summaries/<session_id>.json         ← S1 滚动摘要

<workspace>/.if2ai/memory/                  ← 项目级（project scope）
  ├─ pinned.md / memory.md
  ├─ today.md / week.md / longterm.md / facts.md
  ├─ summaries/<session_id>.json
  ├─ experience/<category>.md / index.md
  └─ diary/YYYY-MM-DD.md
```

---

### Δ-7 `SystemPromptBuilder.build()` 是同步的；memory 注入需要预拉

| 项                                                                                                                           | v1 写的        | 现状                                                                                 | v2 修正                                                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `if config.memory.inject_to_prompt { let injection = build_memory_injection(...).await?; }` 在 `assemble_system_prompt` 内调 | 假设可以 await | `SystemPromptBuilder::build()` 返回 `Vec<String>`，**完全同步**（被 sync code 调用） | T-F4 调整为：(a) 调用方（`commands::agent::run_agent_turn` 或 `runtime::conversation` 启动前）先 `let injection = build_memory_injection(...).await?`；(b) 把 injection 的 markdown 段通过新增 `SystemPromptBuilder::with_memory_injection(injection: MemoryInjection)` 传入；(c) `build()` 中按 `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` 之后追加 |

```rust
// prompt.rs 增量
impl SystemPromptBuilder {
    pub fn with_memory_injection(mut self, injection: MemoryInjection) -> Self {
        self.memory_injection = Some(injection); self
    }
}

// build() 内：在 dynamic boundary 之后、append_sections 之前追加 pinned + compiled + rules
```

---

### Δ-8 `ApiClient` 还是 `Provider`？— Ticker 接入点用 `ConversationRuntime` hook，不要侵入 `commands::agent`

| 项                                                             | v1 写的                                                                                                                                                                                 | v2 修正 |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `ticker.notify_turn` 在 `commands/agent.rs::run_agent_turn` 调 | **改为**：在 `ConversationRuntime` 内部加 `on_turn_complete: Option<Arc<dyn TurnHook>>` 钩子，由 `AppState` 在构造 runtime 时注入 ticker 实现。`commands::agent` 不直接知道 ticker 存在 |

```rust
// runtime/conversation.rs 增量
pub trait TurnHook: Send + Sync {
    fn on_turn_complete(&self, scope: &MemoryExecutionScope, session_id: &str);
    fn on_session_end(&self, scope: &MemoryExecutionScope, session_id: &str);
}

impl<C: ApiClient> ConversationRuntime<C> {
    pub fn with_turn_hook(mut self, hook: Arc<dyn TurnHook>) -> Self { /* ... */ }
}
```

> **理由**：保持 commands/ 层薄；任何用 `ConversationRuntime` 的入口（chat / harness / future API）都自动获得 ticker。

---

### Δ-9 `f64` 不能进 `JsonValue` — 编译/经验/日记 thresholds 的配置全部走 `memory_config.json`

| 项                                                                        | v1 写的                   | 现状                                                                                                                                                                                         | v2 修正                                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `MemoryFeatureConfig` 增 `compiler / pii_scrub / max_inject_tokens / ...` | 隐含从 settings.json 加载 | `MemoryFeatureConfig` 的 `promotion: PromotionThresholds` 已经因为 `f64` 字段被显式排除出 settings.json 解析路径（见 `runtime/config.rs:1174`），仅从 `~/.if2ai/memory_config.json` 反序列化 | **所有新增的浮点字段 / 复杂结构（CompilerConfig / ExperienceConfig / DiaryConfig）必须在 `MemoryConfigOverrides` 中追加 `Option<...>`，并在 `~/.if2ai/memory_config.json` 路径反序列化**。settings.json 的 `JsonValue` 路径只放 bool/u32/u64/string |

```jsonc
// ~/.if2ai/memory_config.json 示例（v2 增量）
{
  "promotion": { "session_to_project_access": 5, "session_to_project_importance": 0.6 },
  "compiler": {
    "today_max_chars": 500, "week_max_chars": 500,
    "longterm_max_chars": 300, "facts_max_chars": 200,
    "daily_check_interval_secs": 3600, "max_concurrent_llm": 3, "max_retries": 3
  },
  "experience": { "min_summary_chars": 100, "max_per_category": 50 },
  "diary": { "auto": false, "max_tokens": 2048, "temperature": 0.7 },
  "ticker": { "turns_per_summary": 6, "logical_day_cutoff_hour": 4 },
  "inject": { "to_prompt": true, "max_inject_tokens": 2000 },
  "pii_scrub": true
}
```

> settings.json 仅保留 master `enabled` + bool 开关镜像（`pii_scrub` / `inject_to_prompt` / `experience_enabled`），数值类型阈值禁止进 settings.json。

---

### Δ-10 依赖增量（Cargo.toml + package.json）

**Cargo.toml**（已经在仓里的不重复列）：

| crate                     | 版本   | 用途                            | 状态                                                   |
| ------------------------- | ------ | ------------------------------- | ------------------------------------------------------ |
| `chrono-tz`               | `0.10` | 逻辑日 4AM 切日 + 用户时区      | **新增**                                               |
| `scopeguard`              | `1`    | T-D3 daily_running flag 防泄漏  | **新增**                                               |
| `md5`                     | `0.7`  | 编译指纹                        | ✅ 已在                                                 |
| `ulid`                    | `1`    | PinnedItem / ExperienceEntry id | **新增**                                               |
| `tokio` semaphore         | —      | T-E3 / T-G2 并发限速            | ✅ 已在（`tokio = "full"`）                             |
| `parking_lot::Mutex`      | —      | TickerState 同步锁              | **可选**（若不引入则 `std::sync::Mutex`）              |
| `rusqlite` `fts5` feature | —      | facts FTS5                      | 已通过 `bundled` 内含；CI 加 `--features bundled` 验证 |

**package.json**（前端）：

| pkg                                   | 用途                           | 状态                          |
| ------------------------------------- | ------------------------------ | ----------------------------- |
| `@dnd-kit/core` + `@dnd-kit/sortable` | PinnedMemoryEditor 拖拽        | **新增**                      |
| `react-markdown` + `remark-gfm`       | CompiledViewer / DiaryRenderer | 检查；若已有则复用            |
| `framer-motion`                       | 微动效（v1 §T-UI-12）          | 检查；可降级为 CSS transition |
| `date-fns`                            | DiaryCalendar                  | 检查                          |

> 每条新增依赖在第一个使用它的 slice 里加 `cargo add` / `npm i` 步骤，并加到该 slice 的 review_checklist。

---

### Δ-11 Tauri command 双注册：`invoke_handler!` + `capabilities.json`

每个新 Tauri command 必须：

1. 添加到 `src-tauri/src/main.rs::tauri::generate_handler![...]`
2. 添加到 `src-tauri/gen/schemas/capabilities.json`（或对应的 `src-tauri/capabilities/*.json`）的 `permissions` 列表
3. 添加 TS wrapper 到 `src/lib/tauri.ts`

v1 §5 命令表中 18 个新命令全部走这套流程；slice acceptance 必须验证 `npm run tauri dev` 启动时无 capability error。

---

### Δ-12 `AppState` 增量（统一在 `commands/mod.rs` 或 `main.rs` AppState 结构内追加）

```rust
pub struct AppState {
    // 既有字段保留 ...
    pub memory_provider: SharedMemoryProvider,

    // === v2 新增 ===
    pub utility_llm: Arc<dyn UtilityLlm>,
    pub job_runner: Arc<JobRunner>,
    pub summary_store: Arc<dyn SessionSummaryStore>,         // S1
    pub pinned_store: Arc<dyn PinnedStore>,                  // S1
    pub memory_compiler: Arc<MemoryCompiler>,                // S2
    pub memory_ticker: Arc<MemoryTicker>,                    // S2
    pub fact_store: Arc<dyn FactStore>,                      // S3
    pub experience_store: Arc<dyn ExperienceStore>,          // S4
    pub diary_store: Arc<dyn DiaryStore>,                    // S4
    pub threat_scanner: Arc<ThreatScanner>,                  // S1（共享一份）
}
```

> 在 `main.rs::run` 启动序列中按依赖顺序构造。`MemoryTicker::start(scope)` 在 AppHandle 准备好后调一次。

---

### Δ-13 Locale 来源：`RuntimeConfig` 已有 `language` 字段，无需新建枚举

v1 多次出现 `Locale::Zh / En`。v2 改为 `crate::modules::runtime::config::current().language`（已经是 `String`，约定值 `"zh-CN" | "en-US"`），新建 helper：

```rust
// src-tauri/src/modules/runtime/locale.rs（新增）
pub fn is_zh() -> bool { config::current().language.starts_with("zh") }
pub fn is_zh_for(language: &str) -> bool { language.starts_with("zh") }
```

所有 prompt builder（rolling / compile_today / extractor / diary）在内部根据 `is_zh()` 切中英文。

---

### Δ-14 工具注册：通过 `tools/builtin/mod.rs` 的 `register_*` 与 `tools/registry.rs`

新工具（`pin_memory`, `unpin_memory`, `search_memory_by_tag`）：

1. 文件放 `src-tauri/src/modules/tools/builtin/{pin_memory,unpin_memory,search_memory_by_tag}.rs`
2. 在 `tools/builtin/mod.rs` 添加 `pub mod ...;` + `pub use ...;`
3. 在 `tools/registry.rs::register_builtin_tools` 中注册到 ToolRegistry
4. 工具 schema 走 `ToolDefinition`（已在 `api::types::ToolDefinition`），permission 字段已支持 `Auto/Confirm/Deny`

---

### Δ-15 既有前端组件复用：MemoryBrowser/MemoryCard 不删，只重定位

| v1 组件                                                                                                           | 实际位置 | v2 处置                                                                                                               |
| ----------------------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------- |
| `src/components/memory/MemoryBrowser.tsx`                                                                         | 已存在   | **保留**作为 `MemoryNarrativeViewer` 的“按 key 列表 fallback”模式（用户切到 `groupBy=category`），不替换              |
| `MemoryCard.tsx` / `MemoryChip.tsx` / `MemoryEvidencePanel.tsx` / `MemoryWriteCard.tsx` / `MemoryCategoryNav.tsx` | 已存在   | 全部保留并增强（不重写）；新组件在 `src/components/memory/{pinned,compiled,narrative,experience,diary}/` 下新建子目录 |
| `src/components/chat/TelemetryDrawer.tsx` / `ContextBar.tsx`                                                      | 已存在   | 增强为 v1 §T-UI-6 / T-UI-8 描述的形态                                                                                 |

---

### Δ-16 `MemoryFeatureConfig.timezone` — 取值来源

v1 T-A3 用 `cfg.timezone`。`RuntimeConfig` 当前**没有**顶层 timezone 字段，`MemoryFeatureConfig` 也没有。v2：在 `MemoryFeatureConfig` 增 `pub timezone: Option<String>`（IANA tz string，None = 用 OS local），通过 `chrono_tz::Tz::from_str(...)` 解析，回落到 `chrono_tz::UTC`。

---

### Δ-17 容量与边界（v1 没说清的）

| 项                           | v2 明确值                                                              |
| ---------------------------- | ---------------------------------------------------------------------- |
| `pin_memory` 单条 max chars  | **500**                                                                |
| `pinned` 总数硬上限          | **50/scope**（超出返回错误，UI 禁用 add 按钮）                         |
| `summary` 单条 max chars     | LLM 输出由 budget 限制；存储不截断                                     |
| `fact` 单条 max chars        | **300**（超出由 extractor 分裂或截断）                                 |
| `experience` 单条 max chars  | **400**                                                                |
| `diary` 单天 max chars       | **8000**（LLM `max_tokens=2048` 自然限制 + 写入截断）                  |
| `memory.md` 总 max chars     | **5000**（assemble 阶段若超出按 facts→today→week→longterm 优先级截断） |
| `max_inject_tokens` 注入上限 | **2000**（默认）                                                       |

---

### Δ-18 v2 不立即引入但要在文档中标注的“延后项”

| 延后项                                      | 原因                                              | 何时回来                       |
| ------------------------------------------- | ------------------------------------------------- | ------------------------------ |
| PULSE/MOOD personality preset（v1 §T-G7）   | 偏角色定位，与 if2Ai“工具型 agent”冲突            | 待产品决策；保留接口位         |
| Diary 自动每日生成                          | token 成本高，先手动跑                            | 用户主动开 `diary.auto=true`   |
| FactStore tag 搜索的语义 reranker           | 先靠 FTS + tag count；语义 reranker 走 future RFC | 评估 retrieval planner 后续    |
| Onboarding `MemoryIntroStep`（v1 §T-UI-10） | 不阻塞核心闭环                                    | S5 末尾；可单独 demote 到 v2.1 |

---

### Δ-19 与既有 Memory Control Plane / promotion / policy 的边界（防止双轨冲突）

- **写入路径**：`memory_store` tool / `memory_store` command 仍然走既有 `MemoryPolicyEngine.evaluate` → `provider.store_scoped` 链路（不绕过）。**PII scrub 插在 policy 之后、store 之前**（policy 决策时看的是原文，存盘时是 redacted；audit 同时 emit `memory_write_decision` + `memory_pii_redacted`）。
- **召回路径**：`memory_recall` 走既有 `intent_classifier → retrieval_planner → provider.recall_scoped`。新增的 `search_memory_by_tag` 工具是 **平行入口**（直接走 `FactStore`），retrieval planner 在 query 含 `[tag:...]` 模式时优先选它。
- **Promotion engine**：保留 `MemoryPromotionEngine`（既有），新增的 facts 不进入向量召回 importance 流，**facts 的“升降级”用 `FactStore` 自己的 `promote(id, scope)` 方法**（独立实现，不改 promotion.rs）。
- **rolling summary 与 compact**：`runtime::compact.rs` 触发的 compact 在写入 session 后调 `summary_store.save(record { source: Compact, ... })`；rolling 在 5 分钟去抖窗口内跳过（v1 §T-B4 不变）。

---

## 0.6 Audit Event 字段约定（与 §4 表合并阅读）

所有新增事件复用 `AuditContext`（trace_id / session_id / project_id / effective_workdir）+ `MemoryEventPayload.extra` 携带变长 metadata。前端类型：

```typescript
// src/lib/tauri.ts 增量
export type MemoryEventName =
  | 'memory_captured' | 'memory_write_decision' | 'memory_persisted'
  | 'memory_recall_served' | 'memory_rejected' | 'memory_promoted' | 'memory_demoted' | 'memory_cleared'
  | 'memory_promotion_candidate'
  // === v2 新增 ===
  | 'memory_pii_redacted' | 'memory_summary_rolled' | 'memory_compiled' | 'memory_assembled'
  | 'memory_pinned' | 'memory_unpinned' | 'memory_fact_extracted' | 'memory_experience_extracted'
  | 'memory_diary_written' | 'memory_ticker_recovery' | 'memory_job_failed' | 'memory_job_skipped';

export interface MemoryEventPayload {
  event: MemoryEventName;
  trace_id?: string; session_id?: string; project_id?: string; effective_workdir?: string;
  /* 既有 string/usize 字段保留 */
  extra?: Record<string, unknown>;          // ← v2 新增：携带 detected/sections/recovered 等结构化数据
  timestamp: string;
}
```

---

## 0.7 跨 Sprint 共享原则（落地约束）

1. **每个 slice 至少修改 impl_targets 中一个文件**（CLAUDE.md 强制 DIFF_GATE 已在）
2. **每个 slice 必须有：1 个 audit 事件 + 1 个 harness suite 断言 + 1 个 UI 可见信号**（即“后端写、前端看、harness 验”三件套）
3. **新增模块文件夹必须有 `mod.rs` 导出 + 单元测试 in-file `#[cfg(test)] mod tests`**
4. **PII scrub 是默认 ON 的红线**：禁止任何 slice 把它默认关闭，UI 也不允许直接关；只能在开发者隐藏 flag 下调试
5. **所有 background job 必须经 `JobRunner.run`**，禁止裸 `tokio::spawn` 调 LLM
6. **所有 LLM 调用必须经 `UtilityLlm` shim**，不允许 memory 子模块直接 import `api::providers`

---

## 1. Gap → Task 映射总表

| Gap                              | 影响                | Sprint     | Task        |
| -------------------------------- | ------------------- | ---------- | ----------- |
| G1 滚动摘要持久化                | 跨会话失忆          | S1         | T-B1 ~ T-B4 |
| G2 编译流水线                    | 缺长期叙事          | S2         | T-C1 ~ T-C5 |
| G3 元事实 + 标签检索             | 召回粒度粗          | S3         | T-E1 ~ T-E4 |
| G4 Pinned 静态记忆               | 无法"钉死"事实      | S1         | T-F1 ~ T-F5 |
| G5 System prompt 硬注入          | agent 经常忘 recall | S1         | T-F4        |
| G6 Experience extractor          | 无法学教训          | S4         | T-G1 ~ T-G3 |
| G7 Diary writer                  | 缺产品差异化        | S4         | T-G4 ~ T-G6 |
| G8 写入侧 PII 硬规则             | 凭证泄漏风险        | S1         | T-A1        |
| G9 逻辑日 4AM 切日               | 按日聚合错位        | S1         | T-A3        |
| G10 Turn-based ticker + 启动补偿 | 崩溃后丢摘要        | S2         | T-D1 ~ T-D4 |
| G11 指纹缓存                     | 重复 LLM 调用       | S2         | T-C2        |
| G12 PULSE/MOOD 内省（可选）      | 角色感弱            | S4（可选） | T-G7        |
| G13 Compiled memory.md viewer    | 用户看不到结果      | S2         | T-UI-2      |
| G14 按日期分组的事实墙           | 缺时间叙事          | S2         | T-UI-3      |
| G15 双开关 + disabledSince       | 无法"这次别记"      | S1         | T-A4        |
| G16 Export/Import 闭环           | 不可迁移            | S5         | T-UI-7      |
| G17 Pin/All/Compiled 三 viewer   | IA 混乱             | S2         | T-UI-1 ~ 3  |
| G18 失败计数 + 跳过              | 坏数据反复重试      | S1         | T-A2        |

---

## 2. 数据落盘约定（跨 Sprint 公共契约）

> **🔧 v2 校准**（§0.5 Δ-6）：根目录是 **`dirs::data_local_dir().join(".if2ai/memory")`**（与 `main.rs::create_memory_provider` 一致），不是字面 `~/.if2ai`。下方树形图中的 `~/.if2ai/memory/` 一律理解为 `<data_local>/.if2ai/memory/`。`~/.if2ai/memory_config.json` 是另一回事（仅用户配置覆盖）。

```
~/.if2ai/memory/                              ← 用户级（global scope）
  ├─ memory.db                                ← SQLite（既有，新增 facts/summaries 表）
  ├─ vector_db/                               ← LanceDB（既有）
  ├─ jobs.db                                  ← S1 新增：后台任务失败计数
  └─ global/
     ├─ pinned.md                             ← S1 新增：用户全局 pin
     ├─ memory.md                             ← S2 新增：编译产物
     ├─ today.md / week.md / longterm.md / facts.md
     ├─ *.fingerprint                         ← S2 指纹边车文件
     └─ summaries/<session_id>.json           ← S1 滚动摘要

<workspace>/.if2ai/memory/                    ← 项目级（project scope）
  ├─ pinned.md                                ← 项目级 pin
  ├─ memory.md                                ← 项目级编译产物
  ├─ today.md / week.md / longterm.md / facts.md
  ├─ summaries/<session_id>.json              ← 当前项目下产生的 session 摘要
  ├─ experience/<category>.md                 ← S4 经验
  ├─ experience/index.md
  └─ diary/YYYY-MM-DD.md                      ← S4 日记
```

**规则**：
- `session_id` 落 `<workspace>/.if2ai/memory/summaries/`，但摘要写入会同时给 SQLite `summaries` 表写一份（用于跨项目查询）
- 编译流水线读 SQLite `summaries` 表，按 scope 过滤，写入对应 scope 的 `memory.md`
- 当前活跃 scope 由 `MemoryExecutionScope { session_id, project_id, workdir }` 决定

---

## 3. 配置 Schema（runtime/config.rs 增量）

> **🔧 v2 校准**（§0.5 Δ-9）：下方代码中的所有 **`f64` / 嵌套结构字段不能从 `settings.json` 反序列化**（`MemoryConfigOverrides` 用的 `JsonValue` 枚举不支持 `f64`）。新增字段必须放进 `MemoryConfigOverrides`（在 `runtime/config.rs:1226` 附近），仅从 `~/.if2ai/memory_config.json` 走 `serde_json` 反序列化。`settings.json` 只能放 bool / u32 / u64 / string 镜像字段。

```rust
// 在 MemoryFeatureConfig 上追加：
pub struct MemoryFeatureConfig {
    // 既有字段保留 ...
    pub enabled: bool,
    pub promotion: PromotionThresholds,

    // === 新增 ===
    /// Pinned + Compiled memory 是否注入 system prompt（默认 true）
    pub inject_to_prompt: bool,
    /// 注入到 prompt 的最大 token 数（默认 2000）
    pub max_inject_tokens: usize,
    /// 滚动摘要触发阈值：每 N 个用户 turn 滚动一次（默认 6）
    pub turns_per_summary: u32,
    /// 编译流水线开关（默认 true）
    pub compiler: CompilerConfig,
    /// PII scrub 开关（默认 true，强烈不建议关）
    pub pii_scrub: bool,
    /// 4AM 日界线（小时，0-23，默认 4）
    pub logical_day_cutoff_hour: u8,
    /// Experience extractor 开关（默认 true）
    pub experience_enabled: bool,
    /// Diary 自动生成（默认 false，仅手动触发）
    pub diary_auto: bool,
}

pub struct CompilerConfig {
    pub today_max_chars: usize,        // 默认 500
    pub week_max_chars: usize,         // 默认 500
    pub longterm_max_chars: usize,     // 默认 300
    pub facts_max_chars: usize,        // 默认 200
    pub daily_check_interval_secs: u64,// 默认 3600（备用 timer）
    pub max_concurrent_llm: usize,     // 默认 3
    pub max_retries: u32,              // 默认 3
}
```

加载位置：`~/.if2ai/memory_config.json`（既有，扩展字段）。所有字段都有合理默认值，向后兼容。

---

## 4. Audit 事件清单（commands 与 telemetry 共享）

> **🔧 v2 校准**（§0.5 Δ-3, Δ-4 + §0.6）：
> - `MemoryAuditEmitter` 是 unit 类型，不要 `Arc`，直接 `MemoryAuditEmitter::xxx(&AuditContext::from_scope(scope), ...)`
> - 下表的“payload 关键字段”如果是 `Vec<...>` / 嵌套对象，必须走新增的 `MemoryEventPayload.extra: Option<serde_json::Value>` 字段（v1 写为顶级字段是错的）
> - `MemoryEventPayload` 的 `event` 字段、TS 端 `MemoryEventName` 联合类型同步追加 11 个新值

新增 `MemoryEventPayload.event` 枚举值：

| 事件名                        | 触发位置                                       | payload 关键字段                                                                                                     |
| ----------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `memory_pii_redacted`         | 任意写入路径 scrub 命中                        | `detected: ["api_key", "id_card"]`, `key`, `category`                                                                |
| `memory_summary_rolled`       | `SessionSummaryManager.rolling_summary` 写入后 | `session_id`, `turn_count`, `chars_before`, `chars_after`, `latency_ms`                                              |
| `memory_compiled`             | 任一 `compile_*` 完成                          | `kind: "today"\|"week"\|"longterm"\|"facts"`, `result: "compiled"\|"skipped"`, `chars_in`, `chars_out`, `latency_ms` |
| `memory_assembled`            | `assemble()` 写入 `memory.md`                  | `chars`, `sections: ["facts","today","week","longterm"]`                                                             |
| `memory_pinned`               | `pin_memory` 工具或 Tauri 命令                 | `scope`, `content_excerpt` (前 60 字), `total_pins`                                                                  |
| `memory_unpinned`             | `unpin_memory`                                 | `scope`, `removed_count`, `keyword`                                                                                  |
| `memory_fact_extracted`       | `deep-memory.process_dirty_sessions` 单条入库  | `fact_excerpt`, `tags: [...]`, `session_id`                                                                          |
| `memory_experience_extracted` | `extractSessionExperiences` 完成               | `category`, `content_excerpt`, `total_in_category`                                                                   |
| `memory_diary_written`        | `diary_writer.write_diary` 完成                | `logical_date`, `file_path`, `chars`                                                                                 |
| `memory_ticker_recovery`      | 启动时 `recover_unsummarized` 修复             | `recovered: [{session_id, mtime, summary_at}]`                                                                       |
| `memory_job_failed`           | 任意 background job 第 N 次失败                | `job: "compile_today"\|...`, `attempt`, `max_retries`, `error`                                                       |
| `memory_job_skipped`          | 失败超过 max_retries                           | `job`, `total_failures`, `last_error`                                                                                |

---

## 5. Tauri 命令清单（commands/memory.rs / commands/diary.rs / commands/pinned.rs）

> **🔧 v2 校准**（§0.5 Δ-11）：每个新命令都要 **三处注册**：
> 1. `src-tauri/src/main.rs::tauri::generate_handler![...]`
> 2. `src-tauri/gen/schemas/capabilities.json` 或 `src-tauri/capabilities/*.json` 的 `permissions`
> 3. `src/lib/tauri.ts` TS wrapper

| 命令                                                                                   | 出入参                                  | Sprint |
| -------------------------------------------------------------------------------------- | --------------------------------------- | ------ |
| 既有：`memory_store / recall / forget / purge / export / promote / demote / clear_all` | —                                       | —      |
| `pinned_get(scope: "project"\|"global"\|"both") -> Vec<PinDto>`                        | —                                       | S1     |
| `pinned_add(content: String, scope: "project"\|"global") -> PinDto`                    | —                                       | S1     |
| `pinned_delete(id: String) -> ()`                                                      | —                                       | S1     |
| `pinned_reorder(ids: Vec<String>) -> ()`                                               | —                                       | S1     |
| `memory_session_set_enabled(session_id, enabled: bool) -> ()`                          | —                                       | S1     |
| `memory_compile_now(scope: "current"\|"all") -> CompileReport`                         | report 含每块状态                       | S2     |
| `memory_compiled_read(scope) -> CompiledMemoryDto`                                     | 含 `memory.md` + 4 块产物的最后编译时间 | S2     |
| `memory_compiled_clear(scope) -> ()`                                                   | 清空编译产物 + fingerprints             | S2     |
| `memory_summaries_list(scope, limit) -> Vec<SessionSummaryDto>`                        | 给 NarrativeViewer                      | S2     |
| `memory_facts_search(query, tags, date_from, date_to, limit) -> Vec<FactDto>`          | 给前端                                  | S3     |
| `memory_facts_list(scope, limit, offset) -> PaginatedFactDto`                          | NarrativeViewer 翻页                    | S3     |
| `experience_list(scope, category) -> Vec<ExperienceDto>`                               | —                                       | S4     |
| `experience_delete(id) -> ()`                                                          | —                                       | S4     |
| `diary_write(date?: NaiveDate) -> DiaryDto`                                            | 默认今天                                | S4     |
| `diary_list(year, month) -> Vec<DiaryMetaDto>`                                         | 月历                                    | S4     |
| `diary_read(date: NaiveDate) -> DiaryDto`                                              | —                                       | S4     |
| `memory_export_v3(include: ExportOptions) -> ExportPayload`                            | facts+summaries+pinned+experience       | S5     |
| `memory_import_v3(payload: ExportPayload) -> ImportReport`                             | 兼容 v1/v2                              | S5     |

---

# Sprint 1 — Hardening + 核心闭环（约 2 周）

**目标**：让 if2Ai 真正具备"记得住、记得安全、用户能参与"的最小闭环。

## Phase A — 写入侧合规与可观测

### T-A1：PII Scrub 接入所有持久化写入路径

> **🔧 v2 校准**（参见 §0.5 Δ-2）：**不要新建 `scrub_for_storage`**；扩展既有 `security::ThreatScanner`，新增 `scan_and_redact()` 方法 + 3 个 pattern (`credit_card`, `id_card_cn`, `ssn_us`)。`scan()` 保持向后兼容（policy 路径继续用）。`AppState` 共享一份 `Arc<ThreatScanner>`。

**目标**：API key、PEM、信用卡、身份证、SSN 在写入 SQLite/LanceDB/markdown 之前被强制脱敏。

**改动文件**：
- `src-tauri/src/modules/memory/security.rs`（新增 `scrub_for_storage`）
- `src-tauri/src/commands/memory.rs::memory_store`（写入前调）
- `src-tauri/src/modules/memory/providers/sqlite_provider.rs::store / store_scoped`
- `src-tauri/src/modules/memory/providers/vector_provider.rs::store_scoped`
- 后续 S1 的 pinned/summary 写入路径

**接口**：

```rust
// security.rs
pub struct ScrubResult {
    pub cleaned: String,
    pub detected: Vec<DetectedPii>,
}

pub struct DetectedPii {
    pub kind: PiiKind,
    pub original_excerpt: String,    // 前 8 字 + 后 4 字 + ...
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PiiKind {
    ApiKey,
    InlineSecret,
    PrivateKey,
    CreditCard,
    IdCard,
    Ssn,
}

pub fn scrub_for_storage(text: &str) -> ScrubResult;
```

**实现要点**：
1. 复刻 `lib/pii-guard.js` 的 6 条 HARD_PATTERNS（regex 表达式直接搬，无需重新设计）
2. 用 `regex::Regex` + `OnceLock` 缓存编译
3. 命中后替换为 `[REDACTED:<kind>]` 而非简单 `[REDACTED]`，便于审计
4. 在每个写入路径包一层：

```rust
// commands/memory.rs::memory_store
let scrubbed = if state.config.memory.pii_scrub {
    let r = security::scrub_for_storage(&content);
    if !r.detected.is_empty() {
        MemoryAuditEmitter::memory_pii_redacted(&audit_ctx, &key, &r.detected);
        tracing::warn!(?r.detected, "PII detected in memory write");
    }
    r.cleaned
} else { content };
state.memory_provider.store(&key, &scrubbed, category).await?;
```

5. 单元测试覆盖每条 pattern 的命中 + 不误命中（普通文本中含数字、含 sk- 前缀但长度不够等）

**验收**：
- [ ] `cargo test memory::security::tests::scrub_*` 通过
- [ ] `memory_store` 命中 6 类 PII 时返回成功且 SQLite 中存的是 `[REDACTED:*]`
- [ ] audit 事件 `memory_pii_redacted` 在 TelemetryDrawer 可见

### T-A2：后台任务失败计数 + skip-after-N

**目标**：复刻 `_failCounts.set / get / delete` + `MAX_RETRIES = 3` 模型，避免坏数据反复重试。

**改动文件**：
- `src-tauri/src/modules/memory/job_runner.rs`（新增）
- `src-tauri/src/modules/memory/promotion.rs`（接入）
- 后续 S2/S4 所有 background LLM 任务都接入

**数据结构**：

```rust
pub struct JobRunner {
    db: Arc<Mutex<Connection>>,                // jobs.db
    max_retries: u32,                          // 默认 3
    max_concurrent: usize,                     // 默认 3
}

#[derive(Debug)]
pub struct JobAttempt {
    pub job_kind: String,                      // "compile_today" / "deep_memory_extract" / ...
    pub job_target: String,                    // 通常是 session_id 或 scope key
    pub attempt: u32,
    pub last_error: Option<String>,
    pub last_attempt_at: DateTime<Utc>,
    pub status: JobStatus,                     // Active | Skipped | Done
}
```

**SQL（jobs.db）**：

```sql
CREATE TABLE IF NOT EXISTS job_attempts (
    job_kind     TEXT NOT NULL,
    job_target   TEXT NOT NULL,
    attempt      INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT,
    last_attempt_at TEXT NOT NULL,
    status       TEXT NOT NULL,                -- 'active'|'skipped'|'done'
    PRIMARY KEY (job_kind, job_target)
);
```

**API**：

```rust
impl JobRunner {
    pub async fn run<F, Fut, T>(&self, kind: &str, target: &str, f: F) -> Result<Option<T>, JobError>
    where F: FnOnce() -> Fut, Fut: Future<Output = Result<T, anyhow::Error>>;
    // Some(T) = 成功，None = 已 skipped 不再重试，Err = 本次失败但仍可重试
}
```

**实现要点**：
1. 进入 `run` 时先查 `status`，若 `skipped` 直接返回 `Ok(None)`
2. 执行 `f()`，成功 → `status='done'` + `attempt+=1`
3. 失败 → `attempt+=1, last_error=...`，若 `attempt >= max_retries` → `status='skipped'`，发 `memory_job_skipped` 事件；否则发 `memory_job_failed`
4. 用 `tokio::sync::Semaphore::new(max_concurrent)` 限制并发

**验收**：
- [ ] 同一 (kind, target) 失败 3 次后第 4 次直接返回 `Ok(None)`
- [ ] 成功一次后 `status` 变 `done`，下次重新调用会重置 `attempt=0`（成功后允许再次执行）

### T-A3：逻辑日（Logical Day）工具

**目标**：4AM 切日，全系统统一按"逻辑日"做按日聚合。

**改动文件**：
- `src-tauri/src/modules/runtime/logical_day.rs`（新增）

**API**：

```rust
pub struct LogicalDay {
    pub date: NaiveDate,                       // 该逻辑日所属日历日
    pub range_start: DateTime<Utc>,            // 当日 04:00 本地时间换算到 UTC
    pub range_end: DateTime<Utc>,              // 次日 04:00 - 1ms
    pub display: String,                       // "2026-04-18"
}

pub fn get_logical_day(now: DateTime<Utc>, cutoff_hour: u8, tz: chrono_tz::Tz) -> LogicalDay;

pub fn get_today() -> LogicalDay {
    let cfg = crate::modules::runtime::config::current();
    let tz = chrono_tz::Tz::from_str(&cfg.timezone).unwrap_or(chrono_tz::UTC);
    get_logical_day(Utc::now(), cfg.memory.logical_day_cutoff_hour, tz)
}
```

**实现要点**：
1. 把 `now` 转到本地时区
2. 如果 hour < cutoff → 归到前一日；否则归到当日
3. range_start = 当日 04:00 本地时间，range_end = 次日 04:00 - 1ms
4. 转回 UTC 返回

**单元测试**：
- 04:00 整 → 当日
- 03:59 → 前一日
- 12:00 → 当日
- 跨夏令时切换边界

**验收**：
- [ ] `cargo test runtime::logical_day::tests` 通过
- [ ] S2 编译流水线全部使用 `get_today()`，不再裸用 `Utc::now().date_naive()`

### T-A4：Master + per-session 双开关

**目标**：用户可以"这一次别记"，session 关闭后历史摘要不再被 ticker 处理。

**改动文件**：
- `src-tauri/src/modules/runtime/config.rs::MemoryFeatureConfig`（master 已有）
- `src-tauri/src/modules/session/manager.rs::SessionMeta`（增字段）
- `src-tauri/src/commands/session.rs`（新命令 `memory_session_set_enabled`）

**SessionMeta 增量**：

```rust
pub struct SessionMeta {
    // 既有字段...
    pub memory_enabled: Option<bool>,          // None = 跟随 master
    pub memory_disabled_since: Option<DateTime<Utc>>,
    pub memory_reenabled_at: Option<DateTime<Utc>>,
}
```

**判断函数**：

```rust
pub fn is_session_memory_on(session: &SessionMeta, master_on: bool) -> bool {
    if !master_on { return false; }
    session.memory_enabled.unwrap_or(true)
}
```

**实现要点**：
1. ticker 在 `notify_turn / notify_session_end` 入口先调 `is_session_memory_on`，false 直接 return
2. 每次写入 summary/fact 时同样检查
3. 关闭时写 `memory_disabled_since = now`；后续编译流水线在 `getSummariesInRange` 时过滤掉 `updated_at >= disabled_since && updated_at <= reenabled_at` 的窗口
4. Tauri 命令：`memory_session_set_enabled(session_id, enabled)` → 修改 SessionMeta 并持久化

**验收**：
- [ ] 关闭后写入 `memory_store` 直接返回 `Ok(())` 但不实际入库（或入库但被打 `disabled_window` 标记）
- [ ] 前端 ChatHeader 切换开关 → 立刻生效

---

## Phase B — Session 滚动摘要持久化（核心 P0）

### T-B1：SessionSummaryManager 模块骨架

**目标**：建立 `modules/memory/summary/` 子模块，提供"每 session 一份 JSON + SQLite 索引"的滚动摘要存储。

**改动文件**（全部新增）：
```
src-tauri/src/modules/memory/summary/
  mod.rs
  store.rs
  rolling.rs
  prompt.rs
  schema.rs
```

**核心数据结构（schema.rs）**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryRecord {
    pub session_id: String,
    pub project_id: Option<String>,            // 用于 scope 过滤
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub summary: String,                       // 当前最新摘要（覆盖式）
    pub snapshot: String,                      // 上次被 deep-memory 处理的版本
    pub snapshot_at: Option<DateTime<Utc>>,
    pub message_count: usize,                  // 已覆盖的消息总数（增量切片用）
}
```

**SQLite 表（追加到 memory.db）**：

```sql
CREATE TABLE IF NOT EXISTS session_summaries (
    session_id     TEXT PRIMARY KEY,
    project_id     TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    summary        TEXT NOT NULL,
    snapshot       TEXT NOT NULL DEFAULT '',
    snapshot_at    TEXT,
    message_count  INTEGER NOT NULL DEFAULT 0,
    is_dirty       INTEGER GENERATED ALWAYS AS (summary != snapshot) VIRTUAL
);
CREATE INDEX IF NOT EXISTS idx_summaries_project ON session_summaries(project_id);
CREATE INDEX IF NOT EXISTS idx_summaries_updated ON session_summaries(updated_at);
```

**store.rs API**：

```rust
pub trait SessionSummaryStore: Send + Sync {
    async fn get(&self, session_id: &str) -> Result<Option<SessionSummaryRecord>>;
    async fn save(&self, record: &SessionSummaryRecord) -> Result<()>;
    async fn list_in_range(
        &self,
        scope: &MemoryExecutionScope,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SessionSummaryRecord>>;
    async fn list_dirty(&self, scope: &MemoryExecutionScope) -> Result<Vec<SessionSummaryRecord>>;
    async fn mark_processed(&self, session_id: &str) -> Result<()>;
}

pub struct SqliteSessionSummaryStore { db: Arc<Mutex<Connection>>, scope_root: PathBuf }
```

**实现要点**：
1. **双写**：每次 `save` 同时写 SQLite **和** `<scope>/.if2ai/memory/summaries/<session_id>.json`（atomic `tmp + rename`）
2. SQLite 是检索源，文件是冷备 + 用户可读
3. `mark_processed` 把 `snapshot = summary, snapshot_at = now`
4. `list_dirty` SQL：`WHERE summary != snapshot AND project_id = ?`

**验收**：
- [ ] 写入后 SQLite 和文件内容一致
- [ ] 按时间范围 / scope 查询命中正确
- [ ] `is_dirty` 虚字段正确反映摘要新鲜度

### T-B2：滚动摘要 LLM Prompt 与 budget 计算

**目标**：复刻 `_callRollingLLM` 的"按 turn 缩放 budget + 双节固定输出"。

**改动文件**：`src-tauri/src/modules/memory/summary/prompt.rs`

**核心函数**：

```rust
pub struct RollingSummaryPrompt {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

pub fn build_rolling_summary_prompt(
    locale: Locale,                            // Zh | En
    has_prev: bool,
    prev_summary: &str,
    new_conversation: &str,
    turn_count: usize,
) -> RollingSummaryPrompt {
    // 1. 按轮数线性缩放：每轮 40 字配额，10 轮封顶 400 字
    let total_budget = (turn_count * 40).clamp(40, 400);
    let facts_budget = ((total_budget as f32) * 0.3).round() as usize;
    let events_budget = total_budget - facts_budget;
    // 2. 英文 word budget = char budget * 0.6
    // 3. system prompt：直接搬 openhanako 中文/英文版本（见 session-summary.js:282-328）
    // 4. user prompt：has_prev ? "## 已有摘要\n\n{prev}\n\n## 新增对话\n\n{conv}" : conv
    // 5. max_tokens = (total_budget * 1.5).clamp(150, 750) as u32
}
```

**对话文本构建（同 _buildConversationText）**：

```rust
pub fn build_conversation_text(messages: &[ChatMessage]) -> String {
    const ASSISTANT_CAP: usize = 300;
    messages.iter().filter_map(|m| {
        let text = extract_text(&m.content)?;
        let time_prefix = m.timestamp.map(|t| format!("[{:02}:{:02}] ", t.hour(), t.minute())).unwrap_or_default();
        let speaker = match m.role {
            Role::User => "用户",
            Role::Assistant => "助手",
            _ => return None,
        };
        let body = if matches!(m.role, Role::Assistant) && text.chars().count() > ASSISTANT_CAP {
            format!("{}…（长回复已截断）", text.chars().take(ASSISTANT_CAP).collect::<String>())
        } else { text };
        Some(format!("{}【{}】{}", time_prefix, speaker, body))
    }).collect::<Vec<_>>().join("\n\n")
}
```

**验收**：
- [ ] 1 轮对话 → budget 40 字
- [ ] 10 轮 → budget 400 字
- [ ] system prompt 包含 `## 重要事实` `## 事情经过` 两节标记

### T-B3：rolling.rs — 滚动摘要主循环

> **🔧 v2 校准**（§0.5 Δ-1, Δ-3, Δ-8）：
> - `llm: Arc<dyn LlmClient>` → **`llm: Arc<dyn UtilityLlm>`**（新建于 `memory/llm.rs`）
> - `audit: Arc<MemoryAuditEmitter>` → **删除字段**，调用 `MemoryAuditEmitter::memory_summary_rolled(&AuditContext::from_scope(scope), ...)`
> - 触发点 **不在 commands/agent.rs**；通过 `ConversationRuntime::with_turn_hook(Arc<dyn TurnHook>)` 注入

**目标**：调度 LLM、scrub PII、写存储、emit audit。

**改动文件**：`src-tauri/src/modules/memory/summary/rolling.rs`

**核心结构**：

```rust
pub struct RollingSummarizer {
    store: Arc<dyn SessionSummaryStore>,
    llm: Arc<dyn LlmClient>,                   // 通过 ProviderManager 取 utility 通道
    job_runner: Arc<JobRunner>,
    pii_enabled: bool,
}

impl RollingSummarizer {
    pub async fn rolling_summary(
        &self,
        session_id: &str,
        scope: &MemoryExecutionScope,
        messages: &[ChatMessage],
    ) -> Result<Option<SessionSummaryRecord>>;
}
```

**实现步骤**：

1. 读 `existing = store.get(session_id)`，取 `prev_summary` 和 `last_message_count`
2. **增量切片**：`new_messages = if last_message_count < messages.len() { &messages[last_message_count..] } else { messages }`
3. `conv_text = build_conversation_text(new_messages)`，空则返回 `Ok(None)`
4. `turn_count = messages.iter().filter(|m| m.role == Role::User).count()`
5. `prompt = build_rolling_summary_prompt(...)`
6. 调 `job_runner.run("rolling_summary", session_id, || llm.complete(prompt))`
7. PII scrub
8. 写存储 `store.save(record)`
9. emit `memory_summary_rolled`

**验收**：
- [ ] 第一次调用：`prev_summary` 空 → 全量
- [ ] 第二次调用：只送增量
- [ ] 失败 3 次后该 session 被标记 skipped
- [ ] 写入后 `is_dirty=true`（snapshot 仍为旧）

### T-B4：Memory Compact ↔ Rolling Summary 双轨协调

**目标**：避免现有 `runtime/compact.rs`（context window 触顶时一次性压缩）和新 rolling summary 重复消耗 token。

**改动文件**：`src-tauri/src/modules/runtime/compact.rs`

**实现要点**：
1. `compact()` 在生成新 summary 后，**也调** `RollingSummarizer.store.save()` 记录到持久化层（标记 `source = "compact"`）
2. `RollingSummarizer` 在每 N 轮执行前先检查：如果 5 分钟内已有 compact 写入，直接 skip 这次 rolling
3. 在 `SessionSummaryRecord` 加字段 `source: SummarySource { Rolling | Compact }`

**验收**：
- [ ] 触发 compact 后立刻触发 rolling → 第二次调用被 skip，token 0 消耗
- [ ] 两个来源的摘要在 viewer 中能区分显示

---

## Phase F — Pinned Memory（核心 P0，体验杠杆最大）

### T-F1：Pinned 存储模块

**改动文件**：
- `src-tauri/src/modules/memory/pinned/mod.rs`（新增）
- `src-tauri/src/modules/memory/pinned/store.rs`（新增）

**数据结构**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedItem {
    pub id: String,                            // ULID
    pub content: String,
    pub scope: PinScope,                       // Project | Global
    pub created_at: DateTime<Utc>,
    pub created_by: PinSource,                 // User | Tool { tool_name, session_id }
}

pub enum PinScope { Project, Global }
pub enum PinSource { User, Tool { tool_name: String, session_id: String } }
```

**存储双写**：
- 主存：SQLite 表 `pinned_items(id, content, scope, project_id, created_at, created_by_kind, created_by_meta JSON)`
- 镜像：`<scope_root>/pinned.md`（每行 `- {content}`，便于人工查看，**写入时 PII scrub**）

**API**：

```rust
pub trait PinnedStore: Send + Sync {
    async fn list(&self, scope: PinScope, project_id: Option<&str>) -> Result<Vec<PinnedItem>>;
    async fn list_all_for_prompt(&self, scope: &MemoryExecutionScope) -> Result<Vec<PinnedItem>>;
    async fn add(&self, content: &str, scope: PinScope, source: PinSource, project_id: Option<&str>) -> Result<PinnedItem>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn reorder(&self, ids: &[String]) -> Result<()>;
}
```

**验收**：
- [ ] 添加重复内容（去前后空白后字符串相等）→ 返回既有 id，不重复入库
- [ ] PII 脱敏与 audit 事件触发

### T-F2：pin_memory / unpin_memory 内置工具

**改动文件**：
- `src-tauri/src/modules/tools/builtin/pin_memory.rs`（新增）
- `src-tauri/src/modules/tools/builtin/unpin_memory.rs`（新增）
- `src-tauri/src/modules/tools/builtin/mod.rs`（注册）

**工具签名**：

```rust
// pin_memory
ToolDefinition {
    name: "pin_memory",
    description: "钉住一条事实，让 agent 永远记住（始终注入 system prompt）",
    parameters: {
        content: String (required, max 500 chars),
        scope: enum["project", "global"] (default "project"),
    },
    permission: ToolPermission::Auto,           // 不需要确认
    execute: async |ctx, params| -> ToolResult,
}

// unpin_memory
ToolDefinition {
    name: "unpin_memory",
    parameters: {
        keyword: String (required, 用于匹配要删除的 pin),
    },
}
```

**执行流程**：
1. 解析 scope → `PinScope`
2. 从 `ctx.scope.project_id / session_id` 推断 `project_id`
3. 调 `pinned_store.add(...)`
4. 返回结果含已脱敏后的内容 + 当前总 pin 数

**验收**：
- [ ] LLM 调用 `pin_memory({content, scope})` 后 settings 页面 pinned 列表立即出现该项
- [ ] `unpin_memory({keyword})` 大小写不敏感、子串匹配

### T-F3：Tauri 命令暴露

**改动文件**：
- `src-tauri/src/commands/pinned.rs`（新增）
- `src-tauri/src/main.rs`（注册）
- `src/lib/tauri.ts`（前端 wrapper）

```rust
#[tauri::command] pub async fn pinned_get(scope: String, state: State<AppState>) -> Result<Vec<PinnedItem>, String>;
#[tauri::command] pub async fn pinned_add(content: String, scope: String, state: State<AppState>) -> Result<PinnedItem, String>;
#[tauri::command] pub async fn pinned_delete(id: String, state: State<AppState>) -> Result<(), String>;
#[tauri::command] pub async fn pinned_reorder(ids: Vec<String>, state: State<AppState>) -> Result<(), String>;
```

### T-F4：System Prompt 静态注入

> **🔧 v2 校准**（§0.5 Δ-7）：`SystemPromptBuilder::build()` 是同步的，**不能 await**。改为：
> 1. 调用方（`commands::agent::run_agent_turn` 或 `runtime` 启动前 prep）先 `let injection = build_memory_injection(...).await?;`
> 2. 通过新增 builder 方法 `with_memory_injection(injection: MemoryInjection)` 传入
> 3. `build()` 内在 `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` 之后、`append_sections` 之前追加 pinned + compiled + rules 三段

**目标**：把 pinned + compiled memory 注入 agent system prompt，agent 不再依赖 active recall。

**改动文件**：
- `src-tauri/src/modules/runtime/prompt.rs`（核心修改）
- `src-tauri/src/modules/memory/inject.rs`（新增 helper）

**inject.rs 接口**：

```rust
pub struct MemoryInjection {
    pub pinned_section: Option<String>,        // markdown
    pub compiled_section: Option<String>,      // markdown
    pub rules_section: String,                 // 6 行"记忆使用规则"
    pub total_tokens_estimate: usize,
}

pub async fn build_memory_injection(
    pinned_store: &dyn PinnedStore,
    scope: &MemoryExecutionScope,
    compiled_path: &Path,                      // <scope>/.if2ai/memory/memory.md
    locale: Locale,
    max_tokens: usize,
) -> Result<MemoryInjection>;
```

**实现步骤**：
1. 拉 `pinned_store.list_all_for_prompt(scope)` → project + global 合并去重
2. 读 `memory.md`（若存在）
3. 按 budget 截断：先满足 pinned（pinned 优先），剩余 token 给 compiled
4. **拼接"记忆使用规则"**（直接复刻 openhanako 中文/英文版本，agent.js:626-644）

**prompt.rs 接入**：

```rust
fn assemble_system_prompt(...) -> String {
    let mut parts = vec![];
    parts.push(identity_section);
    parts.push(skills_section);
    // ...

    if config.memory.inject_to_prompt {
        let injection = build_memory_injection(
            &state.pinned_store,
            &scope,
            &compiled_path,
            locale,
            config.memory.max_inject_tokens,
        ).await?;
        if let Some(p) = injection.pinned_section { parts.push(format!("\n---\n\n# 置顶记忆\n\n{p}")); }
        if let Some(c) = injection.compiled_section { parts.push(format!("\n---\n\n# 记忆\n\n{c}")); }
        parts.push(format!("\n{}", injection.rules_section));
    }

    parts.join("\n")
}
```

**验收**：
- [ ] 加 pin → 下一轮对话的 system prompt 包含该 pin
- [ ] memory.md 编译后 → system prompt 含编译产物
- [ ] 关闭 `inject_to_prompt` → 完全不注入
- [ ] 注入总长度不超过 `max_inject_tokens`

### T-F5：迁移与默认值

**目标**：第一次运行时不让 prompt 暴增，给用户温柔的引导。

**改动**：
- `MemoryFeatureConfig.inject_to_prompt` 默认 **true**
- `max_inject_tokens` 默认 **2000**
- 新建 agent 时自动 touch `pinned.md`（与 openhanako `first-run.js:50` 同）
- 给已存在 agent 提供"扫描 conversation 自动建议 pin"的一次性 onboarding（S5）

---

# Sprint 2 — 编译流水线 + Compiled Viewer + Ticker（约 2 周）

## Phase C — Memory Compiler

### T-C1：Compiler 模块骨架

**改动文件**（全部新增）：
```
src-tauri/src/modules/memory/compiler/
  mod.rs                                       // pub use 与 MemoryCompiler 类型
  fingerprint.rs                               // 指纹计算 + 边车文件
  today.rs
  week.rs
  longterm.rs
  facts.rs
  assemble.rs
```

**MemoryCompiler 主结构**：

```rust
pub struct MemoryCompiler {
    summary_store: Arc<dyn SessionSummaryStore>,
    llm: Arc<dyn LlmClient>,
    job_runner: Arc<JobRunner>,
    config: CompilerConfig,
    audit: Arc<MemoryAuditEmitter>,
}

pub struct CompilePaths {
    pub root: PathBuf,                         // <scope>/.if2ai/memory/
    pub today_md: PathBuf,
    pub week_md: PathBuf,
    pub longterm_md: PathBuf,
    pub facts_md: PathBuf,
    pub memory_md: PathBuf,
}

#[derive(Debug, Serialize)]
pub enum CompileResult { Compiled, Skipped }

impl MemoryCompiler {
    pub async fn compile_today(&self, scope: &MemoryExecutionScope, paths: &CompilePaths) -> Result<CompileResult>;
    pub async fn compile_week(&self, scope: &MemoryExecutionScope, paths: &CompilePaths) -> Result<CompileResult>;
    pub async fn compile_longterm(&self, paths: &CompilePaths) -> Result<CompileResult>;
    pub async fn compile_facts(&self, scope: &MemoryExecutionScope, paths: &CompilePaths) -> Result<CompileResult>;
    pub fn assemble(&self, paths: &CompilePaths) -> Result<()>;
}
```

### T-C2：指纹边车机制

**改动文件**：`compiler/fingerprint.rs`

```rust
pub fn compute_fingerprint(keys: &[String]) -> String {
    let mut hasher = Md5::new();
    hasher.update(keys.join("\n").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn read_fingerprint(output_path: &Path) -> Option<String> {
    fs::read_to_string(output_path.with_extension("md.fingerprint")).ok()
        .map(|s| s.trim().to_string())
}

pub fn write_fingerprint(output_path: &Path, fp: &str) -> Result<()> {
    fs::write(output_path.with_extension("md.fingerprint"), fp).map_err(Into::into)
}

pub fn is_unchanged(output_path: &Path, current_fp: &str) -> bool {
    output_path.exists() && read_fingerprint(output_path).as_deref() == Some(current_fp)
}
```

**所有 compile_* 都要：先算指纹 → 命中则 skip → 否则编译并 atomic 写入 + 写指纹**

**验收**：
- [ ] 同一份摘要连续编译 2 次：第二次直接 skip，0 LLM 调用
- [ ] 改了 1 条摘要 → 指纹变 → 重新编译

### T-C3：compile_today / compile_week / compile_longterm

**today.rs 实现要点**（其他三个同构）：

```rust
pub async fn compile_today(
    summary_store: &dyn SessionSummaryStore,
    scope: &MemoryExecutionScope,
    output_path: &Path,
    llm: &dyn LlmClient,
    config: &CompilerConfig,
) -> Result<CompileResult> {
    fs::create_dir_all(output_path.parent().unwrap())?;
    let today = get_today();
    let summaries = summary_store.list_in_range(scope, today.range_start, today.range_end).await?;
    let fp_keys = if summaries.is_empty() { vec!["empty".to_string()] }
                  else { summaries.iter().map(|s| format!("{}:{}", s.session_id, s.updated_at.to_rfc3339())).collect() };
    let fp = compute_fingerprint(&fp_keys);
    if is_unchanged(output_path, &fp) { return Ok(CompileResult::Skipped); }

    if summaries.is_empty() {
        atomic_write(output_path, "")?; write_fingerprint(output_path, &fp)?;
        return Ok(CompileResult::Compiled);
    }

    let input = summaries.iter().map(|s| s.summary.as_str()).collect::<Vec<_>>().join("\n\n---\n\n");
    let prompt = build_compile_today_prompt(locale(), config.today_max_chars);  // 直接搬 compile.js:62-63 中文/英文
    let result = llm.complete(prompt, &input, /* max_tokens */ 750, /* temperature */ 0.3).await?;
    atomic_write(output_path, &result)?; write_fingerprint(output_path, &fp)?;
    Ok(CompileResult::Compiled)
}
```

**关键差异**：
- `compile_week`：range = 过去 7 天
- `compile_longterm`：依赖 `week.md` 已存在；输入 = 旧 longterm + week；指纹只用 week 内容
- `compile_facts`：从最近 30 天 summary 抽 `## 重要事实` 段（regex `##\s*重要事实\s*\n([\s\S]*?)(?=\n##|$)`），与旧 facts 合并；< 500 字直接写不调 LLM

### T-C4：assemble.rs

```rust
pub fn assemble(paths: &CompilePaths) -> Result<()> {
    let read = |p: &Path| fs::read_to_string(p).unwrap_or_default().trim().to_string();
    let facts = read(&paths.facts_md);
    let today = read(&paths.today_md);
    let week = read(&paths.week_md);
    let longterm = read(&paths.longterm_md);

    let is_zh = locale().is_zh();
    let empty = if is_zh { "（暂无）" } else { "(none)" };
    let section = |title: &str, body: &str| format!("## {title}\n\n{}", if body.is_empty() { empty } else { body });

    let md = [
        section(if is_zh {"重要事实"} else {"Key facts"}, &facts),
        section(if is_zh {"今天"} else {"Today"}, &today),
        section(if is_zh {"最近一周"} else {"Past week"}, &week),
        section(if is_zh {"长期情况"} else {"Long-term context"}, &longterm),
    ].join("\n\n") + "\n";
    atomic_write(&paths.memory_md, &md)
}
```

### T-C5：Tauri 命令 `memory_compile_now / memory_compiled_read / memory_compiled_clear`

**改动文件**：`src-tauri/src/commands/memory.rs`（追加）

```rust
#[derive(Serialize)]
pub struct CompileReport {
    pub today: CompileResult,
    pub week: CompileResult,
    pub longterm: CompileResult,
    pub facts: CompileResult,
    pub assembled: bool,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct CompiledMemoryDto {
    pub memory_md: String,
    pub today: CompiledSection,
    pub week: CompiledSection,
    pub longterm: CompiledSection,
    pub facts: CompiledSection,
}

pub struct CompiledSection {
    pub content: String,
    pub last_compiled_at: Option<DateTime<Utc>>,  // 取文件 mtime
    pub chars: usize,
}

#[tauri::command]
pub async fn memory_compile_now(scope: String, state: State<AppState>) -> Result<CompileReport, String>;
#[tauri::command]
pub async fn memory_compiled_read(scope: String, state: State<AppState>) -> Result<CompiledMemoryDto, String>;
#[tauri::command]
pub async fn memory_compiled_clear(scope: String, state: State<AppState>) -> Result<(), String>;
```

`memory_compiled_clear` 实现：清空 `today/week/longterm/facts/memory.md` 内容 + `unlink` 4 个 fingerprint 文件。

---

## Phase D — Memory Ticker（Turn-based 调度）

### T-D1：Ticker 主结构

**改动文件**：`src-tauri/src/modules/memory/ticker.rs`（新增）

```rust
pub struct MemoryTicker {
    summarizer: Arc<RollingSummarizer>,
    compiler: Arc<MemoryCompiler>,
    summary_store: Arc<dyn SessionSummaryStore>,
    config: TickerConfig,                      // turns_per_summary, daily_check_interval
    state: Mutex<TickerState>,
    audit: Arc<MemoryAuditEmitter>,
}

struct TickerState {
    turn_counts: HashMap<String, u32>,         // session_id → turn count
    summary_in_progress: HashSet<String>,
    last_daily_job_date: Option<NaiveDate>,
    daily_steps_completed: HashSet<DailyStep>, // {Today, Week, Longterm, Facts, Assemble, DeepMemory}
    daily_steps_date: Option<NaiveDate>,
    daily_running: bool,
}

#[derive(Hash, PartialEq, Eq)]
pub enum DailyStep { Today, Week, Longterm, Facts, Assemble, DeepMemory }
```

### T-D2：notify_turn / notify_session_end

> **🔧 v2 校准**（§0.5 Δ-8）：接入点不是 `commands/agent.rs`。
> - `MemoryTicker` 实现 `runtime::conversation::TurnHook` trait（见 Δ-8）
> - 在 `AppState::build` 时 `runtime = ConversationRuntime::new(...).with_turn_hook(state.memory_ticker.clone())`
> - `commands::session::end_session` 仍可调 `state.memory_ticker.flush_session(...)` 做最后冲刷

```rust
impl MemoryTicker {
    pub fn notify_turn(&self, scope: &MemoryExecutionScope, session_id: &str) {
        if !self.is_session_memory_on(session_id) { return; }
        let mut s = self.state.lock();
        let count = s.turn_counts.entry(session_id.to_string()).and_modify(|c| *c += 1).or_insert(1);
        let count = *count;
        drop(s);
        if count % self.config.turns_per_summary == 0 {
            self.spawn_rolling_then_compile_today(scope.clone(), session_id.to_string());
        }
        self.maybe_run_daily(scope);
    }

    pub async fn notify_session_end(&self, scope: &MemoryExecutionScope, session_id: &str) {
        let count = { let mut s = self.state.lock(); s.turn_counts.remove(session_id).unwrap_or(0) };
        if count == 0 || !self.is_session_memory_on(session_id) { return; }
        self.do_rolling_summary(scope, session_id).await.ok();
        self.do_compile_today_and_assemble(scope).await.ok();
        if self.config.experience_enabled {
            self.do_extract_experiences(scope, session_id).await.ok();    // S4
        }
    }

    pub async fn notify_promoted(&self, scope: &MemoryExecutionScope, session_id: &str);
    pub async fn flush_session(&self, scope: &MemoryExecutionScope, session_id: &str);
}
```

**接入点**：
- `commands/agent.rs::run_agent_turn` 在 turn 完成后调 `ticker.notify_turn`
- `commands/session.rs::end_session` 调 `notify_session_end`

### T-D3：每日任务（步骤化断点续跑）

```rust
async fn do_daily(&self, scope: &MemoryExecutionScope, paths: &CompilePaths) -> Result<()> {
    {
        let mut s = self.state.lock();
        if s.daily_running { return Ok(()); }
        s.daily_running = true;
        let today = get_today().date;
        if s.daily_steps_date != Some(today) {
            s.daily_steps_completed.clear();
            s.daily_steps_date = Some(today);
        }
    }
    let _guard = scopeguard::guard((), |_| self.state.lock().daily_running = false);

    let mut had_failure = false;
    for step in [DailyStep::Today, DailyStep::Week, DailyStep::Longterm, DailyStep::Facts] {
        if self.state.lock().daily_steps_completed.contains(&step) { continue; }
        match self.run_step(step, scope, paths).await {
            Ok(_) => { self.state.lock().daily_steps_completed.insert(step); },
            Err(e) => {
                had_failure = true;
                tracing::error!(?step, ?e, "daily step failed");
                self.audit.memory_job_failed(step.into(), &e.to_string());
            }
        }
    }
    self.compiler.assemble(paths)?;
    if !had_failure {
        self.state.lock().last_daily_job_date = Some(get_today().date);
    }
    Ok(())
}
```

**关键约束**：
- `Longterm` 必须在 `Week` 完成后才执行（依赖 week.md）
- 任一步骤失败不阻塞其他步骤，下一小时备用 timer 重试未完成步骤

### T-D4：启动补偿 + 备用 timer

```rust
impl MemoryTicker {
    pub async fn start(&self, scope: &MemoryExecutionScope) {
        self.recover_unsummarized(scope).await.ok();
        let me = self.clone();
        let scope2 = scope.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(me.config.daily_check_interval_secs));
            loop {
                interval.tick().await;
                me.maybe_run_daily(&scope2);
            }
        });
    }

    async fn recover_unsummarized(&self, scope: &MemoryExecutionScope) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        let recent_sessions = self.list_session_files_modified_after(scope, cutoff).await?;
        let mut recovered = vec![];
        for sid in recent_sessions {
            let existing = self.summary_store.get(&sid).await?;
            let summary_at = existing.as_ref().map(|r| r.updated_at).unwrap_or_else(|| DateTime::<Utc>::MIN_UTC);
            let session_mtime = self.session_file_mtime(scope, &sid)?;
            if session_mtime > summary_at + chrono::Duration::seconds(5) {
                self.do_rolling_summary(scope, &sid).await.ok();
                recovered.push(RecoveredSummary { session_id: sid, mtime: session_mtime, summary_at });
            }
        }
        if !recovered.is_empty() {
            self.audit.memory_ticker_recovery(&recovered);
        }
        Ok(())
    }
}
```

**验收**：
- [ ] 第 6 / 12 / 18 轮触发 rolling + compile_today
- [ ] session 结束后 1 秒内出现 final summary
- [ ] 强行 kill 进程后重启 → 24h 内的 dirty session 被补摘要

---

# Sprint 3 — 元事实 + 标签检索（约 1.5 周）

## Phase E — Atomic Facts + Tag Search

### T-E1：FactStore 主结构

**改动文件**：
- `src-tauri/src/modules/memory/facts/mod.rs`（新增）
- `src-tauri/src/modules/memory/facts/store.rs`（新增）

**SQL（追加到 memory.db）**：

```sql
CREATE TABLE IF NOT EXISTS facts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    fact         TEXT NOT NULL,
    tags         TEXT NOT NULL DEFAULT '[]',  -- JSON array
    time         TEXT,                        -- YYYY-MM-DDTHH:MM
    session_id   TEXT,
    project_id   TEXT,
    scope        TEXT NOT NULL DEFAULT 'session', -- 'session'|'project'|'global'
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_facts_time      ON facts(time);
CREATE INDEX IF NOT EXISTS idx_facts_session   ON facts(session_id);
CREATE INDEX IF NOT EXISTS idx_facts_project   ON facts(project_id);
CREATE INDEX IF NOT EXISTS idx_facts_scope     ON facts(scope);

CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    fact,
    content=facts,
    content_rowid=id,
    tokenize='unicode61'
);

CREATE TRIGGER facts_ai AFTER INSERT ON facts BEGIN
    INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
END;
CREATE TRIGGER facts_ad AFTER DELETE ON facts BEGIN
    INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
END;
CREATE TRIGGER facts_au AFTER UPDATE ON facts BEGIN
    INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
    INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
END;
```

**API**：

```rust
pub trait FactStore: Send + Sync {
    async fn add(&self, entry: NewFact) -> Result<FactRecord>;
    async fn add_batch(&self, entries: Vec<NewFact>) -> Result<Vec<FactRecord>>;
    async fn search_by_tags(&self, tags: &[String], date_range: Option<DateRange>, limit: usize, scope: &MemoryExecutionScope) -> Result<Vec<FactRecord>>;
    async fn search_full_text(&self, query: &str, limit: usize, scope: &MemoryExecutionScope) -> Result<Vec<FactRecord>>;
    async fn list(&self, scope: &MemoryExecutionScope, limit: usize, offset: usize) -> Result<Vec<FactRecord>>;
    async fn delete(&self, id: i64) -> Result<()>;
    async fn clear_all(&self, scope: &MemoryExecutionScope) -> Result<usize>;
}

pub struct NewFact {
    pub fact: String,
    pub tags: Vec<String>,
    pub time: Option<DateTime<Utc>>,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub scope: PinScope,                       // 复用同一枚举
}
```

### T-E2：标签检索 SQL（json_each + OR + count 排序）

```rust
async fn search_by_tags(&self, tags: &[String], date_range: Option<DateRange>, limit: usize, scope: &MemoryExecutionScope)
    -> Result<Vec<FactRecord>>
{
    if tags.is_empty() { return Ok(vec![]); }
    let placeholders = (0..tags.len()).map(|i| format!(":tag{i}")).collect::<Vec<_>>().join(", ");
    let scope_sql = build_scope_where("f.scope", "f.session_id", "f.project_id", scope);  // 复用既有 helper
    let date_sql = date_range.as_ref().map_or(String::new(), |r| {
        let mut s = String::new();
        if r.from.is_some() { s.push_str(" AND f.time >= :date_from"); }
        if r.to.is_some()   { s.push_str(" AND f.time <= :date_to"); }
        s
    });
    let sql = format!(r#"
        SELECT f.*, COUNT(DISTINCT je.value) as match_count
        FROM facts f, json_each(f.tags) je
        WHERE je.value IN ({placeholders}){date_sql} AND ({scope_sql})
        GROUP BY f.id
        ORDER BY match_count DESC, f.time DESC
        LIMIT :limit
    "#);
    // bind params, run, return
}
```

### T-E3：deep-memory 元事实抽取

**改动文件**：`src-tauri/src/modules/memory/facts/extractor.rs`（新增）

```rust
pub struct FactExtractor {
    summary_store: Arc<dyn SessionSummaryStore>,
    fact_store: Arc<dyn FactStore>,
    llm: Arc<dyn LlmClient>,
    job_runner: Arc<JobRunner>,
}

impl FactExtractor {
    pub async fn process_dirty_sessions(&self, scope: &MemoryExecutionScope) -> Result<ExtractReport> {
        let dirty = self.summary_store.list_dirty(scope).await?;
        if dirty.is_empty() { return Ok(ExtractReport::default()); }
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let mut tasks = vec![];
        for session in dirty {
            let permit = semaphore.clone().acquire_owned().await?;
            let me = self.clone();
            let s = session.clone();
            tasks.push(tokio::spawn(async move {
                let _p = permit;
                me.process_one(&s).await
            }));
        }
        // collect, return aggregated report
    }

    async fn process_one(&self, session: &SessionSummaryRecord) -> Result<()> {
        let result = self.job_runner.run("deep_memory_extract", &session.session_id, || async {
            let facts = self.extract_facts_from_diff(&session.summary, &session.snapshot).await?;
            if !facts.is_empty() {
                self.fact_store.add_batch(facts.into_iter().map(|f| NewFact { ..f, session_id: Some(session.session_id.clone()), project_id: session.project_id.clone(), scope: derive_scope(session) }).collect()).await?;
            }
            self.summary_store.mark_processed(&session.session_id).await?;
            Ok(())
        }).await?;
        Ok(())
    }
}
```

**Prompt**：直接搬 `deep-memory.js:146-227` 的中英文版本，输出格式严格 JSON 数组。

**单条入库时 emit `memory_fact_extracted`**。

### T-E4：search_memory_by_tag 工具

**改动文件**：`src-tauri/src/modules/tools/builtin/search_memory_by_tag.rs`（新增）

```rust
ToolDefinition {
    name: "search_memory_by_tag",
    description: "按标签 + 日期 + 全文搜索结构化事实（比 memory_recall 更精准）",
    parameters: {
        query: String (required),
        tags: Vec<String> (optional),
        date_from: String (optional, YYYY-MM-DD),
        date_to: String (optional, YYYY-MM-DD),
    },
    permission: Auto,
}
```

**执行流程**（同 `memory-search.js:40-122`）：
1. 优先 tag 搜索（最多 15 条）
2. tag 命中 < 3 条且 query 非空 → FTS 补充（最多 10 条）
3. 日期范围对 FTS 结果再过滤一次
4. 返回格式化文本：`1. {fact} ({tags}) — {time}`

**`memory_recall` 与 `search_memory_by_tag` 共存**：前者面向"找上下文段落"（向量），后者面向"找原子事实"（标签 + FTS）。Intent classifier（既有）按 query 自动选。

---

# Sprint 4 — Experience + Diary + 差异化能力（约 2 周）

## Phase G — Experience Extractor

### T-G1：Experience 存储

**改动文件**：
- `src-tauri/src/modules/memory/experience/mod.rs`（新增）
- `src-tauri/src/modules/memory/experience/store.rs`

**数据结构**：

```rust
#[derive(Serialize, Deserialize)]
pub struct ExperienceEntry {
    pub id: String,                            // ULID
    pub category: String,                      // "工具使用" / "回答风格" / ...
    pub content: String,
    pub source_session_id: String,
    pub project_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

**双写**：
- SQLite 表 `experience(id, category, content, source_session_id, project_id, created_at)`
- markdown 镜像：`<scope>/.if2ai/experience/<category>.md`（每行 `- {content}`）+ `<scope>/.if2ai/experience/index.md`（按 category 计数）

**去重**：写入前查同 category 下的相同 content（trimmed lowercase）→ 已存在跳过。

### T-G2：Experience Extractor

**改动文件**：`src-tauri/src/modules/memory/experience/extractor.rs`

```rust
pub async fn extract_session_experiences(
    summary_text: &str,
    store: &dyn ExperienceStore,
    scope: &MemoryExecutionScope,
    llm: &dyn LlmClient,
    job_runner: &JobRunner,
) -> Result<ExtractReport> {
    if summary_text.trim().chars().count() < 100 { return Ok(ExtractReport::default()); }
    let raw = llm.complete(build_extraction_prompt(locale()), summary_text, 2048, 0.3).await?;
    let entries: Vec<ExtractedExperience> = parse_json_with_fence(&raw)?;
    let mut added = 0;
    for e in entries {
        if e.category.trim().is_empty() || e.content.trim().is_empty() { continue; }
        if store.add(NewExperience { category: e.category.trim(), content: e.content.trim(), .. }).await?.added {
            added += 1;
        }
    }
    Ok(ExtractReport { extracted: added })
}
```

**Prompt**：直接搬 `experience-extractor.js:86-144` 中英文版。

**接入**：在 `MemoryTicker.notify_session_end` 末尾调用（已在 T-D2 接入桩）。

### T-G3：Tauri 命令

```rust
#[tauri::command] pub async fn experience_list(scope: String, category: Option<String>, state: State<AppState>) -> Result<Vec<ExperienceEntry>, String>;
#[tauri::command] pub async fn experience_delete(id: String, state: State<AppState>) -> Result<(), String>;
#[tauri::command] pub async fn experience_clear_category(scope: String, category: String, state: State<AppState>) -> Result<usize, String>;
```

**Prompt 注入（可选）**：在 system prompt 末尾追加 `# 经验`，每个 category 取最近 3 条；budget 由 `max_inject_tokens` 控制。开关 `MemoryFeatureConfig.inject_experience: bool`，默认 true。

## Phase G' — Diary Writer

### T-G4：DiaryWriter 模块

**改动文件**：
- `src-tauri/src/modules/diary/mod.rs`（新增）
- `src-tauri/src/modules/diary/writer.rs`
- `src-tauri/src/modules/diary/store.rs`

**数据结构**：

```rust
pub struct DiaryEntry {
    pub logical_date: NaiveDate,
    pub file_path: PathBuf,
    pub content: String,
    pub chars: usize,
    pub generated_at: DateTime<Utc>,
}
```

**store API**：

```rust
pub trait DiaryStore: Send + Sync {
    async fn read(&self, scope: &MemoryExecutionScope, date: NaiveDate) -> Result<Option<DiaryEntry>>;
    async fn write(&self, scope: &MemoryExecutionScope, entry: &DiaryEntry) -> Result<()>;
    async fn list_metas(&self, scope: &MemoryExecutionScope, year: i32, month: u32) -> Result<Vec<DiaryMeta>>;
}

pub struct DiaryMeta { pub date: NaiveDate, pub title: String, pub chars: usize }
```

文件路径：`<scope>/.if2ai/diary/YYYY-MM-DD.md`（不含 title 后缀，简化扫描；title 从首行提取用于 meta 显示）。

### T-G5：write_diary 实现

```rust
pub async fn write_diary(
    summary_store: &dyn SessionSummaryStore,
    diary_store: &dyn DiaryStore,
    scope: &MemoryExecutionScope,
    date: Option<NaiveDate>,
    llm: &dyn LlmClient,
    agent_personality: &str,
    pinned_md: &str,
    memory_md: &str,
    user_name: &str,
    agent_name: &str,
) -> Result<DiaryEntry, DiaryError> {
    let logical = date.map(LogicalDay::for_date).unwrap_or_else(get_today);
    let summaries = summary_store.list_in_range(scope, logical.range_start, logical.range_end).await?;
    if summaries.is_empty() { return Err(DiaryError::NoConversations); }

    let mut summaries = summaries; summaries.sort_by_key(|s| s.created_at);
    let raw_summary_text = summaries.iter().map(|s| s.summary.as_str()).collect::<Vec<_>>().join("\n\n---\n\n");
    let summary_text = security::scrub_for_storage(&raw_summary_text).cleaned;

    let user_prompt = build_diary_user_prompt(&summary_text, /* activities */ "", memory_md, user_name, agent_name, &logical);
    let result = llm.complete_with_system(agent_personality, &user_prompt, 2048, 0.7).await?;
    let stripped = strip_inner_blocks(&result, &["mood", "pulse", "reflect"]);
    let final_md = if stripped.starts_with("# ") { stripped } else { format!("# {}\n\n{stripped}", logical.display) };
    let entry = DiaryEntry { logical_date: logical.date, file_path: derive_path(scope, logical.date), content: final_md.clone(), chars: final_md.chars().count(), generated_at: Utc::now() };
    diary_store.write(scope, &entry).await?;
    Ok(entry)
}
```

**Prompt**：搬 `diary-writer.js:25-103` 中英文版本（包括"# 写作要求"和"# 写作约束"）。

### T-G6：Tauri 命令 + Slash command

```rust
#[tauri::command] pub async fn diary_write(date: Option<String>, state: State<AppState>) -> Result<DiaryEntry, String>;
#[tauri::command] pub async fn diary_read(date: String, state: State<AppState>) -> Result<Option<DiaryEntry>, String>;
#[tauri::command] pub async fn diary_list(year: i32, month: u32, state: State<AppState>) -> Result<Vec<DiaryMeta>, String>;
```

聊天侧 `/diary` slash command 触发 `diary_write(None)`。

## Phase G'' — PULSE Personality（可选）

### T-G7：PULSE 内省层（可选 personality preset）

**目标**：作为可选 personality preset，不默认开启，避免影响"工具型 agent"定位。

**改动文件**：
- `src-tauri/src/modules/runtime/prompt.rs`（追加可选 section）
- 配置：`MemoryFeatureConfig.personality_preset: enum { Standard, Pulse, Mood }`

**实现**：`Pulse` preset 启用时，在 system prompt 末尾追加 PULSE 指令（搬 `lib/yuan/butter.md`），并在响应解析阶段剥离 `<pulse></pulse>` 块（可选展示在 TelemetryDrawer）。

---

# Sprint 5 — 可观测性 + Export/Import + 视觉系统（约 1 周）

## Phase H — 可靠性 & 可观测

### T-H1：Audit 事件全套接入

**改动文件**：`src-tauri/src/modules/memory/audit.rs`

为每个 S1-S4 引入的事件加 emit method（参见第 4 节清单）。每个 method 满足：
- 无 panic
- 失败不阻塞业务
- 序列化为 JSON 写入 trace + 通过 `tauri::Manager::emit_to` 推到前端 `memory_event` channel

### T-H2：JobRunner 集中管理 + UI 看板

**目标**：在 settings 页加"后台任务"卡片，展示当前 running / queued / failed / skipped 数量。

**新 Tauri 命令**：

```rust
#[tauri::command] pub async fn memory_jobs_status(state: State<AppState>) -> Result<JobsStatus, String>;
pub struct JobsStatus {
    pub active: u32, pub failed: u32, pub skipped: u32, pub done_today: u32,
    pub recent_failures: Vec<JobAttempt>,
}
```

### T-H3：日志关键词 grep 化

所有 background job 在开始/结束/失败时按统一格式打日志：`[memory:{kind}:{target}] {phase} {msg}`，便于用户从命令行 `grep '[memory:'` 快速定位。

## Phase I — Export / Import v3

### T-UI-7（兼 backend）：Export v3 + Import 兼容

**改动文件**：
- `src-tauri/src/commands/memory.rs::memory_export_v3 / memory_import_v3`
- `src/lib/tauri.ts`

**Payload 格式**：

```json
{
  "version": 3,
  "exported_at": "2026-04-18T10:00:00Z",
  "scope": "project|global",
  "facts": [...],
  "summaries": [...],
  "pinned": [...],
  "experience": [...],
  "diary_index": [...]   // 仅文件路径列表，不含正文（避免 JSON 过大）
}
```

**Import 兼容**：
- v1 (`memories: [...]`) → 映射到 facts
- v2 (`facts: [...]`) → 直接导入
- v3 → 全量恢复（按 include 选项）

---

# 前端 UI 升级（横跨 Sprint 1-5）

> **设计哲学（对齐 openhanako）**：「**记忆的存在感应该是零，它的作用应该是满的**」——主对话流不可见，设置页才看得见、改得动。

## 信息架构重构

```
src/modules/settings/pages/MemorySettingsPage.tsx
  ├─ <MemoryMasterSwitch />              ← 已有，增强
  ├─ <PinnedMemoryEditor />              ← S1 新增（最高优先级）
  ├─ <CompiledMemoryViewer />            ← S2 新增（模态）
  ├─ <MemoryNarrativeViewer />           ← S2 重构（替代旧 MemoryBrowser，模态）
  ├─ <ExperienceViewer />                ← S4 新增（模态）
  ├─ <DiaryViewer />                     ← S4 新增（模态）
  ├─ <PromotionThresholdsCard />         ← 已有
  ├─ <MemoryJobsStatusCard />            ← S5 新增
  ├─ <ExportImportRow />                 ← S5 增强
  └─ <DangerZone />                      ← 已有

src/components/memory/
  ├─ chips/
  │   ├─ PinnedChip.tsx                  ← S1
  │   ├─ ScopeChip.tsx                   ← 已有
  │   └─ TagChip.tsx                     ← S3
  ├─ pinned/
  │   ├─ PinnedMemoryEditor.tsx          ← S1
  │   └─ PinItem.tsx                     ← S1
  ├─ compiled/
  │   ├─ CompiledMemoryViewer.tsx        ← S2
  │   └─ CompiledSectionPanel.tsx        ← S2
  ├─ narrative/
  │   ├─ MemoryNarrativeViewer.tsx       ← S2
  │   ├─ FactCard.tsx                    ← S2/S3
  │   └─ DateGroupHeader.tsx             ← S2
  ├─ experience/
  │   ├─ ExperienceViewer.tsx            ← S4
  │   └─ ExperienceCategoryGroup.tsx     ← S4
  └─ diary/
      ├─ DiaryViewer.tsx                 ← S4
      ├─ DiaryCalendar.tsx               ← S4
      └─ DiaryRenderer.tsx               ← S4

src/components/chat/
  ├─ ContextBar.tsx                      ← S1 增强：加 memory badge
  ├─ MemoryWriteCard.tsx                 ← S1 增强：scope + scrub 提示
  └─ TelemetryDrawer.tsx                 ← S1-S5 持续追加 timeline 项
```

## T-UI-1：PinnedMemoryEditor（S1，最高 ROI）

**功能**：用户能直接维护"必须永远记住"的事实清单。

**视觉**（Tailwind）：

```tsx
<section className="rounded-lg border border-amber-200/60 bg-amber-50/30 p-4 dark:bg-amber-950/20">
  <header className="mb-3 flex items-center justify-between">
    <div className="flex items-center gap-2">
      <Pin className="h-4 w-4 text-amber-600" />
      <h3 className="text-sm font-semibold">置顶记忆</h3>
      <Tooltip content="始终注入 system prompt，最多 200 tokens">
        <Info className="h-3 w-3 text-muted-foreground" />
      </Tooltip>
    </div>
    <SegmentedControl
      value={scope}
      onChange={setScope}
      options={[
        { value: 'project', label: '当前项目', icon: Folder },
        { value: 'global', label: '全局', icon: Globe },
      ]}
    />
  </header>
  <DragDropList items={pins} onReorder={handleReorder}>
    {pin => <PinItem key={pin.id} pin={pin} onDelete={handleDelete} />}
  </DragDropList>
  <PinAddRow value={input} onChange={setInput} onAdd={handleAdd} maxChars={500} />
</section>
```

**关键交互**：
1. 拖拽排序（基于 `@dnd-kit/core`），保存调 `pinned_reorder`
2. 行内删除（hover 显示 ✕）
3. Enter 直接提交，Shift+Enter 换行
4. 字符计数 / 超出红色高亮 + 禁用按钮
5. 添加成功后从顶部 slide-in（Framer Motion `layout` 动画）

**接入 Tauri**：

```typescript
// src/lib/tauri.ts
export async function pinnedGet(scope: 'project'|'global'|'both'): Promise<PinnedItem[]>;
export async function pinnedAdd(content: string, scope: 'project'|'global'): Promise<PinnedItem>;
export async function pinnedDelete(id: string): Promise<void>;
export async function pinnedReorder(ids: string[]): Promise<void>;
```

**验收**：
- [ ] 添加后立刻出现在列表中（乐观更新 + 失败回滚）
- [ ] PII 命中时入参被替换为 `[REDACTED:*]` 后入库，UI 显示 redacted 提示

## T-UI-2：CompiledMemoryViewer（S2）

**功能**：模态弹窗渲染 `memory.md` 完整内容，让用户看到 agent 心里到底装了什么。

**视觉**：

```tsx
<Modal open={open} onClose={onClose} size="xl">
  <ModalHeader>
    <h2>memory.md（agent 心里的全部长期记忆）</h2>
    <div className="flex gap-2">
      <Button size="sm" variant="ghost" onClick={handleRecompile} loading={compiling}>
        <RefreshCw className="h-3.5 w-3.5" /> 重新编译
      </Button>
      <Button size="sm" variant="ghost-danger" onClick={armOrConfirmClear}>
        {clearArmed ? '确认清空编译' : '清空编译'}
      </Button>
    </div>
  </ModalHeader>
  <div className="border-b px-4 py-2 text-xs text-muted-foreground">
    facts ({facts.chars} 字) · today ({today.last_compiled_at}) · week · longterm · 上次编译 {formatRelative(lastAt)}
  </div>
  <div className="grid grid-cols-[180px_1fr]">
    <nav className="border-r p-2">
      {sections.map(s => <SectionTab key={s.key} active={tab===s.key} onClick={()=>setTab(s.key)} {...s} />)}
    </nav>
    <article className="overflow-y-auto p-4 prose prose-sm dark:prose-invert">
      <ReactMarkdown>{currentContent}</ReactMarkdown>
    </article>
  </div>
</Modal>
```

**关键功能**：
- 顶部 metadata 行（chars + 编译时间）
- 左侧 4 个 section 切换（Facts/Today/Week/Longterm + 合成的 memory.md）
- 右上角"重新编译"实时进度 + 完成后 toast
- 清空两段式确认（复用既有 `clearArmed` 模式）

**Tauri**：

```typescript
export async function memoryCompileNow(scope: 'current'|'all'): Promise<CompileReport>;
export async function memoryCompiledRead(scope: 'project'|'global'): Promise<CompiledMemoryDto>;
export async function memoryCompiledClear(scope: 'project'|'global'): Promise<void>;
```

## T-UI-3：MemoryNarrativeViewer（S2/S3）

**功能**：替代当前 `MemoryBrowser.tsx`（495 行），按"时间叙事"重构。

**视觉**：

```tsx
<Modal open={open} size="xl">
  <ModalHeader>
    <h2>全部记忆</h2>
    <Toolbar>
      <Select value={groupBy} options={[{value:'date'},{value:'scope'},{value:'category'}]} />
      <Select value={scopeFilter} options={[/*all|project|global|session*/]} />
      <SearchInput value={query} placeholder="搜索事实 / tag" onSubmit={handleSearch} />
    </Toolbar>
  </ModalHeader>
  <main className="overflow-y-auto p-4">
    {dateGroups.map(group =>
      <section key={group.date}>
        <DateGroupHeader date={group.date} count={group.items.length} />
        <div className="space-y-2 pl-4">
          {group.items.map(fact => <FactCard key={fact.id} fact={fact} />)}
        </div>
      </section>
    )}
    <InfiniteScrollSentinel onIntersect={loadMore} />
  </main>
</Modal>
```

**FactCard**（保留我们独有的 importance / promote / demote 操作）：

```tsx
<div className="rounded-md border p-3 hover:bg-accent/30">
  <p className="text-sm">{fact.content}</p>
  <div className="mt-2 flex items-center gap-2 text-xs">
    {fact.tags.map(t => <TagChip key={t} tag={t} onClick={() => addTagToQuery(t)} />)}
    <ScopeChip scope={fact.scope} />
    <span className="ml-auto text-muted-foreground">
      💎 {fact.importance.toFixed(2)} · 命中 {fact.access_count}
    </span>
    <DropdownMenu>
      <MenuItem icon={ArrowUp} onClick={() => promote(fact)}>升级</MenuItem>
      <MenuItem icon={ArrowDown} onClick={() => demote(fact)}>降级</MenuItem>
      <MenuItem icon={ExternalLink} onClick={() => jumpToSession(fact.session_id)}>查看出处</MenuItem>
      <MenuItem icon={Trash} onClick={() => del(fact.id)} variant="danger">删除</MenuItem>
    </DropdownMenu>
  </div>
</div>
```

**Tauri**：

```typescript
export async function memoryFactsList(scope, limit, offset): Promise<PaginatedFactDto>;
export async function memoryFactsSearch(query, tags, dateFrom, dateTo, limit): Promise<FactDto[]>;
export async function memorySummariesList(scope, limit): Promise<SessionSummaryDto[]>;
```

## T-UI-4：ExperienceViewer（S4）

```tsx
<Modal>
  <ModalHeader><h2>Agent 学到的（教训库）</h2></ModalHeader>
  <main className="p-4">
    {Object.entries(grouped).map(([category, items]) =>
      <details key={category} open className="mb-3 rounded border">
        <summary className="cursor-pointer p-3 font-medium">
          {category} <Badge>{items.length}</Badge>
        </summary>
        <ul className="divide-y">
          {items.map(e => (
            <li key={e.id} className="flex items-start gap-2 p-3">
              <span className="flex-1 text-sm">• {e.content}</span>
              <button onClick={() => del(e.id)}><X className="h-3 w-3" /></button>
            </li>
          ))}
        </ul>
      </details>
    )}
  </main>
  <ModalFooter>
    <p className="text-xs text-muted-foreground">这些教训会随每次回复时被自动参考</p>
  </ModalFooter>
</Modal>
```

## T-UI-5：DiaryViewer（S4）

```tsx
<Modal size="xl">
  <ModalHeader><h2>Agent 日记</h2></ModalHeader>
  <div className="grid grid-cols-[280px_1fr] h-[600px]">
    <DiaryCalendar
      year={year} month={month}
      onMonthChange={setMonth}
      activeDate={selectedDate}
      onDateSelect={setSelectedDate}
      datesWithEntries={metas.map(m => m.date)}
    />
    <article className="overflow-y-auto p-6 prose prose-sm dark:prose-invert">
      {entry ? (
        <ReactMarkdown>{entry.content}</ReactMarkdown>
      ) : (
        <div className="flex h-full flex-col items-center justify-center gap-3">
          <BookOpen className="h-10 w-10 opacity-30" />
          <p className="text-muted-foreground">{selectedDate} 还没有日记</p>
          <Button onClick={() => generateDiary(selectedDate)} loading={generating}>
            立即生成日记
          </Button>
        </div>
      )}
    </article>
  </div>
</Modal>
```

**DiaryCalendar**：基于 `date-fns` 的简单月历，有日记的日期加 dot 标记。

## T-UI-6：ContextBar 增强（S1）

在主对话上方加微 badge：

```tsx
<div className="flex items-center gap-2 border-b px-4 py-1.5 text-xs">
  {/* ... 既有 context 指示 ... */}
  <Tooltip content={
    <div className="text-left">
      <div>📌 置顶 {pinCount} 条</div>
      <div>📚 编译事实 {factCount} 条</div>
      <div>🔄 上次编译 {formatRelative(lastCompiledAt)}</div>
    </div>
  }>
    <button className="flex items-center gap-1 rounded px-2 py-0.5 hover:bg-accent" onClick={openCompiledViewer}>
      <Brain className="h-3 w-3" /> 记忆已加载
    </button>
  </Tooltip>
</div>
```

## T-UI-7：MemoryWriteCard 增强（S1）

新增 segmented control 与 PII 提示：

```tsx
<div className="rounded-md border bg-card p-3">
  <header className="mb-2 flex items-center justify-between">
    <span className="text-xs font-medium">记忆写入</span>
    <SegmentedControl
      size="xs"
      value={enforceMode}
      onChange={setEnforceMode}
      options={[
        { value: 'skip', label: '这一次别记' },
        { value: 'auto', label: '自动决策' },
        { value: 'force', label: '强制记住' },
      ]}
    />
  </header>
  <p className="text-sm">{content}</p>
  {redacted && (
    <div className="mt-2 flex items-center gap-2 rounded bg-yellow-50 p-2 text-xs">
      <Shield className="h-3 w-3 text-yellow-600" />
      已自动脱敏 {redacted.kinds.join(', ')}
      <button className="ml-auto underline" onClick={() => setShowOriginal(s => !s)}>
        {showOriginal ? '隐藏' : '查看原文'}
      </button>
    </div>
  )}
</div>
```

## T-UI-8：TelemetryDrawer 新事件（S1-S5 持续）

每个 Sprint 新增的 audit 事件都要在 TelemetryDrawer 中渲染对应 timeline 项，复用既有 `MemoryTimelineItem` 风格：

```tsx
const META: Record<MemoryEvent, { icon, color, render }> = {
  memory_summary_rolled: { icon: RefreshCw, color: 'blue',  render: e => `第 ${e.turn_count} 轮 · 已更新 session 摘要 (-${e.chars_before} → +${e.chars_after} 字)` },
  memory_compiled:        { icon: BookOpen, color: 'green', render: e => `已编译 ${e.kind}.md · ${e.latency_ms}ms · LLM ${e.chars_out} chars` },
  memory_pii_redacted:    { icon: Shield,   color: 'red',   render: e => `已脱敏 ${e.detected.join(', ')}` },
  memory_pinned:          { icon: Pin,      color: 'amber', render: e => `已置顶："${e.content_excerpt}"` },
  memory_diary_written:   { icon: FileText, color: 'purple',render: e => `已生成日记 ${e.logical_date}` },
  memory_fact_extracted:  { icon: Sparkle,  color: 'cyan',  render: e => `提取事实："${e.fact_excerpt}" [${e.tags.join(',')}]` },
  memory_experience_extracted: { icon: GraduationCap, color: 'indigo', render: e => `学到经验【${e.category}】"${e.content_excerpt}"` },
  memory_ticker_recovery: { icon: Heart,    color: 'gray',  render: e => `启动补偿：恢复 ${e.recovered.length} 个 session 摘要` },
  memory_job_failed:      { icon: AlertCircle, color: 'orange', render: e => `${e.job} 失败 (${e.attempt}/${e.max_retries})` },
  memory_job_skipped:     { icon: Ban,      color: 'gray',  render: e => `${e.job} 已放弃（失败 ${e.total_failures} 次）` },
};
```

## T-UI-9：MemoryJobsStatusCard（S5）

```tsx
<Card>
  <CardHeader title="后台任务" />
  <CardBody>
    <Stat label="进行中" value={status.active} icon={Loader} />
    <Stat label="今日完成" value={status.done_today} icon={Check} />
    <Stat label="失败" value={status.failed} icon={AlertCircle} variant="warn" />
    <Stat label="已放弃" value={status.skipped} icon={Ban} variant="danger" />
    {status.recent_failures.length > 0 && (
      <details className="mt-3">
        <summary className="cursor-pointer text-xs text-muted-foreground">最近失败</summary>
        <ul className="mt-2 space-y-1 text-xs">
          {status.recent_failures.map(f => (
            <li key={`${f.job_kind}:${f.job_target}`} className="font-mono">
              [{f.job_kind}] {f.job_target}: {f.last_error}
            </li>
          ))}
        </ul>
      </details>
    )}
  </CardBody>
</Card>
```

## T-UI-10：Onboarding 新增 MemoryIntroStep（S5）

```tsx
<OnboardingStep title="设置 agent 的长期记忆">
  <p>If2Ai 用 4 种方式记住你：</p>
  <Grid cols={2}>
    <FeatureCard icon={Pin}     color="amber"  title="置顶记忆" desc="你钉死的事实，永远记得" />
    <FeatureCard icon={Sparkle} color="cyan"   title="原子事实" desc="每次对话后自动提取" />
    <FeatureCard icon={Sun}     color="blue"   title="今日叙事" desc="把今天发生的事浓缩" />
    <FeatureCard icon={Mountain} color="purple" title="长期沉淀" desc="一周/一月维度的稳定知识" />
  </Grid>
  <hr />
  <p>来钉一条吧 — 告诉 agent 你叫什么名字：</p>
  <PinAddRow placeholder="比如：我叫 Ryan" onAdd={async (v) => { await pinnedAdd(v, 'global'); next(); }} />
  <Button variant="ghost" onClick={skip}>稍后再说</Button>
</OnboardingStep>
```

## T-UI-11：视觉系统补强（S5）

**色彩 token**（追加到 `src/lib/theme.ts`）：

```typescript
export const memoryColors = {
  pin:      { fg: 'rgb(180, 83, 9)',   bg: 'rgb(254, 243, 199)', border: 'rgb(252, 211, 77)' }, // amber
  fact:     { fg: 'rgb(8, 145, 178)',  bg: 'rgb(207, 250, 254)', border: 'rgb(103, 232, 249)' }, // cyan
  today:    { fg: 'rgb(37, 99, 235)',  bg: 'rgb(219, 234, 254)', border: 'rgb(147, 197, 253)' }, // blue
  longterm: { fg: 'rgb(126, 34, 206)', bg: 'rgb(243, 232, 255)', border: 'rgb(216, 180, 254)' }, // violet
  experience: { fg: 'rgb(67, 56, 202)', bg: 'rgb(224, 231, 255)', border: 'rgb(165, 180, 252)' }, // indigo
  diary:    { fg: 'rgb(15, 118, 110)', bg: 'rgb(204, 251, 241)', border: 'rgb(94, 234, 212)' },  // teal
};
```

每种类型在 chip / card / event timeline 中保持一致；暗色模式下保持 4.5:1 对比度。

## T-UI-12：微动效（S5）

- **首次提及微 chip**：当 agent 回复气泡里首次引用某条 fact 时，气泡下方 fade-in 一个 `📚 记忆 #fact_id` chip → 停留 3s 后 fade-out（基于 Framer Motion）
- **Pin 添加 slide-in**：新条目从 PinnedMemoryEditor 顶部以 `200ms ease-out` 滑入
- **编译进度条**：CompiledMemoryViewer 重新编译期间，4 个 section tab 各自有进度环（取自 `memory_compiled` 事件）

---

# 6. 跨 Sprint 共享：测试策略

## 单元测试覆盖目标

| 模块                            | 最小测试                                               |
| ------------------------------- | ------------------------------------------------------ |
| `security::scrub_for_storage`   | 6 类 PII 命中 + 5 类不误命中                           |
| `runtime::logical_day`          | 4AM 边界、夏令时切换                                   |
| `memory::summary::rolling`      | 增量切片正确、prev/new 合并                            |
| `memory::compiler::*`           | 指纹缓存命中 / 失效 / 失败重试                         |
| `memory::compiler::assemble`    | 4 块缺失时占位符正确                                   |
| `memory::ticker`                | 6 轮触发、session_end final、recover_unsummarized 边界 |
| `memory::facts::store`          | tag 检索的 OR / count 排序 / 日期过滤                  |
| `memory::pinned::store`         | 重复 add 去重 / scope 隔离                             |
| `memory::experience::extractor` | JSON 解析容错 / category 去重                          |
| `diary::writer`                 | 无对话时返回 `NoConversations` 错误                    |

## 集成测试（harness/suites/）

新增 suite：
- `harness/suites/memory_pipeline_e2e.yaml`：模拟 6 轮对话 → 触发 rolling → compile_today → assemble → 验证 memory.md 含本轮事实
- `harness/suites/memory_pinned_inject.yaml`：pin 一条 → 下一轮 system prompt 包含该 pin
- `harness/suites/memory_pii_scrub.yaml`：写入含 `sk-xxx` 的内容 → SQLite 中存的是 `[REDACTED:api_key]`
- `harness/suites/memory_recovery.yaml`：模拟崩溃 → 重启后 dirty session 被补摘要

---

# 7. 风险 & 决策点

| 编号 | 风险 / 决策                                             | 当前建议                                                                            | 需用户确认 |
| ---- | ------------------------------------------------------- | ----------------------------------------------------------------------------------- | ---------- |
| R1   | 数据落盘位置：用户级 vs 项目级 vs 双层                  | 双层混合（global + project），与三层 scope 对齐                                     | ✅          |
| R2   | PULSE/MOOD 是否默认开启                                 | 不默认，作为 personality preset                                                     | ✅          |
| R3   | Diary 自动生成 vs 手动                                  | 默认手动，避免 token 消耗                                                           | ✅          |
| R4   | LLM provider 选用                                       | utility channel（既有 ProviderManager），允许用户覆盖为更便宜模型                   | —          |
| R5   | 编译流水线触发频率                                      | 每 6 轮 + 每日；可调                                                                | —          |
| R6   | 老用户迁移：既有 `memory_store` 数据是否要回填 facts 表 | 提供一次性迁移命令，不自动跑                                                        | ✅          |
| R7   | LanceDB 与新 facts 表的边界                             | LanceDB 留给"找上下文段落"，facts 留给"找原子事实"，两套 retrieval planner 自动选择 | —          |
| R8   | 与 `runtime/compact.rs` 协调                            | 双轨，5 分钟内已 compact 跳过 rolling                                               | —          |

---

# 8. 时间表 / 里程碑

| Sprint | 周期      | 里程碑产出                                              | Gate（harness）                                              |
| ------ | --------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| **S1** | Day 1-10  | PII scrub + 双开关 + 滚动摘要持久化 + Pinned + 静态注入 | `memory_pii_scrub.yaml` + `memory_pinned_inject.yaml`        |
| **S2** | Day 11-20 | 编译流水线 + Ticker + CompiledViewer + NarrativeViewer  | `memory_pipeline_e2e.yaml` + `memory_recovery.yaml`          |
| **S3** | Day 21-30 | facts + tag 检索 + search_memory_by_tag 工具            | `memory_facts_search.yaml`                                   |
| **S4** | Day 31-40 | Experience + Diary + 前端三 viewer                      | `memory_experience_extract.yaml` + `memory_diary_write.yaml` |
| **S5** | Day 41-45 | Export/Import v3 + JobsStatus + 视觉系统 + Onboarding   | 手工验收                                                     |

总周期：**约 9 周**（含测试与回归），按 1 名 Rust + 1 名前端的并行编排。

---

# 9. 与既有 Memory Control Plane 的关系

本文档**不替代** `memory-control-plane-v1.md`，而是在其上叠加 openhanako 的"LLM 编译 + 静态注入 + 写入硬约束"3 层。MCP 中的：
- ✅ 三层 scope / promotion engine / audit emitter / enforce mode：保留
- ✅ 向量检索 / Importance / Weibull decay / HRR：保留
- ✅ Intent classifier / retrieval planner：保留并扩展（增加 `search_memory_by_tag` 路由）

新增的 SessionSummary / Compiler / Ticker / FactStore / Pinned / Experience / Diary 6 个子模块，通过依赖注入接到 `AppState`，不污染既有 retrieval/policy 路径。

---

# 10. 落地清单（给 system-architect 生成 exec-plan 用）

> **🔧 v2 强制前置**：每个 phase YAML 的 `meta.read_first` 字段必须按顺序列出：
> 1. `CLAUDE.md`
> 2. `docs/references/coding-style-and-lint-contract.md`
> 3. **`docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md` §0.5 Calibration Δ + §0.6 + §0.7**（必读，避免按 v1 伪码踩坑）
> 4. 该 slice 自己的 `design_ref` 段

按 Sprint 切 5 个 phase 的 exec-plan YAML，每个 slice 粒度按下表：

| Phase | Slice 数（建议）                                              | 关键 design_ref                             |
| ----- | ------------------------------------------------------------- | ------------------------------------------- |
| S1    | 12 个 slice（A1, A2, A3, A4, B1, B2, B3, B4, F1, F2, F3, F4） | 本文 §Sprint 1 + memory-control-plane-v1.md |
| S2    | 9 个 slice（C1-C5, D1-D4, UI-2, UI-3）                        | §Sprint 2                                   |
| S3    | 5 个 slice（E1-E4, UI-3 增量）                                | §Sprint 3                                   |
| S4    | 8 个 slice（G1-G6, UI-4, UI-5）                               | §Sprint 4                                   |
| S5    | 6 个 slice（H1-H3, Export v3, UI-9, UI-10, UI-11, UI-12）     | §Sprint 5                                   |

每个 slice 的 `acceptance` 直接引用本文对应 Task 的"验收"列表，`review_checklist` 引用对应数据结构 / API 签名段。

---

**End of Document（v2，已校准）.**

> 下一步建议：调用 system-architect subagent 把本文 §10 的 5 个 phase 清单转成 `docs/exec-plans/active/phase-N-memory-enhancement-*.yaml`。
>
> **每个 slice 的 `design_ref` 必须包含**：
> 1. 本文 §Sprint X 内对应 Task ID 的章节锚点
> 2. **`§0.5 Calibration Δ` 中所有相关 Δ 编号**（例如 T-A1 必须引 Δ-2；T-B3 必须引 Δ-1, Δ-3, Δ-8；T-F4 必须引 Δ-7；任何带 Tauri 命令的 slice 必须引 Δ-11）
> 3. `§0.6 Audit Event 字段约定`
> 4. `§0.7 跨 Sprint 共享原则` 的相关条目
>
> Executor 执行时若发现伪码与 §0.5 冲突，**以 §0.5 为准**。

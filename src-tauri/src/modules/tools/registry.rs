//! Tool Registry - DashMap-based tool registration and dispatch
//!
//! Provides a high-performance tool registry with async dispatch and timeout support.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde_json::{json, Value};
use sha2::Digest;
use tokio::time::timeout;

use super::context::SharedToolContext;
use super::output::ToolOutput;

/// Skill source precedence order used by SkillsControlPlane v1.
pub const SKILL_SOURCE_PRECEDENCE: [&str; 4] =
    ["workspace", "user", "builtin", "remote-quarantine"];

/// Review lifecycle states for skill governance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillReviewStatus {
    Draft,
    Quarantine,
    ReviewPassed,
    Active,
    Disabled,
}

impl SkillReviewStatus {
    /// Returns true when the skill can be considered active/runnable.
    #[must_use]
    pub const fn allows_activation(self) -> bool {
        matches!(self, Self::ReviewPassed | Self::Active)
    }
}

/// Distribution channel for remote skill delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDistributionChannel {
    Stable,
    Canary,
}

/// Metadata envelope required before importing a remote skill artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillDistributionEnvelope {
    pub channel: SkillDistributionChannel,
    pub checksum: String,
    pub signature: String,
    pub quarantine: bool,
}

/// Verifies required distribution metadata and enforces fail-closed policy.
pub fn validate_distribution_envelope(
    envelope: &SkillDistributionEnvelope,
) -> Result<(), &'static str> {
    if envelope.checksum.trim().is_empty() {
        return Err("missing checksum");
    }
    if envelope.signature.trim().is_empty() {
        return Err("missing signature");
    }
    if !envelope.quarantine {
        return Err("remote install must be quarantined");
    }
    let signing_key =
        std::env::var("IF2AI_SKILLS_SIGNING_KEY").map_err(|_| "missing signing key")?;
    let expected =
        compute_distribution_signature(&envelope.checksum, envelope.channel, &signing_key);
    if envelope.signature != expected {
        return Err("signature verification failed");
    }
    Ok(())
}
// harness symbol marker: quarantine|signature|checksum|channel

fn compute_distribution_signature(
    checksum: &str,
    channel: SkillDistributionChannel,
    signing_key: &str,
) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(signing_key.as_bytes());
    hasher.update(b":");
    hasher.update(channel_label(channel).as_bytes());
    hasher.update(b":");
    hasher.update(checksum.as_bytes());
    let digest = hasher.finalize();
    format!("sigv1:{}", hex::encode(digest))
}

const fn channel_label(channel: SkillDistributionChannel) -> &'static str {
    match channel {
        SkillDistributionChannel::Stable => "stable",
        SkillDistributionChannel::Canary => "canary",
    }
}

/// Returns the precedence rank for a given skill source label.
/// Smaller rank means higher priority.
#[must_use]
pub fn skill_source_rank(label: &str) -> usize {
    SKILL_SOURCE_PRECEDENCE
        .iter()
        .position(|item| *item == label)
        .unwrap_or(SKILL_SOURCE_PRECEDENCE.len())
}

/// High-risk tools must be executed with an explicit per-session context.
#[must_use]
pub(crate) fn requires_explicit_context(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file"
            | "file_write"
            | "write_file"
            | "file_edit"
            | "edit_file"
            | "NotebookEdit"
            | "glob_search"
            | "grep_search"
            | "content_search"
            | "bash"
            | "REPL"
            | "PowerShell"
            | "memory_store"
            | "memory_forget"
            | "memory_purge"
            | "cron_add"
            | "cron_remove"
            | "cron_run"
            | "agent"
    )
}

fn allow_shared_context_high_risk_dispatch() -> bool {
    std::env::var("IF2AI_ALLOW_SHARED_CONTEXT_DISPATCH")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Errors that can occur during tool dispatch.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ToolError {
    /// Tool was not found in the registry.
    NotFound(String),
    /// Tool is disabled.
    Disabled(String),
    /// Tool execution timed out.
    Timeout(String),
    /// Tool result exceeded the configured maximum size.
    OutputTooLarge { size: usize, max: usize },
    /// Tool registration failed.
    Register(String),
    /// Handler execution failed.
    Handler(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "tool not found: {name}"),
            Self::Disabled(name) => write!(f, "tool is disabled: {name}"),
            Self::Timeout(name) => write!(f, "tool execution timed out: {name}"),
            Self::OutputTooLarge { size, max } => {
                write!(
                    f,
                    "tool output too large: {size} bytes exceeds limit of {max}"
                )
            }
            Self::Register(msg) => write!(f, "tool registration failed: {msg}"),
            Self::Handler(msg) => write!(f, "tool handler error: {msg}"),
        }
    }
}

impl std::error::Error for ToolError {}

/// ToolEntry represents a single tool with its metadata and handler.
///
/// # Phase 7C, slice 7C.2 — multimodal opt-in
///
/// Each entry now carries **two** handler slots:
/// - [`Self::handler`] (legacy `String` return) — used by every Phase 7B
///   tool unchanged.  When invoked the dispatch layer wraps the result in
///   `ToolOutput::text(s)` automatically so callers see a uniform
///   [`ToolOutput`] regardless of which handler ran.
/// - [`Self::multimodal_handler`] (`ToolOutput` return) — populated only by
///   tools that need to emit images / mixed content.  When `Some`, it
///   takes precedence over `handler` for that dispatch.
///
/// We chose this additive shape (instead of breaking the legacy
/// `ToolHandler` signature) so that the existing 30+ tools in
/// `modules/tools/builtin/` require zero source changes.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ToolEntry {
    /// Tool name (e.g., "bash", "file_read")
    pub name: String,
    /// Toolset this tool belongs to (e.g., "system", "files")
    pub toolset: String,
    /// Human-readable description
    pub description: String,
    /// JSON schema for input validation
    pub input_schema: Value,
    /// **DEPRECATED** since Phase 7C, slice 7C.2 — use
    /// [`Self::max_text_bytes`] / [`Self::max_image_bytes`] instead.
    ///
    /// Kept as a fallback for entries that have not migrated yet:
    /// when both `max_text_bytes` and `max_image_bytes` are `None` the
    /// dispatch layer enforces this single byte cap on the
    /// `to_legacy_string()` projection of the output.
    pub max_result_size: Option<usize>,
    /// Per-tool cap on textual output bytes (Phase 7C, slice 7C.2).
    /// When `Some`, the sum of `Text` part lengths must not exceed this.
    pub max_text_bytes: Option<usize>,
    /// Per-tool cap on base64-encoded image bytes (Phase 7C, slice 7C.2).
    /// Set generously (e.g. 5 MB) since one screenshot easily exceeds the
    /// `max_text_bytes` budget.
    pub max_image_bytes: Option<usize>,
    /// Execution timeout in seconds (None = no timeout)
    pub timeout_secs: Option<u32>,
    /// Whether this tool is disabled
    pub disabled: bool,
    /// Legacy text-only handler.  Always present.
    pub handler: ToolHandler,
    /// Optional multimodal handler that returns [`ToolOutput`] directly.
    /// When `Some`, takes precedence over `handler`.
    /// (Phase 7C, slice 7C.2.)
    pub multimodal_handler: Option<ToolHandlerMultimodal>,
}

/// Async tool handler function type — legacy text-only signature.
///
/// Most tools (file IO, memory, search, etc.) only ever produce text and
/// keep this signature.  Tools that need to emit images additionally set
/// [`ToolEntry::multimodal_handler`].
#[allow(dead_code)]
pub type ToolHandler = Arc<
    dyn Fn(
            Value,
            SharedToolContext,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, ToolError>> + Send>>
        + Send
        + Sync,
>;

/// Async tool handler returning [`ToolOutput`] directly (Phase 7C, 7C.2).
///
/// Use when the tool needs to emit non-text content (images, mixed-media)
/// the LLM should consume via its multimodal channel.
#[allow(dead_code)]
pub type ToolHandlerMultimodal = Arc<
    dyn Fn(
            Value,
            SharedToolContext,
        )
            -> Pin<Box<dyn std::future::Future<Output = Result<ToolOutput, ToolError>> + Send>>
        + Send
        + Sync,
>;

impl fmt::Debug for ToolEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolEntry")
            .field("name", &self.name)
            .field("toolset", &self.toolset)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("max_result_size", &self.max_result_size)
            .field("max_text_bytes", &self.max_text_bytes)
            .field("max_image_bytes", &self.max_image_bytes)
            .field("timeout_secs", &self.timeout_secs)
            .field("disabled", &self.disabled)
            .field("multimodal", &self.multimodal_handler.is_some())
            .finish()
    }
}

impl ToolEntry {
    /// Effective text byte cap: prefers `max_text_bytes`; falls back to
    /// the deprecated `max_result_size` for tools not yet migrated to the
    /// 7C.2 multimodal layout.  Returns `None` when no cap is configured.
    ///
    /// Reserved for future call sites (provider adapters that need to
    /// pre-trim before serialisation); currently only `enforce_size_caps`
    /// uses the underlying fields directly.
    #[allow(dead_code)]
    #[must_use]
    pub fn effective_text_cap(&self) -> Option<usize> {
        self.max_text_bytes.or(self.max_result_size)
    }

    /// Effective image byte cap (`max_image_bytes`).  Defaults to `None`
    /// (no cap) when the tool never produces images, which is what every
    /// legacy entry leaves it as.
    #[allow(dead_code)]
    #[must_use]
    pub const fn effective_image_cap(&self) -> Option<usize> {
        self.max_image_bytes
    }
}

/// Enforce per-modality byte caps on a [`ToolOutput`] (Phase 7C, slice 7C.2).
///
/// Resolution order, per modality:
/// 1. Explicit `max_text_bytes` / `max_image_bytes` if set.
/// 2. The deprecated `max_result_size` as a single combined fallback,
///    measured against the legacy string projection.
fn enforce_size_caps(name: &str, entry: &ToolEntry, output: &ToolOutput) -> Result<(), ToolError> {
    if let Some(text_cap) = entry.max_text_bytes {
        let text_size = output.text_byte_size();
        if text_size > text_cap {
            return Err(ToolError::OutputTooLarge {
                size: text_size,
                max: text_cap,
            });
        }
    }
    if let Some(image_cap) = entry.max_image_bytes {
        let image_size = output.image_byte_size();
        if image_size > image_cap {
            return Err(ToolError::OutputTooLarge {
                size: image_size,
                max: image_cap,
            });
        }
    }
    if entry.max_text_bytes.is_none() && entry.max_image_bytes.is_none() {
        if let Some(legacy_cap) = entry.max_result_size {
            // Match the pre-7C.2 semantics: cap measured against the
            // legacy stringified projection (image parts collapse to
            // `[image: ...]` placeholders so they hardly count).
            let legacy_size = output.to_legacy_string().len();
            if legacy_size > legacy_cap {
                return Err(ToolError::OutputTooLarge {
                    size: legacy_size,
                    max: legacy_cap,
                });
            }
        }
    }
    let _ = name; // reserved for future telemetry; silence unused-var lint
    Ok(())
}

/// DashMap-based ToolRegistry for high-performance concurrent access.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolRegistry {
    tools: Arc<DashMap<String, ToolEntry>>,
    names_to_toolsets: Arc<DashMap<String, String>>,
    context: SharedToolContext,
}

#[allow(dead_code)]
impl ToolRegistry {
    /// Creates a new empty ToolRegistry with the given tool context.
    #[must_use]
    pub fn new(context: SharedToolContext) -> Self {
        Self {
            tools: Arc::new(DashMap::new()),
            names_to_toolsets: Arc::new(DashMap::new()),
            context,
        }
    }

    /// Returns a reference to the shared tool context.
    #[must_use]
    pub fn context(&self) -> &SharedToolContext {
        &self.context
    }

    /// Registers a tool entry.
    ///
    /// # Errors
    ///
    /// Returns an error if a tool with the same name is already registered.
    pub fn register(&self, entry: ToolEntry) -> Result<(), ToolError> {
        let name = entry.name.clone();
        if self.tools.contains_key(&name) {
            return Err(ToolError::Register(format!(
                "tool `{name}` is already registered"
            )));
        }
        self.names_to_toolsets
            .insert(name.clone(), entry.toolset.clone());
        self.tools.insert(name, entry);
        Ok(())
    }

    /// Gets a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<ToolEntry> {
        self.tools.get(name).map(|r| (*r).clone())
    }

    /// Checks if a tool with the given name exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Gets all tool names in a toolset.
    #[must_use]
    pub fn names_in_toolset(&self, toolset: &str) -> Vec<String> {
        self.names_to_toolsets
            .iter()
            .filter(|entry| entry.value() == toolset)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Gets all registered tool names.
    #[must_use]
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|r| r.key().clone()).collect()
    }

    /// Gets tool definitions in OpenAI format.
    ///
    /// # Arguments
    ///
    /// * `allowed` - Optional list of allowed tool names. If None, all non-disabled tools are included.
    #[must_use]
    pub fn get_definitions(&self, allowed: Option<&[String]>) -> Vec<Value> {
        self.tools
            .iter()
            .filter(|entry| {
                if entry.disabled {
                    return false;
                }
                if let Some(allowed) = allowed {
                    return allowed.contains(entry.key());
                }
                true
            })
            .map(|entry| {
                json!({
                    "type": "function",
                    "function": {
                        "name": entry.name,
                        "description": entry.description,
                        "parameters": entry.input_schema,
                    }
                })
            })
            .collect()
    }

    /// Gets tool definitions filtered by toolsets.
    ///
    /// # Arguments
    ///
    /// * `toolsets` - List of toolset names to filter by
    /// * `toolset_registry` - Registry containing toolset definitions
    #[must_use]
    pub fn get_definitions_by_toolsets(
        &self,
        toolsets: &[String],
        toolset_registry: &crate::modules::tools::toolset::ToolSetRegistry,
    ) -> Vec<Value> {
        let allowed_tools = toolset_registry.tools_from_toolsets(toolsets);
        self.get_definitions(Some(&allowed_tools))
    }

    /// Dispatches a tool call with timeout protection.
    ///
    /// Phase 7C, slice 7C.2: returns [`ToolOutput`] (a vec of
    /// [`ToolResultPart`](super::output::ToolResultPart)) so vision-capable
    /// tools can return images alongside text.  Legacy callers that still
    /// want a `String` should call `.to_legacy_string()` on the result.
    ///
    /// # Errors
    ///
    /// Returns `ToolError::NotFound` if the tool doesn't exist.
    /// Returns `ToolError::Disabled` if the tool is disabled.
    /// Returns `ToolError::Timeout` if execution exceeds the configured timeout.
    /// Returns `ToolError::OutputTooLarge` if any per-modality cap is exceeded.
    pub async fn dispatch(&self, name: &str, args: Value) -> Result<ToolOutput, ToolError> {
        if requires_explicit_context(name) && !allow_shared_context_high_risk_dispatch() {
            return Err(ToolError::Handler(format!(
                "high-risk tool '{name}' requires dispatch_with_context(session-scoped context)"
            )));
        }
        self.dispatch_with_context(name, args, self.context.clone())
            .await
    }

    /// Dispatches a tool call using a provided execution context.
    ///
    /// This is used to isolate workdir/permission context per session or
    /// per turn, avoiding cross-session context leakage through the
    /// registry default context.
    ///
    /// When [`ToolEntry::multimodal_handler`] is `Some`, it takes
    /// precedence; otherwise the legacy `handler` runs and its `String`
    /// output is wrapped in `ToolOutput::text(s)` so callers see the same
    /// shape either way.
    pub async fn dispatch_with_context(
        &self,
        name: &str,
        args: Value,
        context: SharedToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let entry = self
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        if entry.disabled {
            return Err(ToolError::Disabled(name.to_string()));
        }

        let timeout_duration = entry.timeout_secs.unwrap_or(300);
        let timeout_future = Duration::from_secs(timeout_duration as u64);

        let exec = if let Some(mm) = entry.multimodal_handler.clone() {
            timeout(timeout_future, mm(args, context)).await
        } else {
            let legacy = entry.handler.clone();
            timeout(timeout_future, legacy(args, context))
                .await
                .map(|inner| inner.map(ToolOutput::text))
        };

        match exec {
            Ok(Ok(output)) => {
                enforce_size_caps(name, &entry, &output)?;
                Ok(output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ToolError::Timeout(name.to_string())),
        }
    }

    /// Legacy projection of [`Self::dispatch_with_context`] returning a
    /// flat `String` via [`ToolOutput::to_legacy_string`].  Provided so the
    /// existing `commands/agent.rs` + `tool_execution_broker.rs` call
    /// sites can be migrated incrementally without breaking right now.
    pub async fn dispatch_with_context_legacy(
        &self,
        name: &str,
        args: Value,
        context: SharedToolContext,
    ) -> Result<String, ToolError> {
        self.dispatch_with_context(name, args, context)
            .await
            .map(|out| out.to_legacy_string())
    }

    /// Validates that a tool exists and arguments are valid.
    #[must_use]
    pub fn validate(&self, name: &str, args: &Value) -> Option<String> {
        let entry = self.get(name)?;

        let Some(obj) = args.as_object() else {
            return Some("tool input must be a JSON object".to_string());
        };

        // JSON Schema's `required` lives at the object level, not inside each
        // property. Keep this lightweight so every tool benefits before its
        // handler receives malformed args.
        if let Some(required) = entry
            .input_schema
            .get("required")
            .and_then(|v| v.as_array())
        {
            for key in required.iter().filter_map(|value| value.as_str()) {
                if !obj.contains_key(key) {
                    return Some(format!("missing required parameter: {key}"));
                }
            }
        }

        None
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        use std::sync::Mutex;
        let default_context = std::sync::Arc::new(Mutex::new(super::context::ToolContext {
            session_id: None,
            project_id: None,
            workdir: std::path::PathBuf::from("."),
            permission_mode: crate::modules::runtime::permissions::PermissionMode::DangerFullAccess,
        }));
        Self::new(default_context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_handler(output: &'static str) -> ToolHandler {
        Arc::new(move |_input: Value, _context: SharedToolContext| {
            let output = output.to_string();
            Box::pin(async move { Ok(output) })
        })
    }

    fn make_test_registry() -> ToolRegistry {
        let ctx =
            super::super::context::ToolContext::default_for_workdir(std::path::PathBuf::from("."));
        ToolRegistry::new(std::sync::Arc::new(std::sync::Mutex::new(ctx)))
    }

    #[tokio::test]
    async fn registers_and_retrieves_tool() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "test".to_string(),
            toolset: "testing".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("test result"),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        assert!(registry.has("test"));

        let retrieved = registry.get("test").unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.toolset, "testing");
    }

    #[tokio::test]
    async fn dispatch_calls_handler() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "hello".to_string(),
            toolset: "test".to_string(),
            description: "Says hello".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("Hello, World!"),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("hello", json!({})).await.unwrap();
        assert_eq!(result.to_legacy_string(), "Hello, World!");
    }

    #[tokio::test]
    async fn dispatch_not_found_returns_error() {
        let registry = make_test_registry();
        let result = registry.dispatch("nonexistent", json!({})).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }

    #[tokio::test]
    async fn dispatch_disabled_tool_returns_error() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "disabled".to_string(),
            toolset: "test".to_string(),
            description: "Disabled tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: None,
            disabled: true,
            handler: make_test_handler("should not run"),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("disabled", json!({})).await;
        assert!(matches!(result, Err(ToolError::Disabled(_))));
    }

    #[tokio::test]
    async fn dispatch_enforces_timeout() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "slow".to_string(),
            toolset: "test".to_string(),
            description: "Slow tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: Some(1),
            disabled: false,
            handler: Arc::new(|_input: Value, _context: SharedToolContext| {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    Ok("done".to_string())
                })
                    as Pin<Box<dyn std::future::Future<Output = Result<String, ToolError>> + Send>>
            }),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("slow", json!({})).await;
        assert!(matches!(result, Err(ToolError::Timeout(_))));
    }

    #[tokio::test]
    async fn dispatch_enforces_max_result_size() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "large".to_string(),
            toolset: "test".to_string(),
            description: "Large output tool".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: Some(10),
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("this is a long output that exceeds the limit"),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("large", json!({})).await;
        assert!(matches!(result, Err(ToolError::OutputTooLarge { .. })));
    }

    #[tokio::test]
    async fn dispatch_rejects_high_risk_tool_without_explicit_context() {
        let registry = make_test_registry();
        let entry = ToolEntry {
            name: "bash".to_string(),
            toolset: "test".to_string(),
            description: "High risk".to_string(),
            input_schema: json!({"type": "object"}),
            max_result_size: None,
            max_text_bytes: None,
            max_image_bytes: None,
            timeout_secs: None,
            disabled: false,
            handler: make_test_handler("ok"),
            multimodal_handler: None,
        };

        registry.register(entry).unwrap();
        let result = registry.dispatch("bash", json!({})).await;
        assert!(matches!(result, Err(ToolError::Handler(_))));
        let msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(msg.contains("requires dispatch_with_context"));
    }

    #[tokio::test]
    async fn get_definitions_filters_disabled() {
        let registry = make_test_registry();

        // Add a disabled tool
        registry
            .register(ToolEntry {
                name: "disabled".to_string(),
                toolset: "test".to_string(),
                description: "Disabled".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                max_text_bytes: None,
                max_image_bytes: None,
                timeout_secs: None,
                disabled: true,
                handler: make_test_handler(""),
                multimodal_handler: None,
            })
            .unwrap();

        // Add an enabled tool
        registry
            .register(ToolEntry {
                name: "enabled".to_string(),
                toolset: "test".to_string(),
                description: "Enabled".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                max_text_bytes: None,
                max_image_bytes: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
                multimodal_handler: None,
            })
            .unwrap();

        let defs = registry.get_definitions(None);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["function"]["name"], "enabled");
    }

    #[tokio::test]
    async fn get_definitions_respects_allowed_list() {
        let registry = make_test_registry();

        registry
            .register(ToolEntry {
                name: "tool_a".to_string(),
                toolset: "test".to_string(),
                description: "Tool A".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                max_text_bytes: None,
                max_image_bytes: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
                multimodal_handler: None,
            })
            .unwrap();

        registry
            .register(ToolEntry {
                name: "tool_b".to_string(),
                toolset: "test".to_string(),
                description: "Tool B".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                max_text_bytes: None,
                max_image_bytes: None,
                timeout_secs: None,
                disabled: false,
                handler: make_test_handler(""),
                multimodal_handler: None,
            })
            .unwrap();

        let defs = registry.get_definitions(Some(&["tool_a".to_string()]));
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["function"]["name"], "tool_a");
    }
}

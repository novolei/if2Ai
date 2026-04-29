//! Smart Browser runtime orchestration.
//!
//! This is the thin bridge between the stable Smart Browser contract and the
//! existing tool/MCP runtimes. It keeps raw browser-use MCP tools behind the
//! first-party `browser` tool surface.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::modules::runtime::mcp::mcp_tool_name;
use crate::modules::runtime::mcp_stdio::{McpServerManager, McpServerManagerError};

use super::browser_use_mcp::{
    browser_use_arguments_for_command, browser_use_mcp_server_config, browser_use_tool_for_command,
    observation_from_mcp_result, projected_event_from_mcp_result, BROWSER_USE_MCP_SERVER_NAME,
};
use super::contract::{
    SmartBrowserBackend, SmartBrowserCommand, SmartBrowserCommandKind, SmartBrowserEvent,
    SmartBrowserSessionId,
};
use super::local_adapter::browser_tool_action_to_command_kind;
use super::policy::{resolve_backend_label, BackendPolicyError};
use super::session_health::{
    build_recovery_plan, BrowserRecoveryAction, BrowserSessionObservation,
};

static BROWSER_USE_MCP_MANAGER: OnceLock<Arc<tokio::sync::Mutex<McpServerManager>>> =
    OnceLock::new();

/// Output from a browser-use MCP execution.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartBrowserMcpExecution {
    pub event: SmartBrowserEvent,
    pub output: String,
}

/// Runtime failure while routing a Smart Browser command.
#[derive(Debug)]
pub enum SmartBrowserRuntimeError {
    Policy(BackendPolicyError),
    UnsupportedAction(String),
    UnsupportedMcpCommand(SmartBrowserCommandKind),
    MissingMcpArguments(SmartBrowserCommandKind),
    Mcp(McpServerManagerError),
    McpJsonRpc(String),
    McpToolError(String),
}

impl std::fmt::Display for SmartBrowserRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Policy(error) => write!(f, "{error}"),
            Self::UnsupportedAction(action) => {
                write!(f, "unsupported Smart Browser action: {action}")
            }
            Self::UnsupportedMcpCommand(kind) => {
                write!(
                    f,
                    "browser-use MCP does not support command '{}'",
                    kind.as_str()
                )
            }
            Self::MissingMcpArguments(kind) => {
                write!(
                    f,
                    "missing browser-use MCP arguments for '{}'",
                    kind.as_str()
                )
            }
            Self::Mcp(error) => write!(
                f,
                "browser-use MCP backend is unavailable or failed: {error}. \
                 Install browser-use/uvx or switch backend to local_rust_cdp."
            ),
            Self::McpJsonRpc(message) => write!(f, "browser-use MCP JSON-RPC error: {message}"),
            Self::McpToolError(message) => write!(f, "browser-use MCP tool failed: {message}"),
        }
    }
}

impl std::error::Error for SmartBrowserRuntimeError {}

impl From<BackendPolicyError> for SmartBrowserRuntimeError {
    fn from(value: BackendPolicyError) -> Self {
        Self::Policy(value)
    }
}

impl From<McpServerManagerError> for SmartBrowserRuntimeError {
    fn from(value: McpServerManagerError) -> Self {
        Self::Mcp(value)
    }
}

/// Resolve the requested backend from tool args and optional environment.
pub fn resolve_backend_for_args(
    args: &Value,
    env_backend: Option<&str>,
) -> Result<SmartBrowserBackend, SmartBrowserRuntimeError> {
    let requested = args
        .get("backend")
        .and_then(Value::as_str)
        .or(env_backend)
        .filter(|label| !label.trim().is_empty());
    Ok(resolve_backend_label(requested)?)
}

/// Return true when a tool invocation should go through browser-use MCP.
#[must_use]
pub fn should_route_to_browser_use_mcp(backend: SmartBrowserBackend) -> bool {
    backend == SmartBrowserBackend::BrowserUseMcp
}

/// Build the one-server map used by the managed MCP runtime.
#[must_use]
pub fn browser_use_mcp_servers(
    env: BTreeMap<String, String>,
) -> BTreeMap<String, crate::modules::runtime::config::ScopedMcpServerConfig> {
    let (name, config) = browser_use_mcp_server_config(env);
    [(name, config)].into()
}

/// Return the process-wide browser-use MCP manager.
///
/// browser-use owns browser/page state inside the MCP server process, so Smart
/// Browser must reuse one manager instead of spawning a fresh server per action.
#[must_use]
pub fn browser_use_mcp_manager() -> Arc<tokio::sync::Mutex<McpServerManager>> {
    Arc::clone(BROWSER_USE_MCP_MANAGER.get_or_init(|| {
        Arc::new(tokio::sync::Mutex::new(McpServerManager::from_servers(
            &browser_use_mcp_servers(BTreeMap::new()),
        )))
    }))
}

/// Execute a first-party browser action through browser-use MCP.
pub async fn execute_browser_use_mcp_action(
    session_id: &str,
    action: &str,
    args: Value,
) -> Result<SmartBrowserMcpExecution, SmartBrowserRuntimeError> {
    let kind = browser_tool_action_to_command_kind(action)
        .ok_or_else(|| SmartBrowserRuntimeError::UnsupportedAction(action.to_string()))?;
    let raw_tool = browser_use_tool_for_command(kind)
        .ok_or(SmartBrowserRuntimeError::UnsupportedMcpCommand(kind))?;
    let command = SmartBrowserCommand {
        command_id: format!("smart-browser-{}-{kind}", now_ms(), kind = kind.as_str()),
        session_id: SmartBrowserSessionId::new(session_id),
        backend: SmartBrowserBackend::BrowserUseMcp,
        kind,
        input: args,
    };
    let arguments = browser_use_arguments_for_command(&command)
        .ok_or(SmartBrowserRuntimeError::MissingMcpArguments(kind))?;
    let qualified_tool_name = mcp_tool_name(BROWSER_USE_MCP_SERVER_NAME, raw_tool);

    let manager = browser_use_mcp_manager();
    let mut manager = manager.lock().await;
    let response = manager
        .call_tool_discovering(&qualified_tool_name, Some(arguments))
        .await?;
    if let Some(error) = response.error {
        return Err(SmartBrowserRuntimeError::McpJsonRpc(format!(
            "{} ({})",
            error.message, error.code
        )));
    }
    let result = response
        .result
        .ok_or_else(|| SmartBrowserRuntimeError::McpJsonRpc("missing result".to_string()))?;
    let event = projected_event_from_mcp_result(
        format!("smart-browser-event-{}", now_ms()),
        &command,
        &result,
        now_ms(),
    );
    let observation = observation_from_mcp_result(&command, &result);
    let output = observation
        .text_state
        .or_else(|| observation.raw_summary.map(|value| value.to_string()))
        .unwrap_or_else(|| "browser-use MCP completed without textual output".to_string());

    if result.is_error == Some(true) {
        return Err(SmartBrowserRuntimeError::McpToolError(output));
    }

    // WU-006 — when the textual output looks like raw HTML, run it
    // through BR-001's `adaptive_simplify` so the agent sees a
    // bounded, ≤ 35K-char view. Pure-function failure is impossible
    // (`adaptive_simplify` always returns *something*); the
    // env-flag kill-switch + non-HTML inputs flow through untouched.
    let output = simplify_browser_result_text(&output, BROWSER_SIMPLIFY_DEFAULT_TOKENS);

    Ok(SmartBrowserMcpExecution { event, output })
}

// ---------------------------------------------------------------------------
// WU-006 — wire-up helpers (BR-001 simplify + BR-002 coordinate strategy)
// ---------------------------------------------------------------------------

/// Default token budget for `adaptive_simplify` when callers don't
/// pin a value. Conservatively below the 35K char hard limit.
pub const BROWSER_SIMPLIFY_DEFAULT_TOKENS: usize = 8_000;

/// Env var that disables BR-001 HTML simplification (BR-002
/// coordinate-strategy decisions are unaffected).
pub const DISABLE_BROWSER_SIMPLIFY_ENV: &str = "IF2AI_DISABLE_BROWSER_SIMPLIFY";

fn browser_simplify_disabled() -> bool {
    std::env::var(DISABLE_BROWSER_SIMPLIFY_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn looks_like_html(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('<')
        && (trimmed.contains("<html")
            || trimmed.contains("<body")
            || trimmed.contains("<div")
            || trimmed.contains("<main")
            || trimmed.contains("<section")
            || trimmed.contains("<article"))
}

/// Run `raw` through BR-001's `adaptive_simplify` when the input
/// looks like HTML. Pure / sync; failure modes:
///
/// - Kill-switch on → original passed through unchanged.
/// - Input doesn't look like HTML → original passed through unchanged.
/// - `adaptive_simplify` is total over `&str` (never panics, never
///   errors), so the only fall-back path is the two above.
#[must_use]
pub fn simplify_browser_result_text(raw: &str, available_tokens: usize) -> String {
    if browser_simplify_disabled() || !looks_like_html(raw) {
        return raw.to_string();
    }
    let simplified = super::content_simplifier::adaptive_simplify(raw, available_tokens);
    if simplified.html.is_empty() {
        return raw.to_string();
    }
    simplified.html
}

/// WU-006 — coordinate-strategy wrapper used at click dispatch.
/// Pure pass-through to BR-002's `decide_interaction` so the
/// runtime layer doesn't depend on `coordinate_strategy.rs`'s
/// concrete types beyond the public re-export.
#[must_use]
pub fn decide_browser_click_strategy(
    target: &str,
    screenshot: Option<&super::coordinate_strategy::ScreenshotAnalysis>,
    available_selectors: &[String],
) -> super::coordinate_strategy::InteractionDecision {
    super::coordinate_strategy::decide_interaction(target, screenshot, available_selectors)
}

/// DW-005 — env var that disables the BR-002 click-strategy
/// decision (caller falls back to whatever selector path was
/// originally going to run).
pub const DISABLE_BROWSER_STRATEGY_ENV: &str = "IF2AI_DISABLE_BROWSER_STRATEGY";

fn browser_strategy_disabled() -> bool {
    std::env::var(DISABLE_BROWSER_STRATEGY_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// DW-005 — Decide a click strategy AND emit a `BrowserHealth`
/// `click_strategy` envelope so the frontend store sees every
/// dispatch decision. Returns the chosen strategy (or `None`
/// when the kill-switch is set, signalling "use original
/// CssSelector path").
///
/// Failure-isolated: emit failures are swallowed (only `tracing::warn`).
pub fn decide_and_emit_click_strategy(
    target: &str,
    screenshot: Option<&super::coordinate_strategy::ScreenshotAnalysis>,
    available_selectors: &[String],
) -> Option<super::coordinate_strategy::InteractionDecision> {
    if browser_strategy_disabled() {
        return None;
    }
    let decision = decide_browser_click_strategy(target, screenshot, available_selectors);
    let strategy_label = match &decision.strategy {
        super::coordinate_strategy::InteractionStrategy::CoordinateClick { .. } => "coordinate",
        super::coordinate_strategy::InteractionStrategy::LabelReference { .. } => "label",
        super::coordinate_strategy::InteractionStrategy::CssSelector { .. } => "css_selector",
    };
    tracing::debug!(
        strategy = strategy_label,
        target = target,
        fallback_count = decision.fallback_chain.len(),
        "[browser] DW-005 click strategy chosen"
    );
    let payload = serde_json::json!({
        "sessionId": "smart_browser",
        "status": "connected",
        "recoveryPlan": "no_action",
        "strategy": strategy_label,
        "target": target,
    });
    let _ = crate::modules::runtime::evolution_emitter::emit_evolution_event(
        None::<&tauri::AppHandle>,
        crate::modules::runtime::contracts::common::RuntimeEventType::BrowserHealth,
        "click_strategy",
        crate::modules::runtime::contracts::common::CorrelationIds::default(),
        &payload,
        None,
    );
    Some(decision)
}

/// FEAT-SH-003 — Smart Browser monitoring hook. Given an observation
/// of one session's heartbeat, returns the recovery action the
/// daemon should drive next. Pure passthrough to
/// [`build_recovery_plan`] so this module's only responsibility is
/// the *binding* to the runtime — the rules live in
/// `session_health.rs`. Currently exposed for future supervisor
/// wiring; daemon-level integration uses
/// [`super::session_health::BrowserSessionLivenessCheck`] directly.
#[must_use]
pub fn next_browser_recovery_step(obs: &BrowserSessionObservation) -> BrowserRecoveryAction {
    build_recovery_plan(obs).action
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        browser_use_mcp_manager, browser_use_mcp_servers, resolve_backend_for_args,
        should_route_to_browser_use_mcp,
    };
    use crate::modules::smart_browser::contract::SmartBrowserBackend;

    #[test]
    fn selects_backend_from_args_or_env() {
        assert_eq!(
            resolve_backend_for_args(&json!({"backend":"browser_use_mcp"}), None).expect("backend"),
            SmartBrowserBackend::BrowserUseMcp
        );
        assert_eq!(
            resolve_backend_for_args(&json!({}), Some("browser_use_cloud")).expect("env backend"),
            SmartBrowserBackend::BrowserUseCloud
        );
    }

    #[test]
    fn registers_browser_use_mcp_server_without_exposing_raw_agent_tool() {
        let servers = browser_use_mcp_servers(Default::default());
        assert!(servers.contains_key("browser-use"));
        assert!(should_route_to_browser_use_mcp(
            SmartBrowserBackend::BrowserUseMcp
        ));
        assert!(!should_route_to_browser_use_mcp(
            SmartBrowserBackend::BrowserUseCloud
        ));
    }

    #[test]
    fn browser_use_mcp_manager_is_process_wide() {
        let first = browser_use_mcp_manager();
        let second = browser_use_mcp_manager();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }
}

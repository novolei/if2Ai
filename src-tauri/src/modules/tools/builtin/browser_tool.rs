//! Browser tool — AI-driven interactive browser automation.
//!
//! Exposes a single `browser` tool with an `action` discriminant so the LLM
//! issues one coherent tool call per browser operation. The tool delegates
//! to [`BrowserRegistry`] which manages per-session Chromium processes via
//! `chromiumoxide` (Chrome DevTools Protocol).
//!
//! # Actions
//!
//! | Action      | Description                                        |
//! |-------------|----------------------------------------------------|
//! | `start`     | Launch a headless browser for the session          |
//! | `stop`      | Close the browser for the session                  |
//! | `navigate`  | Navigate to a URL and return AXTree snapshot       |
//! | `snapshot`  | Return the current AXTree snapshot                 |
//! | `screenshot`| Return a full-page JPEG (base64)                   |
//! | `click`     | Click element by `data-if2ai-ref` number           |
//! | `type`      | Type text into an element                          |
//! | `scroll`    | Scroll the page up or down                         |
//! | `select`    | Select an option in a `<select>` element           |
//! | `key`       | Press a named key (e.g., "Enter", "Tab")           |
//! | `wait`      | Wait for navigation                                |
//! | `evaluate`  | Execute arbitrary JavaScript                       |

use std::sync::Arc;

use serde_json::{json, Value};

use crate::modules::browser::{BrowserRegistry, ScrollDir};
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

// ── SSRF / URL safety ─────────────────────────────────────────────────────────

/// Return an error string when the URL targets disallowed resources.
///
/// Blocks `file://`, `javascript:`, and RFC-1918 / cloud-metadata addresses
/// to prevent server-side request forgery via the AI's browser tool.
fn check_url_safety(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();

    if lower.starts_with("file://")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
    {
        return Err(format!(
            "URL scheme not allowed for browser navigation: {lower}"
        ));
    }

    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(format!(
            "Only http:// and https:// URLs are allowed; got: {url}"
        ));
    }

    // Block RFC-1918 ranges and cloud metadata endpoints.
    let blocked_hosts = [
        "localhost",
        "127.",
        "0.0.0.0",
        "::1",
        "10.",
        "172.16.",
        "172.17.",
        "172.18.",
        "172.19.",
        "172.20.",
        "172.21.",
        "172.22.",
        "172.23.",
        "172.24.",
        "172.25.",
        "172.26.",
        "172.27.",
        "172.28.",
        "172.29.",
        "172.30.",
        "172.31.",
        "192.168.",
        "169.254.",          // AWS metadata / link-local
        "metadata.google",   // GCP metadata
        "metadata.azure",    // Azure metadata
    ];

    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            let host_lower = host.to_ascii_lowercase();
            for blocked in &blocked_hosts {
                if host_lower.starts_with(blocked) || host_lower == blocked.trim_end_matches('.') {
                    return Err(format!("URL targets a blocked host: {host}"));
                }
            }
        }
    }

    Ok(())
}

// ── Tool entry factory ────────────────────────────────────────────────────────

/// Create the `browser` [`ToolEntry`] with the given [`BrowserRegistry`].
///
/// The registry is captured by the handler closure; its lifetime matches the
/// Tauri application lifetime.
pub fn browser_tool_entry(registry: Arc<BrowserRegistry>) -> ToolEntry {
    let handler: ToolHandler = Arc::new(move |args: Value, ctx| {
        let registry = Arc::clone(&registry);
        Box::pin(async move { execute_browser_action(registry, args, ctx).await })
    });

    ToolEntry {
        name: "browser".to_owned(),
        toolset: "browser".to_owned(),
        description: concat!(
            "Control an interactive headless web browser. ",
            "Use 'start' first, then 'navigate' to load a URL. ",
            "Interact with elements using their [N] ref numbers from 'snapshot'. ",
            "Actions: start | stop | navigate | snapshot | screenshot | ",
            "click | type | scroll | select | key | wait | evaluate"
        )
        .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start","stop","navigate","snapshot","screenshot",
                             "click","type","scroll","select","key","wait","evaluate"],
                    "description": "The browser operation to perform."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate')."
                },
                "ref": {
                    "type": "integer",
                    "description": "data-if2ai-ref number of the target element."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (required for 'type')."
                },
                "press_enter": {
                    "type": "boolean",
                    "description": "Whether to press Enter after typing (default: false)."
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down"],
                    "description": "Scroll direction (default: 'down')."
                },
                "amount": {
                    "type": "integer",
                    "description": "Number of 'pages' to scroll (default: 3)."
                },
                "value": {
                    "type": "string",
                    "description": "Option value to select (required for 'select')."
                },
                "key": {
                    "type": "string",
                    "description": "Key name to press, e.g. 'Enter', 'Tab' (required for 'key')."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum milliseconds to wait (default: 5000, for 'wait')."
                },
                "expression": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (required for 'evaluate')."
                }
            },
            "required": ["action"]
        }),
        max_result_size: Some(64 * 1024), // 64 KB — AXTree + screenshot can be large
        timeout_secs: Some(60),
        disabled: false,
        handler,
    }
}

// ── Action dispatch ───────────────────────────────────────────────────────────

async fn execute_browser_action(
    registry: Arc<BrowserRegistry>,
    args: Value,
    ctx: crate::modules::tools::context::SharedToolContext,
) -> Result<String, ToolError> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::Handler("'action' field is required".into()))?;

    // Extract session_id from the tool context.
    let session_id = ctx
        .lock()
        .map_err(|e| ToolError::Handler(format!("context lock: {e}")))?
        .session_id
        .clone()
        .unwrap_or_else(|| "default".to_owned());

    match action {
        "start" => {
            registry
                .launch(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok("Browser started. Use 'navigate' to load a URL.".to_owned())
        }

        "stop" => {
            registry
                .close(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok("Browser stopped.".to_owned())
        }

        "navigate" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'url' field is required for 'navigate'".into()))?;

            check_url_safety(url)
                .map_err(|e| ToolError::Handler(format!("URL safety check failed: {e}")))?;

            if !registry.is_running(&session_id) {
                registry
                    .launch(&session_id)
                    .await
                    .map_err(|e| ToolError::Handler(e.to_string()))?;
            }

            let result = registry
                .navigate(&session_id, url)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;

            Ok(format!(
                "Navigated to: {}\nTitle: {}\n\n{}",
                result.url, result.title, result.snapshot
            ))
        }

        "snapshot" => {
            ensure_running(&registry, &session_id)?;
            let snapshot = registry
                .snapshot(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "screenshot" => {
            ensure_running(&registry, &session_id)?;
            let b64 = registry
                .screenshot(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(format!("data:image/jpeg;base64,{b64}"))
        }

        "click" => {
            ensure_running(&registry, &session_id)?;
            let ref_num = args
                .get("ref")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .ok_or_else(|| ToolError::Handler("'ref' field (integer) is required for 'click'".into()))?;

            let snapshot = registry
                .click(&session_id, ref_num)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "type" => {
            ensure_running(&registry, &session_id)?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'text' field is required for 'type'".into()))?;
            let ref_num = args
                .get("ref")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let press_enter = args
                .get("press_enter")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let snapshot = registry
                .type_text(&session_id, text, ref_num, press_enter)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "scroll" => {
            ensure_running(&registry, &session_id)?;
            let direction = match args
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("down")
            {
                "up" => ScrollDir::Up,
                _ => ScrollDir::Down,
            };
            let amount = args
                .get("amount")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(3);

            let snapshot = registry
                .scroll(&session_id, direction, amount)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "select" => {
            ensure_running(&registry, &session_id)?;
            let ref_num = args
                .get("ref")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .ok_or_else(|| ToolError::Handler("'ref' field (integer) is required for 'select'".into()))?;
            let value = args
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'value' field is required for 'select'".into()))?;

            let snapshot = registry
                .select_option(&session_id, ref_num, value)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "key" => {
            ensure_running(&registry, &session_id)?;
            let key = args
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'key' field is required for 'key'".into()))?;

            let snapshot = registry
                .press_key(&session_id, key)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "wait" => {
            ensure_running(&registry, &session_id)?;
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(5_000);

            let snapshot = registry
                .wait(&session_id, timeout_ms, "load")
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(snapshot)
        }

        "evaluate" => {
            ensure_running(&registry, &session_id)?;
            let expression = args
                .get("expression")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("'expression' field is required for 'evaluate'".into())
                })?;

            let result = registry
                .evaluate(&session_id, expression)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            Ok(result)
        }

        unknown => Err(ToolError::Handler(format!(
            "Unknown browser action '{unknown}'. Valid actions: \
             start | stop | navigate | snapshot | screenshot | \
             click | type | scroll | select | key | wait | evaluate"
        ))),
    }
}

fn ensure_running(registry: &BrowserRegistry, session_id: &str) -> Result<(), ToolError> {
    if !registry.is_running(session_id) {
        Err(ToolError::Handler(
            "Browser is not running. Use action='start' first.".to_owned(),
        ))
    } else {
        Ok(())
    }
}

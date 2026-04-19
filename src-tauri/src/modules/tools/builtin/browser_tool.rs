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
use tracing::info;

use crate::modules::browser::events::emit_browser_status;
use crate::modules::browser::session::WaitState;
use crate::modules::browser::{BrowserError, BrowserRegistry, ScrollDir};
use crate::modules::tools::output::ToolOutput;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler, ToolHandlerMultimodal};
use crate::modules::viewer_registry::sync_viewer_url;

// ── SSRF / URL safety ─────────────────────────────────────────────────────────

/// Return an error string when the URL targets disallowed resources.
///
/// Blocks `file://`, `javascript:`, and RFC-1918 / cloud-metadata addresses
/// to prevent server-side request forgery via the AI's browser tool.
///
/// Uses `std::net::Ipv6Addr` for IPv6 range checks to avoid prefix-string
/// false-negatives (e.g. `fd12::1` is ULA but does not start with `"fd00:"`).
/// URLs that fail to parse are rejected rather than silently allowed.
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

    // Parse to extract the host; reject unparseable URLs rather than silently
    // allowing them (e.g. Zone-ID URLs like http://[fe80::1%25eth0]/).
    let parsed =
        url::Url::parse(url).map_err(|e| format!("Malformed URL (rejected for safety): {e}"))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host (rejected for safety)".to_owned())?;
    let host_lower = host.to_ascii_lowercase();

    // ── IPv6 address check via structured parsing ──────────────────────────
    // Covers full RFC ranges without relying on prefix-string matching, which
    // misses addresses like fd12::1 (ULA but doesn't start with "fd00:").
    if let Ok(ipv6) = host_lower.parse::<std::net::Ipv6Addr>() {
        let segments = ipv6.segments();
        let first_byte = (segments[0] >> 8) as u8;

        // Loopback ::1
        if ipv6.is_loopback() {
            return Err(format!("URL targets blocked IPv6 loopback: {host}"));
        }
        // Link-local fe80::/10 — first 10 bits are 1111111010
        if (segments[0] & 0xffc0) == 0xfe80 {
            return Err(format!("URL targets blocked IPv6 link-local: {host}"));
        }
        // Unique Local Address fc00::/7 — first byte is 0xfc or 0xfd
        if first_byte == 0xfc || first_byte == 0xfd {
            return Err(format!("URL targets blocked IPv6 ULA address: {host}"));
        }
        // IPv4-mapped ::ffff:0:0/96 — handle via the IPv4 check below
        if let Some(ipv4) = ipv6.to_ipv4() {
            return check_ipv4_safety(ipv4, host);
        }

        // Reject all other private / documentation / unspecified ranges.
        if ipv6.is_unspecified() {
            return Err(format!("URL targets blocked IPv6 unspecified: {host}"));
        }
    }

    // ── IPv4 address check ─────────────────────────────────────────────────
    if let Ok(ipv4) = host_lower.parse::<std::net::Ipv4Addr>() {
        return check_ipv4_safety(ipv4, host);
    }

    // ── Hostname deny-list ─────────────────────────────────────────────────
    let blocked_hostnames = [
        "localhost",
        "metadata.google.internal", // GCP metadata server
        "metadata.azure.internal",  // Azure IMDS
    ];
    for blocked in &blocked_hostnames {
        if host_lower == *blocked {
            return Err(format!("URL targets a blocked hostname: {host}"));
        }
    }

    Ok(())
}

/// Check a parsed IPv4 address against RFC-1918 and other blocked ranges.
fn check_ipv4_safety(ipv4: std::net::Ipv4Addr, host: &str) -> Result<(), String> {
    if ipv4.is_loopback()
        || ipv4.is_private()
        || ipv4.is_link_local()
        || ipv4.is_unspecified()
        || ipv4.is_broadcast()
        || ipv4.is_documentation()
        // Cloud metadata (169.254.169.254 is link-local; already covered above,
        // but also check the full metadata.* prefix just in case).
        || ipv4.octets()[0..2] == [169, 254]
    {
        Err(format!("URL targets a blocked IPv4 address: {host}"))
    } else {
        Ok(())
    }
}

// ── Tool entry factory ────────────────────────────────────────────────────────

/// Create the `browser` [`ToolEntry`] with the given [`BrowserRegistry`].
///
/// The registry is captured by the handler closure; its lifetime matches the
/// Tauri application lifetime.
pub fn browser_tool_entry(registry: Arc<BrowserRegistry>) -> ToolEntry {
    // Phase 7C, slice 7C.2 — when action == "screenshot" we emit a real
    // multimodal `ToolOutput` (Text caption + Image part) so vision-capable
    // models actually see the page.  All other actions are still text-only
    // and flow through the legacy String-returning `handler` for the rest
    // of the agent pipeline (which expects strings today).
    let registry_for_mm = Arc::clone(&registry);
    let multimodal: ToolHandlerMultimodal = Arc::new(move |args: Value, ctx| {
        let registry = Arc::clone(&registry_for_mm);
        Box::pin(async move { execute_browser_action_multimodal(registry, args, ctx).await })
    });

    let handler: ToolHandler = Arc::new(move |args: Value, ctx| {
        let registry = Arc::clone(&registry);
        Box::pin(async move { execute_browser_action(registry, args, ctx).await })
    });

    ToolEntry {
        name: "browser".to_owned(),
        toolset: "browser".to_owned(),
        description: concat!(
            "Control an interactive headless web browser. ",
            "Use 'navigate' to load a URL (auto-starts browser if needed). ",
            "For other actions, start explicitly with action='start' first. ",
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
                "state": {
                    "type": "string",
                    "enum": ["load", "domcontentloaded", "networkidle"],
                    "description": "Lifecycle state to wait for (action='wait'). 'load' = window.onload; 'domcontentloaded' (default) = DOM tree ready; 'networkidle' = no inflight requests for 500ms (best for SPA route changes).",
                    "default": "domcontentloaded"
                },
                "expression": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (required for 'evaluate')."
                }
            },
            "required": ["action"]
        }),
        // Phase 7C, slice 7C.2 — split text vs image budgets so a screenshot
        // (multi-MB base64 JPEG) does not blow the AXTree text budget.
        max_result_size: None,
        max_text_bytes: Some(64 * 1024),        // AXTree snapshots
        max_image_bytes: Some(5 * 1024 * 1024), // up to ~5 MB JPEG/PNG
        timeout_secs: Some(60),
        disabled: false,
        handler,
        multimodal_handler: Some(multimodal),
    }
}

// ── Multimodal dispatch (Phase 7C, slice 7C.2) ───────────────────────────────

/// Multimodal entry point preferred by the registry over the legacy
/// `execute_browser_action`.  Currently only the `screenshot` action emits
/// a non-text part — every other action yields a single `Text` part by
/// delegating to the legacy implementation.  This minimises the diff while
/// giving vision-capable LLMs a real image they can reason about.
async fn execute_browser_action_multimodal(
    registry: Arc<BrowserRegistry>,
    args: Value,
    ctx: crate::modules::tools::context::SharedToolContext,
) -> Result<ToolOutput, ToolError> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::Handler("'action' field is required".into()))?;

    if action == "screenshot" {
        return execute_screenshot_multimodal(registry, ctx).await;
    }

    // Everything else: reuse the legacy text-only path and lift the
    // String into a single `Text` part.
    execute_browser_action(registry, args, ctx)
        .await
        .map(ToolOutput::text)
}

/// Capture a screenshot and return it as a real `Image` part so vision
/// providers (Anthropic / OpenAI vision-capable models) can actually look
/// at the page.  The textual part summarises context (URL) so non-vision
/// fallbacks still get something useful.
async fn execute_screenshot_multimodal(
    registry: Arc<BrowserRegistry>,
    ctx: crate::modules::tools::context::SharedToolContext,
) -> Result<ToolOutput, ToolError> {
    let session_id = ctx
        .lock()
        .map_err(|e| ToolError::Handler(format!("context lock: {e}")))?
        .session_id
        .clone()
        .unwrap_or_else(|| "default".to_owned());

    // Phase 7C, slice 7C.3 — symmetric guard with the legacy text path.
    if registry.is_taken_over(&session_id) {
        return Err(ToolError::Handler(
            "User has taken over the browser; AI tools are paused. \
             Wait for the next user message before retrying."
                .to_owned(),
        ));
    }

    ensure_running_or_restore(&registry, &session_id).await?;

    let b64 = match registry.screenshot(&session_id).await {
        Ok(b64) => b64,
        Err(BrowserError::Cdp(msg)) => {
            return Err(on_cdp_crash(&registry, session_id, &msg).await);
        }
        Err(e) => return Err(ToolError::Handler(e.to_string())),
    };

    let url = registry.current_url(&session_id).unwrap_or_default();
    let caption = if url.is_empty() {
        "Browser screenshot (current viewport).".to_owned()
    } else {
        format!("Browser screenshot at {url}")
    };

    Ok(ToolOutput::text_then_image(
        caption.clone(),
        "image/jpeg",
        b64,
        Some(caption),
    ))
}

// ── Action dispatch ───────────────────────────────────────────────────────────

/// Spawn a fire-and-forget task to emit a `"browser-status"` Tauri event.
///
/// The `AppHandle` is obtained from the registry; if it hasn't been injected
/// yet (e.g. during unit tests) the call is silently a no-op.
fn spawn_emit(registry: Arc<BrowserRegistry>, session_id: String) {
    if let Some(app) = registry.app_handle().cloned() {
        tokio::spawn(async move {
            emit_browser_status(&app, &session_id, &registry).await;
        });
    }
}

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

    // Phase 7C, slice 7C.3 — refuse all actions while the user has taken
    // over the browser.  The LLM is expected to wait for the next user
    // message rather than retry; the routing prompt block (slice 7C.4)
    // explicitly tells it so.
    if registry.is_taken_over(&session_id) {
        return Err(ToolError::Handler(
            "User has taken over the browser; AI tools are paused. \
             Wait for the next user message before retrying."
                .to_owned(),
        ));
    }

    match action {
        "start" => {
            registry
                .launch(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            spawn_emit(Arc::clone(&registry), session_id);
            Ok("Browser started. Use 'navigate' to load a URL.".to_owned())
        }

        "stop" => {
            registry
                .close(&session_id)
                .await
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            spawn_emit(Arc::clone(&registry), session_id);
            Ok("Browser stopped.".to_owned())
        }

        "navigate" => {
            let raw_url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                ToolError::Handler("'url' field is required for 'navigate'".into())
            })?;

            // Normalize scheme-less URLs (e.g. "google.com" → "https://google.com")
            // so the LLM does not need to include the scheme explicitly.
            // Dangerous scheme prefixes are still caught by check_url_safety below.
            let normalized;
            let url: &str = if !raw_url.starts_with("http://")
                && !raw_url.starts_with("https://")
                && !raw_url.starts_with("file://")
                && !raw_url.starts_with("javascript:")
                && !raw_url.starts_with("data:")
            {
                normalized = format!("https://{raw_url}");
                &normalized
            } else {
                raw_url
            };

            check_url_safety(url)
                .map_err(|e| ToolError::Handler(format!("URL safety check failed: {e}")))?;

            if !registry.is_running(&session_id) {
                registry.launch(&session_id).await.map_err(|e| {
                    spawn_emit(Arc::clone(&registry), session_id.clone());
                    ToolError::Handler(e.to_string())
                })?;
            }

            match registry.navigate(&session_id, url).await {
                Ok(result) => {
                    // Mirror the AI's navigation into the live BrowserViewer window
                    // (no-op when the viewer isn't open).
                    sync_viewer_url(&session_id, &result.url);

                    // Wait briefly for the page to render before emitting thumbnail.
                    // Without this delay the screenshot may capture a blank page.
                    let reg_clone = Arc::clone(&registry);
                    let sid_clone = session_id.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                        // Registry is constructed with `set_app_handle` during
                        // Tauri `setup()` (see `main.rs`), so this branch is
                        // never reached without an installed handle.
                        #[allow(clippy::expect_used)]
                        let handle = reg_clone.app_handle().cloned().expect("app handle");
                        emit_browser_status(&handle, &sid_clone, &reg_clone).await;
                    });

                    let captcha_hint =
                        detect_verification_hint(&result.url, &result.title, &result.snapshot);
                    let mut output = format!(
                        "Navigated to: {}\nTitle: {}\n\n{}",
                        result.url, result.title, result.snapshot
                    );
                    if let Some(hint) = captcha_hint {
                        output.push_str("\n\n");
                        output.push_str(&hint);
                    }
                    Ok(output)
                }
                Err(BrowserError::Cdp(msg)) | Err(BrowserError::Snapshot(msg)) => {
                    // The browser process may have exited mid-operation.
                    // Remove the stale session entry so the next navigate call
                    // can auto-launch a fresh browser instead of failing again.
                    let _ = registry.close(&session_id).await;
                    spawn_emit(Arc::clone(&registry), session_id);
                    Err(ToolError::Handler(format!(
                        "Browser process exited unexpectedly ({msg}). \
                         The session has been reset — call 'navigate' again \
                         and a fresh browser will start automatically."
                    )))
                }
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "snapshot" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            match registry.snapshot(&session_id).await {
                Ok(snapshot) => Ok(snapshot),
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "screenshot" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            match registry.screenshot(&session_id).await {
                Ok(b64) => Ok(format!("data:image/jpeg;base64,{b64}")),
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "click" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let ref_num = args
                .get("ref")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .ok_or_else(|| {
                    ToolError::Handler("'ref' field (integer) is required for 'click'".into())
                })?;

            match registry.click(&session_id, ref_num).await {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "type" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'text' field is required for 'type'".into()))?;
            let ref_num = args.get("ref").and_then(|v| v.as_u64()).map(|v| v as u32);
            let press_enter = args
                .get("press_enter")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            match registry
                .type_text(&session_id, text, ref_num, press_enter)
                .await
            {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "scroll" => {
            ensure_running_or_restore(&registry, &session_id).await?;
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

            match registry.scroll(&session_id, direction, amount).await {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "select" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let ref_num = args
                .get("ref")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .ok_or_else(|| {
                    ToolError::Handler("'ref' field (integer) is required for 'select'".into())
                })?;
            let value = args.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                ToolError::Handler("'value' field is required for 'select'".into())
            })?;

            match registry.select_option(&session_id, ref_num, value).await {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "key" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let key = args
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("'key' field is required for 'key'".into()))?;

            match registry.press_key(&session_id, key).await {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "wait" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(5_000);
            // Phase 7C, slice 7C.5 — honour the LLM's "state" choice.
            // Defaults to `domcontentloaded` (was hard-coded "load" before).
            let state = WaitState::from_str(
                args.get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("domcontentloaded"),
            );

            match registry.wait(&session_id, timeout_ms, state).await {
                Ok(snapshot) => {
                    spawn_emit(Arc::clone(&registry), session_id);
                    Ok(snapshot)
                }
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        "evaluate" => {
            ensure_running_or_restore(&registry, &session_id).await?;
            let expression = args
                .get("expression")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("'expression' field is required for 'evaluate'".into())
                })?;

            match registry.evaluate(&session_id, expression).await {
                Ok(result) => Ok(result),
                Err(BrowserError::Cdp(msg)) => Err(on_cdp_crash(&registry, session_id, &msg).await),
                Err(e) => Err(ToolError::Handler(e.to_string())),
            }
        }

        unknown => Err(ToolError::Handler(format!(
            "Unknown browser action '{unknown}'. Valid actions: \
             start | stop | navigate | snapshot | screenshot | \
             click | type | scroll | select | key | wait | evaluate"
        ))),
    }
}

// ── Session health helpers ────────────────────────────────────────────────────

/// Check whether the session is running, and if not, attempt to restore it
/// from cold state (previously visited URL).  Returns an error only when the
/// browser is absent AND cannot be auto-restored.
async fn ensure_running_or_restore(
    registry: &Arc<BrowserRegistry>,
    session_id: &str,
) -> Result<(), ToolError> {
    if registry.is_running(session_id) {
        return Ok(());
    }

    match registry.restore_cold_state(session_id).await {
        Ok(true) => {
            info!(session_id, "browser auto-restored from cold state");
            Ok(())
        }
        Ok(false) => Err(ToolError::Handler(
            "Browser is not running. Use 'navigate' with a URL to start browsing.".into(),
        )),
        Err(e) => Err(ToolError::Handler(format!(
            "Browser is not running and auto-restore failed ({e}). \
             Use 'navigate' to restart."
        ))),
    }
}

/// Handle a CDP crash during a non-navigate action: clean up the stale session
/// and emit a stopped status event so the BrowserCard UI clears itself.
async fn on_cdp_crash(
    registry: &Arc<BrowserRegistry>,
    session_id: String,
    cdp_msg: &str,
) -> ToolError {
    let _ = registry.close(&session_id).await;
    spawn_emit(Arc::clone(registry), session_id);
    ToolError::Handler(format!(
        "Browser process crashed during this action ({cdp_msg}). \
         The session has been reset — use 'navigate' to restart."
    ))
}

// ── CAPTCHA / bot-protection detection ───────────────────────────────────────

/// Detect common human-verification page patterns and return an actionable hint
/// for the LLM so it can switch strategies instead of blindly trying click/type.
fn detect_verification_hint(url: &str, title: &str, snapshot: &str) -> Option<String> {
    let url_lower = url.to_ascii_lowercase();
    let title_lower = title.to_ascii_lowercase();
    let snap_lower = snapshot.to_ascii_lowercase();

    // ── Cloudflare Turnstile (enterprise-grade, cannot be bypassed by headless) ──
    // Affects: claude.ai, anthropic.com, and many other Cloudflare-protected sites.
    // Even with stealth mode, Cloudflare's server-side checks (TLS fingerprinting,
    // behavioural scoring) will still block headless Chrome.  The ONLY reliable
    // option for these sites is web_fetch or web_search.
    let is_cloudflare_protected = url_lower.contains("claude.ai")
        || url_lower.contains("anthropic.com")
        || title_lower.contains("just a moment")
        || title_lower.contains("attention required")
        || snap_lower.contains("checking if the site connection is secure")
        || snap_lower.contains("enable javascript and cookies to continue")
        || snap_lower.contains("cf-browser-verification")
        || snap_lower.contains("ray id");

    if is_cloudflare_protected {
        return Some(
            "⚠️  Cloudflare bot-protection detected (or site is claude.ai/anthropic.com). \
             Headless Chrome CANNOT bypass Cloudflare Turnstile regardless of stealth settings — \
             this is a fundamental limitation. \
             You MUST switch strategy: use web_search or web_fetch to get content \
             from this site instead of continuing with browser actions."
                .into(),
        );
    }

    // ── Google "unusual traffic" / reCAPTCHA ──────────────────────────────────
    if url_lower.contains("/sorry/") || url_lower.contains("google.com/sorry") {
        return Some(
            "⚠️  Google bot-protection detected. \
             The page is requiring human verification. \
             Switch to web_search to search Google programmatically instead."
                .into(),
        );
    }

    // ── Generic CAPTCHA patterns ───────────────────────────────────────────────
    if title_lower.contains("captcha")
        || snap_lower.contains("i am not a robot")
        || snap_lower.contains("prove you are human")
        || snap_lower.contains("verify you are human")
        || snap_lower.contains("human verification")
    {
        return Some(
            "⚠️  Human verification page detected. \
             Headless browsers cannot reliably solve CAPTCHAs. \
             Switch to web_fetch or web_search for this content instead."
                .into(),
        );
    }

    None
}

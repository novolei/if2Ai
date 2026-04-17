//! Browser session — wraps a single chromiumoxide Browser + Page pair.
//!
//! Each chat session owns one [`BrowserSession`] which manages:
//!
//! - The headless Chromium process (via `chromiumoxide::Browser`)
//! - A single active page
//! - An operation log for AI error recovery
//! - The current URL cache

use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, InsertTextParams,
};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::{Page, ScreenshotParams};
use chrono::Utc;
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::modules::browser::chrome_finder::find_chrome_binary;
use crate::modules::browser::errors::BrowserError;
use crate::modules::browser::snapshot::SNAPSHOT_SCRIPT;

/// Direction for scroll operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDir {
    Up,
    Down,
}

/// Result of a navigate operation.
#[derive(Debug, Clone, Serialize)]
pub struct NavigateResult {
    /// Final URL after navigation (may differ from input due to redirects).
    pub url: String,
    /// Page title.
    pub title: String,
    /// DOM AXTree snapshot text with data-if2ai-ref annotations.
    pub snapshot: String,
}

/// One log entry per browser action, accumulated during a session.
#[derive(Debug, Clone, Serialize)]
pub struct ActionLogEntry {
    /// ISO-8601 timestamp.
    pub ts: String,
    /// Action name (e.g., "navigate", "click").
    pub action: String,
    /// Key parameters (url, ref, text preview, etc.).
    pub params: serde_json::Value,
    /// Short outcome: `"ok: <summary>"` or `"error: <msg>"`.
    pub result: String,
    /// URL at the time of the action.
    pub url: Option<String>,
}

/// A single browser session bound to one chat session identifier.
pub struct BrowserSession {
    /// Identifier matching the parent chat session.
    pub session_id: String,
    /// The chromiumoxide Browser handle (owns the Chromium process).
    browser: Browser,
    /// Background task that polls the CDP handler — must not be dropped.
    _handler: JoinHandle<()>,
    /// Active page.
    page: Page,
    /// Cached current URL; updated after every navigation/interaction.
    pub current_url: Option<String>,
    /// Full log of every action taken in this session.
    pub action_log: Vec<ActionLogEntry>,
}

impl BrowserSession {
    /// Launch a new headless Chromium process and open an initial blank page.
    ///
    /// Searches for an installed Chrome/Chromium binary via
    /// [`find_chrome_binary`]. Returns [`BrowserError::ChromeNotFound`] when
    /// no browser is available.
    pub async fn new(session_id: String) -> Result<Self, BrowserError> {
        let chrome_status = find_chrome_binary();
        if !chrome_status.found {
            return Err(BrowserError::ChromeNotFound);
        }
        let chrome_path = chrome_status.path.ok_or(BrowserError::ChromeNotFound)?;

        debug!(
            session_id = %session_id,
            chrome = %chrome_path.display(),
            "launching headless browser"
        );

        let config = BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .build()
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Handler implements Stream; it must be polled so CDP messages flow.
        let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        Ok(Self {
            session_id,
            browser,
            _handler: handler_task,
            page,
            current_url: None,
            action_log: Vec::new(),
        })
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn log(&mut self, action: &str, params: serde_json::Value, result: &str) {
        self.action_log.push(ActionLogEntry {
            ts: Utc::now().to_rfc3339(),
            action: action.to_owned(),
            params,
            result: result.to_owned(),
            url: self.current_url.clone(),
        });
    }

    async fn sync_url(&mut self) {
        if let Ok(Some(url)) = self.page.url().await {
            self.current_url = Some(url);
        }
    }

    /// Run the AXTree snapshot script and return the text representation.
    async fn run_snapshot(&self) -> Result<String, BrowserError> {
        let result = self
            .page
            .evaluate(SNAPSHOT_SCRIPT)
            .await
            .map_err(|e| BrowserError::Snapshot(e.to_string()))?;

        let value = result
            .value()
            .ok_or_else(|| BrowserError::Snapshot("no return value".into()))?;

        let text = value
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrowserError::Snapshot("missing 'text' field".into()))?;

        Ok(text.to_owned())
    }

    /// Capture a JPEG screenshot and return it as base64.
    async fn capture_jpeg(&self, quality: i64) -> Result<String, BrowserError> {
        let params = ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Jpeg)
            .quality(quality)
            .build();
        let bytes = self
            .page
            .screenshot(params)
            .await
            .map_err(|e| BrowserError::Screenshot(e.to_string()))?;
        Ok(B64.encode(bytes))
    }

    /// Send `text` as InsertText CDP event (bulk, no element focus needed).
    async fn insert_text(&self, text: &str) -> Result<(), BrowserError> {
        let params = InsertTextParams::builder()
            .text(text)
            .build()
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        self.page
            .execute(params)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        Ok(())
    }

    /// Dispatch a named key (e.g., `"Enter"`, `"Tab"`) via CDP KeyDown/KeyUp.
    async fn dispatch_key(&self, key: &str) -> Result<(), BrowserError> {
        let down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key(key)
            .build()
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        self.page
            .execute(down)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        let up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key(key)
            .build()
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        self.page
            .execute(up)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        Ok(())
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Navigate to `url`, wait for `DOMContentLoaded`, and return the result
    /// including an AXTree snapshot of the loaded page.
    pub async fn navigate(&mut self, url: &str) -> Result<NavigateResult, BrowserError> {
        self.page
            .goto(url)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Wait up to 10 s for initial page load.
        let _ =
            tokio::time::timeout(Duration::from_secs(10), self.page.wait_for_navigation()).await;

        self.sync_url().await;
        let current_url = self.current_url.clone().unwrap_or_else(|| url.to_owned());

        let title = self
            .page
            .get_title()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?
            .unwrap_or_default();

        let snapshot = self.run_snapshot().await.unwrap_or_else(|e| {
            warn!("snapshot after navigate failed: {e}");
            String::new()
        });

        self.log(
            "navigate",
            serde_json::json!({ "url": url }),
            &format!("ok: {title}"),
        );

        Ok(NavigateResult {
            url: current_url,
            title,
            snapshot,
        })
    }

    /// Return the AXTree snapshot of the current page.
    pub async fn snapshot(&self) -> Result<String, BrowserError> {
        self.run_snapshot().await
    }

    /// Capture a full-page JPEG screenshot (quality 85) and return base64.
    pub async fn screenshot(&self) -> Result<String, BrowserError> {
        self.capture_jpeg(85).await
    }

    /// Capture a small thumbnail (quality 60) suitable for the BrowserCard.
    ///
    /// Uses a lower quality to keep the Tauri event payload small (~8-15 KB).
    pub async fn thumbnail(&self) -> Result<Option<String>, BrowserError> {
        match self.capture_jpeg(60).await {
            Ok(b64) => Ok(Some(b64)),
            Err(e) => {
                warn!("thumbnail capture failed: {e}");
                Ok(None)
            }
        }
    }

    /// Click the element annotated with `data-if2ai-ref="{ref_num}"` and
    /// return a fresh AXTree snapshot.
    pub async fn click(&mut self, ref_num: u32) -> Result<String, BrowserError> {
        let js = format!(
            r#"(function(){{
                var el = document.querySelector('[data-if2ai-ref="{ref_num}"]');
                if (!el) return {{ ok: false }};
                el.click();
                return {{ ok: true }};
            }})()"#
        );
        let result = self
            .page
            .evaluate(js)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        let ok = result
            .value()
            .and_then(|v| v.get("ok"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !ok {
            return Err(BrowserError::RefNotFound(ref_num));
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        self.sync_url().await;

        let snapshot = self.run_snapshot().await?;
        self.log(
            "click",
            serde_json::json!({ "ref": ref_num }),
            &format!("ok: clicked [{ref_num}]"),
        );
        Ok(snapshot)
    }

    /// Focus `ref_num` (if given), type `text`, and optionally press Enter.
    /// Returns a fresh AXTree snapshot.
    pub async fn type_text(
        &mut self,
        text: &str,
        ref_num: Option<u32>,
        press_enter: bool,
    ) -> Result<String, BrowserError> {
        if let Some(r) = ref_num {
            let js = format!(
                r#"(function(){{
                    var el = document.querySelector('[data-if2ai-ref="{r}"]');
                    if (!el) return {{ ok: false }};
                    el.focus();
                    return {{ ok: true }};
                }})()"#
            );
            let result = self
                .page
                .evaluate(js)
                .await
                .map_err(|e| BrowserError::Cdp(e.to_string()))?;
            let ok = result
                .value()
                .and_then(|v| v.get("ok"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !ok {
                return Err(BrowserError::RefNotFound(r));
            }
        }

        // Use CDP InsertText for efficient bulk input.
        self.insert_text(text).await?;

        if press_enter {
            self.dispatch_key("Enter").await?;
        }

        tokio::time::sleep(Duration::from_millis(150)).await;
        self.sync_url().await;

        let snapshot = self.run_snapshot().await?;
        let preview: String = text.chars().take(60).collect();
        self.log(
            "type",
            serde_json::json!({ "ref": ref_num, "text": preview }),
            "ok: typed",
        );
        Ok(snapshot)
    }

    /// Scroll the page in the given direction by `amount` "pages".
    /// Returns a fresh AXTree snapshot.
    pub async fn scroll(
        &mut self,
        direction: ScrollDir,
        amount: u32,
    ) -> Result<String, BrowserError> {
        let delta = match direction {
            ScrollDir::Down => amount as i64 * 600,
            ScrollDir::Up => -(amount as i64 * 600),
        };
        let js = format!("window.scrollBy(0, {delta})");
        self.page
            .evaluate(js)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = self.run_snapshot().await?;
        self.log(
            "scroll",
            serde_json::json!({ "direction": direction, "amount": amount }),
            "ok: scrolled",
        );
        Ok(snapshot)
    }

    /// Select the `<option>` with the given `value` inside the `<select>`
    /// identified by `ref_num`. Returns a fresh AXTree snapshot.
    pub async fn select_option(
        &mut self,
        ref_num: u32,
        value: &str,
    ) -> Result<String, BrowserError> {
        let escaped = value.replace('"', "\\\"");
        let js = format!(
            r#"(function(){{
                var el = document.querySelector('[data-if2ai-ref="{ref_num}"]');
                if (!el) return {{ ok: false }};
                el.value = "{escaped}";
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return {{ ok: true }};
            }})()"#
        );
        let result = self
            .page
            .evaluate(js)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        let ok = result
            .value()
            .and_then(|v| v.get("ok"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ok {
            return Err(BrowserError::RefNotFound(ref_num));
        }

        let snapshot = self.run_snapshot().await?;
        self.log(
            "select",
            serde_json::json!({ "ref": ref_num, "value": value }),
            "ok: selected",
        );
        Ok(snapshot)
    }

    /// Dispatch a named key event to the page. Returns a fresh AXTree snapshot.
    pub async fn press_key(&mut self, key: &str) -> Result<String, BrowserError> {
        self.dispatch_key(key).await?;
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.sync_url().await;

        let snapshot = self.run_snapshot().await?;
        self.log("key", serde_json::json!({ "key": key }), "ok: key pressed");
        Ok(snapshot)
    }

    /// Wait up to `timeout_ms` for page navigation. Returns a fresh AXTree snapshot.
    pub async fn wait(&mut self, timeout_ms: u64, _state: &str) -> Result<String, BrowserError> {
        let _ = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.page.wait_for_navigation(),
        )
        .await;

        self.sync_url().await;
        self.run_snapshot().await
    }

    /// Evaluate arbitrary JavaScript in the page and return the serialised
    /// result. Output is capped at 30 000 characters.
    pub async fn evaluate(&self, expression: &str) -> Result<String, BrowserError> {
        let result = self
            .page
            .evaluate(expression)
            .await
            .map_err(|e| BrowserError::Evaluate(e.to_string()))?;

        let value = result.value().cloned().unwrap_or(serde_json::Value::Null);
        let serialised = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_owned());

        if serialised.len() > 30_000 {
            Ok(format!("{}\n[output truncated]", &serialised[..30_000]))
        } else {
            Ok(serialised)
        }
    }

    /// Close the browser and release all resources.
    pub async fn close(mut self) -> Result<(), BrowserError> {
        self.browser
            .close()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        Ok(())
    }
}

//! Browser session — wraps a single chromiumoxide Browser + Page pair.
//!
//! Each chat session owns one [`BrowserSession`] which manages:
//!
//! - The headless Chromium process (via `chromiumoxide::Browser`)
//! - A single active page
//! - An operation log for AI error recovery
//! - The current URL cache

use std::path::Path;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, InsertTextParams,
};
use chromiumoxide::cdp::browser_protocol::network::{
    EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent,
};
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, EventDomContentEventFired, EventLoadEventFired,
};
use chromiumoxide::page::{Page, ScreenshotParams};
use chrono::Utc;
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::modules::browser::chrome_finder::find_chrome_binary;
use crate::modules::browser::errors::BrowserError;
use crate::modules::browser::profile::{BrowserProfileMode, ProfileHandle};
use crate::modules::browser::snapshot::SNAPSHOT_SCRIPT;

/// Direction for scroll operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDir {
    Up,
    Down,
}

/// Phase 7C, slice 7C.6 — short metadata for one open browser tab,
/// returned by [`BrowserSession::list_tabs`] for the LLM and the
/// frontend BrowserCard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    /// Position in the tab list at the time of the call.  Pass this
    /// back as `tab_index` to `switch_tab` / `close_tab`.
    pub idx: usize,
    /// Current URL of the page (or empty when the tab is still
    /// loading `about:blank`).
    pub url: String,
    /// Page title (best effort; empty for protected / loading pages).
    pub title: String,
    /// `true` for the page that subsequent click / type / snapshot
    /// actions will operate on.
    pub active: bool,
    /// Opaque CDP target id, kept stable across `refresh_pages` calls.
    pub target_id: String,
}

/// Hard cap so a misbehaving site (`for(;;) window.open()`) can't
/// drag us into OOM.  The oldest *non-active* tab is closed first
/// when `refresh_pages` discovers we're over the limit.
const MAX_TRACKED_TABS: usize = 10;

/// Phase 7C, slice 7C.5 — what `wait()` should wait for.  Maps directly
/// to the three Playwright lifecycle states the LLM already knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitState {
    /// `Page.domContentEventFired` — DOM tree built (initial markup
    /// parsed, but images / fonts may still be loading).
    DomContentLoaded,
    /// `Page.loadEventFired` — `window.onload` fired (all sub-resources
    /// loaded).
    Load,
    /// Network has been quiet for `NETWORK_IDLE_QUIET_WINDOW_MS` —
    /// inflight request count returned to 0 and stayed there.  Useful
    /// for SPA route transitions whose JS finishes after `load`.
    NetworkIdle,
}

impl WaitState {
    /// Parse the string the LLM sends through the tool schema.  Unknown
    /// values default to `DomContentLoaded` so a typo never causes a
    /// hard error — matches Playwright's leniency.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "load" => Self::Load,
            "networkidle" | "network_idle" | "network-idle" => Self::NetworkIdle,
            _ => Self::DomContentLoaded,
        }
    }
}

// ── Phase 7C, slice 7C.5: post-action settle delays ───────────────────────────
//
// Pre-7C.5 values were 200/150/150/150ms — too short for SPA route
// transitions, leading to AI getting the *previous* snapshot and
// double-clicking.  openhanako uses 500-800ms; we picked slightly more
// conservative defaults so quick pages still feel snappy.

const DELAY_AFTER_CLICK_MS: u64 = 600;
const DELAY_AFTER_TYPE_MS: u64 = 300;
const DELAY_AFTER_TYPE_ENTER_MS: u64 = 800;
const DELAY_AFTER_SCROLL_MS: u64 = 400;
const DELAY_AFTER_KEY_MS: u64 = 300;

/// Soft post-click navigation timeout — if a click triggers navigation
/// we wait up to this long for it to settle, otherwise we just snapshot.
const POST_ACTION_NAV_TIMEOUT_MS: u64 = 1500;

/// Quiet window required for `NetworkIdle` to fire (inflight count
/// must remain 0 for this duration before we return).
const NETWORK_IDLE_QUIET_WINDOW_MS: u64 = 500;
/// Polling cadence inside the network-idle loop.  Bounded so a stalled
/// `EventStream` never starves the await.
const NETWORK_IDLE_TICK_MS: u64 = 50;

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
    /// Strongly-typed UTC timestamp; serde serialises to ISO-8601 string.
    pub ts: chrono::DateTime<Utc>,
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
    /// Access via [`BrowserRegistry::take_action_log`] rather than directly.
    pub(crate) action_log: Vec<ActionLogEntry>,
    /// User-data-dir backing this Chromium process.
    ///
    /// Phase 7C, slice 7C.1 replaced the legacy unique-`tempdir` approach
    /// (which wiped cookies on every restart) with a [`ProfileHandle`] that
    /// honours [`BrowserProfileMode`].  `Drop` only deletes the directory
    /// when the mode is `Ephemeral`; persistent profiles survive between
    /// restarts so the AI keeps the user's login state.
    _profile: ProfileHandle,
    /// Phase 7C, slice 7C.3 — `true` when this session was launched with
    /// `--headed` and a visible Chromium window.  The user takes over the
    /// browser by relaunching the session in this mode (see
    /// [`crate::modules::browser::registry::BrowserRegistry::relaunch_with_mode`]).
    pub headed: bool,
}

impl BrowserSession {
    /// Launch a new Chromium process bound to a [`ProfileHandle`] derived
    /// from `profile_mode` and open an initial blank page.
    ///
    /// `if2ai_home` is the user-data root (typically `~/.if2ai/`); it
    /// determines where the persistent profile directory lives.  `headed`
    /// (Phase 7C, slice 7C.3) controls whether Chromium runs visible to
    /// the user — `false` is the production default; `true` is set by the
    /// "request takeover" path so the user can interact with the page.
    ///
    /// Searches for an installed Chrome/Chromium binary via
    /// [`find_chrome_binary`]. Returns [`BrowserError::ChromeNotFound`] when
    /// no browser is available, or [`BrowserError::ProfileError`] when the
    /// profile directory cannot be created.
    pub async fn new(
        session_id: String,
        profile_mode: BrowserProfileMode,
        if2ai_home: &Path,
        headed: bool,
    ) -> Result<Self, BrowserError> {
        let chrome_status = find_chrome_binary();
        if !chrome_status.found {
            return Err(BrowserError::ChromeNotFound);
        }
        let chrome_path = chrome_status.path.ok_or(BrowserError::ChromeNotFound)?;

        debug!(
            session_id = %session_id,
            chrome = %chrome_path.display(),
            mode = ?profile_mode,
            headed = headed,
            "launching browser"
        );

        // Resolve the profile location.  PerSessionPersistent / Shared keep
        // cookies + localStorage on disk so login state survives restarts;
        // Ephemeral hands back a `TempDir` that wipes itself on Drop.
        let profile = profile_mode.resolve(if2ai_home, &session_id)?;

        debug!(
            session_id = %session_id,
            data_dir = %profile.path.display(),
            mode = ?profile.mode,
            "resolved browser user-data-dir"
        );

        // no_sandbox() passes --no-sandbox and --disable-setuid-sandbox to Chrome.
        // This is REQUIRED when Chrome is launched as a child process of another
        // application (e.g. a Tauri app) on macOS and Linux: without it, Chrome's
        // internal sandbox layer conflicts with the host OS process-level security
        // policy and the process exits before writing its DevTools WebSocket URL,
        // which chromiumoxide surfaces as "CDP error: Browser process exit".
        let mut builder = BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .user_data_dir(&profile.path)
            .no_sandbox()
            // Skip first-run wizard and default-browser check for faster startup.
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            // Prevent /dev/shm exhaustion on Linux (harmless on macOS).
            .arg("--disable-dev-shm-usage")
            // Remove the `navigator.webdriver = true` signal injected by the
            // Chrome DevTools automation layer. Without this, any site that
            // calls `navigator.webdriver` will immediately know it's being
            // automated (basic bot detection).
            .arg("--disable-blink-features=AutomationControlled")
            // Remove the `--enable-automation` Chrome feature flag that is
            // added automatically by chromiumoxide. It shows an info-bar in
            // headed mode and exposes an automation flag in JS.
            .arg("--exclude-switches=enable-automation");

        if headed {
            // Phase 7C, slice 7C.3 — visible Chrome window so the user can
            // take over (log in, solve CAPTCHA, fill multi-step forms).
            // Position the window away from the If2Ai main window so both
            // remain visible side-by-side.
            builder = builder
                .with_head()
                .arg("--window-position=900,80")
                .arg("--window-size=1024,768");
        } else {
            // Headless: keep the GPU disabled to avoid initialisation crashes
            // on headless macOS / Linux runners.  In headed mode we leave
            // Chrome's default GPU on so pages render normally.
            builder = builder.arg("--disable-gpu");
        }

        let config = builder
            .build()
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Handler implements Stream; it must be polled so CDP messages flow.
        let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

        // Phase 7C, slice 7C.1: opening a normal page on the default
        // browser context (no `start_incognito_context`).  Cookie /
        // localStorage isolation is now provided by the per-session profile
        // directory itself, not by an extra in-memory incognito layer.
        // This is what lets `PerSessionPersistent` actually persist anything.
        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Inject stealth-mode JS patches on every new document (runs before page JS):
        // - Removes navigator.webdriver = true (most common bot detection signal)
        // - Fakes window.chrome, navigator.plugins, WebGL vendor, permissions
        // - Sets a realistic macOS Chrome user-agent (removes "HeadlessChrome" string)
        //
        // NOTE: This reduces detection by basic-to-medium bot protection systems.
        // Cloudflare Turnstile (used by claude.ai, some Anthropic sites) and other
        // enterprise-grade solutions use additional signals (TLS fingerprinting,
        // behavioural analysis, Canvas fingerprinting) that cannot be bypassed by
        // headless Chrome regardless of these patches. For such sites the correct
        // approach is to use `web_fetch` or `web_search` instead.
        page.enable_stealth_mode_with_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/131.0.0.0 Safari/537.36",
        )
        .await
        .map_err(|e| BrowserError::Cdp(format!("stealth mode init failed: {e}")))?;

        debug!(session_id = %session_id, "stealth mode enabled");

        Ok(Self {
            session_id,
            browser,
            _handler: handler_task,
            page,
            current_url: None,
            action_log: Vec::new(),
            _profile: profile,
            headed,
        })
    }

    /// Test-only convenience constructor that uses
    /// [`BrowserProfileMode::Ephemeral`] and a throw-away `if2ai_home`
    /// (`tempfile::TempDir`).  Used by every existing unit/integration test
    /// that previously relied on the legacy 1-argument `new(session_id)`.
    ///
    /// Production code MUST go through [`BrowserSession::new`] with the real
    /// configured `profile_mode` and `if2ai_home` so cookies persist.
    #[cfg(test)]
    pub async fn new_for_test(session_id: String) -> Result<Self, BrowserError> {
        // The TempDir lives long enough for `Ephemeral` to materialise
        // its own inner tempdir before we go out of scope.  We do not
        // need to keep it alive past `new()` because Ephemeral copies
        // a self-owned `TempDir` into the `ProfileHandle`.
        let if2ai_home = tempfile::Builder::new()
            .prefix("if2ai-test-home-")
            .tempdir()
            .map_err(|e| BrowserError::ProfileError(e.to_string()))?;
        Self::new(
            session_id,
            BrowserProfileMode::Ephemeral,
            if2ai_home.path(),
            false,
        )
        .await
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn log(&mut self, action: &str, params: serde_json::Value, result: &str) {
        self.action_log.push(ActionLogEntry {
            ts: Utc::now(),
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

        // get_title() can fail on protected/CAPTCHA pages (e.g. Cloudflare).
        // Treat it as non-fatal: a missing title does not invalidate the session.
        let title = self
            .page
            .get_title()
            .await
            .unwrap_or_default() // Result<Option<String>, CdpError> → Option<String>
            .unwrap_or_default(); // Option<String> → String (empty if None)

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

        // Phase 7C, slice 7C.5 — settle delay then opportunistic
        // wait-for-navigation: if the click triggered a route change we
        // catch the new page; if not the timeout falls through harmlessly.
        tokio::time::sleep(Duration::from_millis(DELAY_AFTER_CLICK_MS)).await;
        let _ = tokio::time::timeout(
            Duration::from_millis(POST_ACTION_NAV_TIMEOUT_MS),
            self.page.wait_for_navigation(),
        )
        .await;
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

        // Phase 7C, slice 7C.5 — pressing Enter on an input usually
        // triggers form submit / search, so we wait longer then
        // opportunistically follow the navigation.
        if press_enter {
            self.dispatch_key("Enter").await?;
            tokio::time::sleep(Duration::from_millis(DELAY_AFTER_TYPE_ENTER_MS)).await;
            let _ = tokio::time::timeout(
                Duration::from_millis(POST_ACTION_NAV_TIMEOUT_MS),
                self.page.wait_for_navigation(),
            )
            .await;
        } else {
            tokio::time::sleep(Duration::from_millis(DELAY_AFTER_TYPE_MS)).await;
        }
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

        tokio::time::sleep(Duration::from_millis(DELAY_AFTER_SCROLL_MS)).await;
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
        // Use serde_json for complete JSON-string escaping (handles `\`, `"`,
        // newlines, and all other control characters).
        let escaped_json = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned());
        let js = format!(
            r#"(function(){{
                var el = document.querySelector('[data-if2ai-ref="{ref_num}"]');
                if (!el) return {{ ok: false }};
                el.value = {escaped_json};
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
        tokio::time::sleep(Duration::from_millis(DELAY_AFTER_KEY_MS)).await;
        // Enter / Esc / Space / Tab can all trigger navigation; mirror
        // the click path's opportunistic wait.
        let _ = tokio::time::timeout(
            Duration::from_millis(POST_ACTION_NAV_TIMEOUT_MS),
            self.page.wait_for_navigation(),
        )
        .await;
        self.sync_url().await;

        let snapshot = self.run_snapshot().await?;
        self.log("key", serde_json::json!({ "key": key }), "ok: key pressed");
        Ok(snapshot)
    }

    /// Wait up to `timeout_ms` for the requested lifecycle [`WaitState`]
    /// and return a fresh AXTree snapshot.
    ///
    /// Phase 7C, slice 7C.5 replaced the legacy "always call
    /// `wait_for_navigation`" stub with a real CDP listener for each
    /// state.  When the state never fires before the timeout we still
    /// return a snapshot rather than an error — Playwright-style
    /// best-effort.
    pub async fn wait(
        &mut self,
        timeout_ms: u64,
        state: WaitState,
    ) -> Result<String, BrowserError> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        match state {
            WaitState::DomContentLoaded => {
                if let Ok(mut events) = self
                    .page
                    .event_listener::<EventDomContentEventFired>()
                    .await
                {
                    let _ = tokio::time::timeout_at(deadline.into(), events.next()).await;
                }
            }
            WaitState::Load => {
                if let Ok(mut events) = self.page.event_listener::<EventLoadEventFired>().await {
                    let _ = tokio::time::timeout_at(deadline.into(), events.next()).await;
                }
            }
            WaitState::NetworkIdle => {
                self.wait_network_idle(deadline).await;
            }
        }

        self.sync_url().await;
        self.run_snapshot().await
    }

    /// Watch CDP `Network.requestWillBeSent` / `loadingFinished` /
    /// `loadingFailed` until the inflight count has been zero for at
    /// least [`NETWORK_IDLE_QUIET_WINDOW_MS`], or until `deadline`.
    async fn wait_network_idle(&self, deadline: Instant) {
        // Each listener subscribe is fallible (the page may have just
        // closed); drop the whole wait and just return on subscribe
        // failure — caller falls through to snapshot anyway.
        let mut started = match self.page.event_listener::<EventRequestWillBeSent>().await {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut finished = match self.page.event_listener::<EventLoadingFinished>().await {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut failed = match self.page.event_listener::<EventLoadingFailed>().await {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut inflight: i64 = 0;
        let mut last_zero_at = Instant::now();
        let quiet = Duration::from_millis(NETWORK_IDLE_QUIET_WINDOW_MS);
        let tick = Duration::from_millis(NETWORK_IDLE_TICK_MS);

        loop {
            if Instant::now() >= deadline {
                return;
            }
            tokio::select! {
                evt = started.next() => {
                    if evt.is_some() { inflight += 1; }
                }
                evt = finished.next() => {
                    if evt.is_some() {
                        inflight = (inflight - 1).max(0);
                        if inflight == 0 { last_zero_at = Instant::now(); }
                    }
                }
                evt = failed.next() => {
                    if evt.is_some() {
                        inflight = (inflight - 1).max(0);
                        if inflight == 0 { last_zero_at = Instant::now(); }
                    }
                }
                _ = tokio::time::sleep(tick) => {
                    if inflight == 0 && last_zero_at.elapsed() >= quiet {
                        return;
                    }
                }
            }
        }
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
            // Find a safe character boundary to avoid slicing in the middle of
            // a multi-byte UTF-8 sequence (e.g., Chinese characters / emoji).
            let cut = serialised
                .char_indices()
                .nth(30_000)
                .map(|(i, _)| i)
                .unwrap_or(serialised.len());
            Ok(format!("{}\n[output truncated]", &serialised[..cut]))
        } else {
            Ok(serialised)
        }
    }

    // ── Multi-tab support (Phase 7C, slice 7C.6) ──────────────────────────────
    //
    // We keep `self.page` as the canonical "active" page so the 30+
    // existing `self.page.evaluate(...)` call sites stay untouched.  Other
    // open tabs are enumerated lazily via `browser.pages()` whenever the
    // LLM asks (`list_tabs` / `switch_tab` / `close_tab`).  This avoids
    // standing up a background `EventTargetCreated` listener task whose
    // lifetime would have to be tied to the session.

    /// Enumerate every page currently held by the underlying Browser
    /// process and return a [`TabInfo`] vec ordered by discovery.  The
    /// active page (`self.page`) is flagged with `active=true`; its
    /// position in the returned list is also `self.tab_idx`.
    ///
    /// When the live tab count exceeds [`MAX_TRACKED_TABS`], the oldest
    /// *non-active* page is force-closed before returning so the list
    /// never grows unboundedly.
    pub async fn list_tabs(&mut self) -> Result<Vec<TabInfo>, BrowserError> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Enforce hard cap by closing the oldest non-active page.
        let active_id_string = self.page.target_id().as_ref().to_owned();
        if pages.len() > MAX_TRACKED_TABS {
            // Collect candidates, then take ownership of the one to close
            // (Page::close consumes self).
            let owned: Vec<Page> = pages.into_iter().collect();
            for page in owned {
                if page.target_id().as_ref() != active_id_string.as_str() {
                    let _ = page.close().await;
                    break;
                }
            }
        }

        // Re-fetch after potential cleanup.
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        let mut out = Vec::with_capacity(pages.len());
        for (idx, page) in pages.iter().enumerate() {
            let url = page.url().await.ok().flatten().unwrap_or_default();
            let title = page.get_title().await.ok().flatten().unwrap_or_default();
            let target_id = page.target_id().as_ref().to_owned();
            let active = target_id == active_id_string;
            out.push(TabInfo {
                idx,
                url,
                title,
                active,
                target_id,
            });
        }
        Ok(out)
    }

    /// Make the page at `idx` the active one.  Returns a fresh AXTree
    /// snapshot of the newly-active page.  Future click / type / snapshot
    /// calls will operate on it.
    pub async fn switch_tab(&mut self, idx: usize) -> Result<String, BrowserError> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        let page = pages
            .into_iter()
            .nth(idx)
            .ok_or(BrowserError::TabIndexOutOfRange(idx))?;
        // Re-inject stealth on the new page so vendor JS that reads
        // navigator.webdriver still sees a clean slate.  Best-effort:
        // some pages (e.g. about:blank with no document) reject the
        // injection — that's harmless.
        let _ = page
            .enable_stealth_mode_with_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/131.0.0.0 Safari/537.36",
            )
            .await;
        self.page = page;
        self.sync_url().await;
        let snapshot = self.run_snapshot().await?;
        self.log("switch_tab", serde_json::json!({ "idx": idx }), "ok");
        Ok(snapshot)
    }

    /// Close the page at `idx`.  When the active page is closed the
    /// next remaining page (or the previous one when none follows)
    /// becomes active automatically; the caller can then call
    /// `snapshot` or `switch_tab` to refresh.  Returns the count of
    /// pages remaining.
    pub async fn close_tab(&mut self, idx: usize) -> Result<usize, BrowserError> {
        // Take ownership so we can `close()` (which consumes the Page).
        let target_page = self
            .browser
            .pages()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?
            .into_iter()
            .nth(idx)
            .ok_or(BrowserError::TabIndexOutOfRange(idx))?;
        let target_id_string = target_page.target_id().as_ref().to_owned();
        let was_active = target_id_string.as_str() == self.page.target_id().as_ref();
        target_page
            .close()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;

        // Re-fetch the live list and pick a new active page when needed.
        let pages_after = self
            .browser
            .pages()
            .await
            .map_err(|e| BrowserError::Cdp(e.to_string()))?;
        if was_active {
            if let Some(replacement) = pages_after
                .into_iter()
                .find(|p| p.target_id().as_ref() != target_id_string.as_str())
            {
                self.page = replacement;
                self.sync_url().await;
            }
            // Else: no pages left.  Caller is expected to close() the
            // session shortly thereafter; we leave self.page pointing at
            // the (now closed) handle which will surface as a CDP error
            // on the next operation.
        }
        let remaining = self.browser.pages().await.map(|p| p.len()).unwrap_or(0);
        self.log(
            "close_tab",
            serde_json::json!({ "idx": idx }),
            &format!("ok: {remaining} remaining"),
        );
        Ok(remaining)
    }

    /// Append a "## Other tabs" section to `snapshot` when there are
    /// more than one live page.  Used by `run_snapshot` callers (and
    /// the tool layer) so the LLM is always reminded which tabs exist.
    pub async fn append_tabs_summary(&mut self, snapshot: String) -> String {
        let tabs = match self.list_tabs().await {
            Ok(t) => t,
            Err(_) => return snapshot,
        };
        if tabs.len() <= 1 {
            return snapshot;
        }
        let mut out = snapshot;
        out.push_str("\n\n## Other tabs (use action='switch_tab' with tab_index):\n");
        for tab in &tabs {
            let marker = if tab.active { "*" } else { " " };
            let title = if tab.title.is_empty() {
                "(no title)".to_string()
            } else {
                tab.title.chars().take(60).collect::<String>()
            };
            let url = if tab.url.is_empty() {
                "about:blank".to_string()
            } else {
                tab.url.chars().take(120).collect::<String>()
            };
            out.push_str(&format!("[{}] {} {} — {}\n", tab.idx, marker, url, title));
        }
        out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_state_from_str_recognises_load() {
        assert_eq!(WaitState::from_str("load"), WaitState::Load);
        assert_eq!(WaitState::from_str("LOAD"), WaitState::Load);
    }

    #[test]
    fn wait_state_from_str_recognises_networkidle_aliases() {
        assert_eq!(WaitState::from_str("networkidle"), WaitState::NetworkIdle);
        assert_eq!(WaitState::from_str("network_idle"), WaitState::NetworkIdle);
        assert_eq!(WaitState::from_str("network-idle"), WaitState::NetworkIdle);
        assert_eq!(WaitState::from_str("NetworkIdle"), WaitState::NetworkIdle);
    }

    #[test]
    fn wait_state_from_str_defaults_to_dom_content_loaded() {
        assert_eq!(
            WaitState::from_str("domcontentloaded"),
            WaitState::DomContentLoaded
        );
        // Unknown / typo -> default to DomContentLoaded (Playwright-style).
        assert_eq!(
            WaitState::from_str("anything-unknown"),
            WaitState::DomContentLoaded
        );
        assert_eq!(WaitState::from_str(""), WaitState::DomContentLoaded);
    }

    #[test]
    fn delay_constants_are_above_pre_7c5_baseline() {
        // Phase 7C, slice 7C.5 raised every default delay; this guard
        // prevents accidental regression to the old 150-200ms values
        // that caused double-clicks on SPA pages.
        assert!(DELAY_AFTER_CLICK_MS >= 500);
        assert!(DELAY_AFTER_TYPE_MS >= 250);
        assert!(DELAY_AFTER_TYPE_ENTER_MS >= 600);
        assert!(DELAY_AFTER_SCROLL_MS >= 300);
        assert!(DELAY_AFTER_KEY_MS >= 250);
    }
}

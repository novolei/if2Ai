//! Extended lifecycle hooks inspired by Steward `hooks/` (priority registry + optional webhook).
//!
//! v1 wires `BeforeInbound` / `BeforeOutbound` from streaming turns. Shell hooks remain in
//! [`super::hooks::HookRunner`] for tool use.
//!
//! P0-4 dual-track contract:
//! - **`BeforeToolCall`** lives here (HTTP / programmable lifecycle hooks).
//! - **`PreToolUse` / `PostToolUse`** in [`super::hooks::HookRunner`] are
//!   shell-script hooks executed by `tool_executor`. Both are first-class
//!   and may co-exist; the merge to a single hook seam is a follow-up
//!   pack and intentionally NOT done here. See
//!   [`HookRegistry::run_before_tool_call`] for the lifecycle entry-point.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use tokio::sync::RwLock;

/// Steward-compatible hook points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    BeforeInbound,
    BeforeToolCall,
    BeforeOutbound,
    OnSessionStart,
    OnSessionEnd,
    TransformResponse,
}

#[derive(Debug, Clone)]
pub enum HookOutcome {
    Pass,
    Modify(String),
    Reject(String),
    Warn(String),
}

#[derive(Debug, Serialize)]
struct WebhookPayload<'a> {
    hook_point: HookPoint,
    session_id: Option<&'a str>,
    text_preview: &'a str,
}

#[async_trait]
pub trait LifecycleHook: Send + Sync {
    fn name(&self) -> &'static str;
    fn priority(&self) -> u32 {
        100
    }
    async fn fire(&self, point: HookPoint, session_id: Option<&str>, text: &str) -> HookOutcome;
}

struct WebhookLifecycleHook {
    url: String,
    client: reqwest::Client,
}

#[async_trait]
impl LifecycleHook for WebhookLifecycleHook {
    fn name(&self) -> &'static str {
        "webhook_lifecycle"
    }

    fn priority(&self) -> u32 {
        300
    }

    async fn fire(&self, point: HookPoint, session_id: Option<&str>, text: &str) -> HookOutcome {
        let preview: String = text.chars().take(4000).collect();
        let body = WebhookPayload {
            hook_point: point,
            session_id,
            text_preview: preview.as_str(),
        };
        match self
            .client
            .post(&self.url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HookOutcome::Pass,
            Ok(resp) => HookOutcome::Warn(format!("lifecycle webhook returned {}", resp.status())),
            Err(e) => HookOutcome::Warn(format!("lifecycle webhook error: {e}")),
        }
    }
}

struct HookEntry {
    priority: u32,
    hook: Arc<dyn LifecycleHook>,
}

/// Async hook registry (lower `priority` runs first).
pub struct HookRegistry {
    hooks: RwLock<Vec<HookEntry>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(Vec::new()),
        }
    }

    pub async fn register(&self, hook: Arc<dyn LifecycleHook>, priority: u32) {
        let mut g = self.hooks.write().await;
        g.push(HookEntry { priority, hook });
        g.sort_by_key(|e| e.priority);
    }

    /// Convenience wrapper for the `BeforeToolCall` lifecycle point.
    /// Pass the tool name + serialized JSON args (after redaction) as
    /// `text` so HTTP webhook hooks can inspect / reject the call.
    pub async fn run_before_tool_call(
        &self,
        session_id: Option<&str>,
        tool_name: &str,
        redacted_args_json: &str,
    ) -> Result<String, String> {
        let payload = format!("{tool_name}\u{1F}{redacted_args_json}");
        self.run_point(HookPoint::BeforeToolCall, session_id, payload)
            .await
    }

    /// Run all hooks for `point` on `text`. Returns first `Reject` or applies `Modify` in order.
    pub async fn run_point(
        &self,
        point: HookPoint,
        session_id: Option<&str>,
        mut text: String,
    ) -> Result<String, String> {
        let hooks: Vec<Arc<dyn LifecycleHook>> = {
            let g = self.hooks.read().await;
            g.iter().map(|e| e.hook.clone()).collect()
        };
        for hook in hooks {
            match hook.fire(point, session_id, &text).await {
                HookOutcome::Pass => {}
                HookOutcome::Warn(w) => {
                    tracing::warn!(hook = hook.name(), "{w}");
                }
                HookOutcome::Modify(new) => {
                    text = new;
                }
                HookOutcome::Reject(reason) => {
                    return Err(reason);
                }
            }
        }
        Ok(text)
    }
}

const WEBHOOK_HOOK_PRIORITY: u32 = 300;

/// Build a registry with an optional outbound webhook when `IF2AI_LIFECYCLE_WEBHOOK_URL` is set.
#[must_use]
pub async fn build_default_registry() -> Arc<HookRegistry> {
    let reg = Arc::new(HookRegistry::new());
    if let Ok(url) = std::env::var("IF2AI_LIFECYCLE_WEBHOOK_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            reg.register(
                Arc::new(WebhookLifecycleHook { url, client }),
                WEBHOOK_HOOK_PRIORITY,
            )
            .await;
        }
    }
    reg
}

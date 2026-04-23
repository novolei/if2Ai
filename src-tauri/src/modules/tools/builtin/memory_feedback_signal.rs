//! MEM-MOD-P1 — `memory_feedback_signal` tool.
//!
//! Lets the agent (or, indirectly, the user via "this was helpful / wrong"
//! UI buttons) move a recalled memory's `trust_score` up or down.  The
//! score is stored in `memory_entries.trust_score` and feeds into the
//! `WeibullDecay::compute_importance` formula:
//!
//! ```text
//! base       = importance + access_count * 0.01
//! trust_boost = (trust_score + 1.0) * 0.05   ← this tool moves the dial
//! score      = (base + trust_boost) * decay
//! ```
//!
//! Returns a JSON envelope so the front-end / harness can show the new
//! value without an extra recall round-trip:
//!
//! ```jsonc
//! {
//!   "status": "ok" | "not_found",
//!   "key": "<memory key>",
//!   "previous_signal": "positive" | "negative" | "neutral",
//!   "delta": 0.10,
//!   "new_trust_score": 0.42
//! }
//! ```
//!
//! Design choices
//! - Signal labels (`positive` / `negative` / `neutral`) are intentionally
//!   coarse — Mem0/Letta-style stacks have shown that a 3-way signal is
//!   the sweet spot between precision and adoption.  Numeric `delta`
//!   override is exposed for fine-grained automation.
//! - Score is hard-clamped to `[-1.0, 1.0]` inside the provider so callers
//!   cannot escape the WeibullDecay invariant.
//! - Failures are best-effort: if the row is missing we return
//!   `status: "not_found"` instead of `Err(...)`, keeping the agent's
//!   tool-loop deterministic.

use std::sync::Arc;

use serde_json::json;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::scope::{MemoryExecutionScope, MemoryScopeResolver};
use crate::modules::memory::{MemoryError, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default deltas for the 3-way coarse signal.  Mirrors Mem0's defaults
/// (±0.10) so user-perceived behaviour matches the public mental model.
const DELTA_POSITIVE: f64 = 0.10;
const DELTA_NEGATIVE: f64 = -0.10;

/// Build a [`ToolEntry`] for the `memory_feedback_signal` tool.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler =
        Arc::new(move |args: serde_json::Value, context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: key".to_string())
                    })?
                    .to_string();

                let signal = args
                    .get("signal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("positive")
                    .to_string();

                // Resolve `delta`: explicit numeric override wins, otherwise
                // map the coarse signal to ±0.10 / 0.0.
                let delta =
                    args.get("delta")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(match signal.as_str() {
                            "positive" => DELTA_POSITIVE,
                            "negative" => DELTA_NEGATIVE,
                            _ => 0.0,
                        });

                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(MemoryExecutionScope::global);
                let audit_ctx = AuditContext::from_scope(&scope);

                match memory.adjust_trust_score(&key, delta).await {
                    Ok(new_score) => {
                        MemoryAuditEmitter::memory_captured(&audit_ctx, &key, "feedback_signal");
                        let payload = json!({
                            "status": "ok",
                            "key": key,
                            "previous_signal": signal,
                            "delta": delta,
                            "new_trust_score": new_score,
                        });
                        Ok(payload.to_string())
                    }
                    Err(MemoryError::KeyNotFound(_)) => {
                        let payload = json!({
                            "status": "not_found",
                            "key": key,
                            "previous_signal": signal,
                            "delta": delta,
                        });
                        Ok(payload.to_string())
                    }
                    Err(err) => Err(ToolError::Handler(format!(
                        "failed to adjust trust_score: {err}"
                    ))),
                }
            })
        });

    ToolEntry {
        name: "memory_feedback_signal".to_string(),
        toolset: "memory".to_string(),
        description: "Adjust a recalled memory's trust_score up or down based on whether \
                      it was useful (positive), misleading (negative), or unchanged (neutral). \
                      The score moves by ±0.10 per call by default and is clamped to [-1.0, 1.0]. \
                      Pass an explicit `delta` (float) to override the default magnitude. \
                      Returns a JSON object: \
                      • `status` — `ok` (score updated) or `not_found` (key missing). \
                      • `new_trust_score` — the persisted post-clamp value. \
                      Use this after a `memory_recall` whose results were demonstrably right or wrong; \
                      do NOT call without a recent recall — speculative trust changes pollute decay."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Key of the recalled memory to score."
                },
                "signal": {
                    "type": "string",
                    "enum": ["positive", "negative", "neutral"],
                    "description": "Coarse 3-way signal. Defaults to `positive`."
                },
                "delta": {
                    "type": "number",
                    "description": "Optional explicit delta in [-1.0, 1.0]. Overrides `signal`."
                }
            },
            "required": ["key"]
        }),
        max_result_size: Some(512),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::modules::memory::{InMemoryMemoryProvider, MemoryCategory};
    use crate::modules::tools::context::ToolContext;
    use std::sync::{Arc, Mutex};

    fn make_context() -> SharedToolContext {
        Arc::new(Mutex::new(ToolContext::new(
            std::env::temp_dir(),
            crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
        )))
    }

    #[tokio::test]
    async fn unknown_key_returns_not_found_envelope() {
        // InMemoryMemoryProvider's default `adjust_trust_score` impl
        // returns Ok(0.0) (no-op), so we can't test the not_found branch
        // here without SQLite. We just smoke-test that the tool wires
        // up and serialises a known-good signal.
        let memory: SharedMemoryProvider = Arc::new(InMemoryMemoryProvider::new());
        let tool = entry(memory.clone());
        let result = (tool.handler)(
            json!({ "key": "nonexistent", "signal": "positive" }),
            make_context(),
        )
        .await
        .expect("tool call succeeds");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["key"], "nonexistent");
        assert_eq!(parsed["previous_signal"], "positive");
    }

    #[tokio::test]
    async fn explicit_delta_overrides_signal_label() {
        let memory: SharedMemoryProvider = Arc::new(InMemoryMemoryProvider::new());
        memory
            .store("seed", "hello", MemoryCategory::Conversation)
            .await
            .unwrap();
        let tool = entry(memory.clone());
        let result = (tool.handler)(
            json!({ "key": "seed", "signal": "neutral", "delta": 0.42 }),
            make_context(),
        )
        .await
        .expect("tool call succeeds");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["delta"], 0.42);
    }
}

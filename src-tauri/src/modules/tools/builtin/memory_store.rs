//! Memory Store tool — stores a fact in long-term memory.
//!
//! Provides persistent memory storage across sessions, scoped to the current
//! session when a `session_id` is available in the tool context.
//!
//! # Policy decisions surfaced to the frontend
//!
//! The handler returns a **JSON string** (parsed by `App.tsx` /
//! `MemoryStoreToolCard`) so the UI can render allow / prompt / deny states
//! consistently with the audit log.  Schema:
//!
//! ```jsonc
//! {
//!   "status": "stored" | "pending_approval" | "denied",
//!   "key": "<memory key>",
//!   "category": "core" | "daily" | "conversation" | "<custom>",
//!   "scope": "global" | "project" | "session",
//!   "policy_decision": "allow" | "prompt" | "deny",
//!   "reason_code": "<ReasonCode::label()>",
//!   "message": "<human-readable explanation>",
//!   "content_preview": "<first 120 chars of content>"   // pending_approval only
//! }
//! ```
//!
//! # Enforce-mode wiring
//!
//! The handler reads `~/.if2ai/memory_config.json` on every invocation to
//! discover the user-selected `policy_enforce_mode` written by the Memory
//! Settings UI, then constructs a per-call `MemoryPolicyEngine`.  This makes
//! UI toggles take effect immediately without restarting the app, at the cost
//! of one small JSON read per `memory_store` call (acceptable: this tool is
//! invoked at most a few times per turn).
//!
//! The `Prompt` decision **does not persist** the write — the tool returns a
//! `pending_approval` payload so the agent can surface the request to the
//! user instead of silently writing.  Approval / rejection of pending writes
//! is handled by a separate IPC path (future work).

use std::sync::Arc;

use serde_json::json;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::decision_tree::{decide_via_llm, DecisionPlan};
use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::policy::{
    MemoryPolicyConfig, MemoryPolicyEngine, PolicyDecision, PolicyEnforceMode,
};
use crate::modules::memory::scope::{MemoryExecutionScope, MemoryScopeResolver};
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::{MemoryCategory, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum content length, in bytes, before a write is hard-denied.
const POLICY_MAX_CONTENT_BYTES: usize = 10_000;

/// Content length, in bytes, above which the policy engine returns `Prompt`
/// (user must approve before the write is persisted).
const POLICY_PROMPT_THRESHOLD_BYTES: usize = 2_000;

/// Length of the `content_preview` field included in `pending_approval`
/// responses so the agent can describe the pending write to the user without
/// echoing the whole payload back to the LLM.
const PENDING_APPROVAL_PREVIEW_CHARS: usize = 120;

/// Test-only override for the enforcement mode.  Production paths must NOT
/// touch any of this — it exists solely so unit tests can drive `Enforce`
/// mode without mutating the user's real `~/.if2ai/memory_config.json`.
///
/// Two-tier design:
/// - `TEST_ENFORCE_VALUE` (AtomicU8): the actual override the loader reads
///   on every call (lock-free, so handlers do not deadlock against the
///   guard mutex while themselves running inside `set` → handler).
/// - `TEST_ENFORCE_LOCK` (Mutex): serializes guards across concurrent
///   tokio tests so two guards cannot clobber each other's value.
#[cfg(test)]
mod test_override {
    use super::PolicyEnforceMode;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(super) const NONE: u8 = 0;
    pub(super) const SHADOW: u8 = 1;
    pub(super) const ENFORCE: u8 = 2;

    pub(super) static VALUE: AtomicU8 = AtomicU8::new(NONE);

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_cell() -> &'static Mutex<()> {
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(super) fn read() -> Option<PolicyEnforceMode> {
        match VALUE.load(Ordering::SeqCst) {
            SHADOW => Some(PolicyEnforceMode::Shadow),
            ENFORCE => Some(PolicyEnforceMode::Enforce),
            _ => None,
        }
    }

    pub(super) struct TestEnforceGuard {
        _serial: MutexGuard<'static, ()>,
    }

    impl TestEnforceGuard {
        pub(super) fn set(mode: PolicyEnforceMode) -> Self {
            let serial = match lock_cell().lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let raw = match mode {
                PolicyEnforceMode::Shadow => SHADOW,
                PolicyEnforceMode::Enforce => ENFORCE,
            };
            VALUE.store(raw, Ordering::SeqCst);
            Self { _serial: serial }
        }
    }

    impl Drop for TestEnforceGuard {
        fn drop(&mut self) {
            VALUE.store(NONE, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
use test_override::TestEnforceGuard;

/// Resolve the user-selected enforcement mode by reading
/// `~/.if2ai/memory_config.json`.  This is the same file the Memory Settings
/// UI writes to via [`crate::commands::settings::set_memory_config`], so any
/// toggle in the UI takes effect on the next `memory_store` invocation
/// without requiring an app restart.
///
/// On any read / parse error this falls back to the safe default
/// (`Shadow`), matching the behaviour of [`MemoryPolicyConfig::default`].
fn load_enforce_mode_from_disk() -> PolicyEnforceMode {
    #[cfg(test)]
    {
        if let Some(mode) = test_override::read() {
            return mode;
        }
    }
    let Ok(home) = std::env::var("HOME") else {
        return PolicyEnforceMode::default();
    };
    let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return PolicyEnforceMode::default();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return PolicyEnforceMode::default();
    };
    match parsed
        .get("policy_enforce_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("shadow")
    {
        "enforce" => PolicyEnforceMode::Enforce,
        _ => PolicyEnforceMode::Shadow,
    }
}

/// Build the per-call `MemoryPolicyEngine` using the user-configured enforce
/// mode plus the static byte / category limits defined above.
fn build_policy_engine() -> MemoryPolicyEngine {
    let enforce_mode = load_enforce_mode_from_disk();
    let enforce_label = match enforce_mode {
        PolicyEnforceMode::Shadow => "shadow",
        PolicyEnforceMode::Enforce => "enforce",
    };
    tracing::debug!(
        target: "memory.policy",
        enforce_mode = enforce_label,
        max_content_bytes = POLICY_MAX_CONTENT_BYTES,
        prompt_threshold_bytes = POLICY_PROMPT_THRESHOLD_BYTES,
        "[memory_store] constructed policy engine for write evaluation",
    );
    MemoryPolicyEngine::new(MemoryPolicyConfig {
        enforce_mode,
        max_content_bytes: POLICY_MAX_CONTENT_BYTES,
        prompt_threshold_bytes: POLICY_PROMPT_THRESHOLD_BYTES,
        denied_categories: vec![],
    })
}

/// Map a `MemoryExecutionScope` to the wire-format scope label consumed by
/// the frontend (`global` / `project` / `session`).  We prefer the most
/// specific binding the scope has, matching the precedence used by
/// `MemoryWriteCard`.
fn scope_label(scope: &MemoryExecutionScope) -> &'static str {
    if scope.session_id.is_some() {
        "session"
    } else if scope.project_id.is_some() {
        "project"
    } else {
        "global"
    }
}

/// Wire-format label for a `PolicyDecision` matching the frontend type
/// `'allow' | 'deny' | 'prompt'`.
fn decision_label(decision: &PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Deny => "deny",
        PolicyDecision::Prompt => "prompt",
    }
}

/// Truncate `content` to at most `max_chars` Unicode characters for inclusion
/// in a `pending_approval` payload.  We intentionally truncate by characters
/// (not bytes) to avoid splitting multi-byte sequences.
fn content_preview(content: &str, max_chars: usize) -> String {
    let mut out: String = content.chars().take(max_chars).collect();
    if content.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// Creates the memory_store tool entry for the registry.
///
/// MEM-MOD-P4 — `utility_llm` is consulted by the Mem0-style update
/// decision tree before persisting, but only when the user has flipped
/// `MemoryFeatureConfig::decision_tree_enabled` on (defaults `false`).
/// When the flag is off OR no LLM is provided, the historical "always
/// add" path is taken byte-identical to the pre-P4 behaviour.
#[allow(dead_code)]
#[must_use]
pub fn entry(
    memory: SharedMemoryProvider,
    utility_llm: Option<Arc<dyn UtilityLlm>>,
) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, context: SharedToolContext| {
            let memory = memory.clone();
            let utility_llm = utility_llm.clone();
            Box::pin(async move {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: key".to_string())
                    })?
                    .to_string();

                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: content".to_string())
                    })?
                    .to_string();

                let category = args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(|c| match c {
                        "core" => MemoryCategory::Core,
                        "daily" => MemoryCategory::Daily,
                        "conversation" => MemoryCategory::Conversation,
                        // MEM-MOD-P2 — three new built-in facets surfaced to LLMs.
                        "working" => MemoryCategory::Working,
                        "procedural" => MemoryCategory::Procedural,
                        "reflection" => MemoryCategory::Reflection,
                        other => MemoryCategory::Custom(other.to_string()),
                    })
                    .unwrap_or(MemoryCategory::Conversation);

                // Resolve scope from the tool execution context.  When a session_id
                // is present, the entry is tagged so recall_scoped() can filter by session.
                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(MemoryExecutionScope::global);

                let audit_ctx = AuditContext::from_scope(&scope);

                // Emit: memory captured (agent has identified info to remember).
                MemoryAuditEmitter::memory_captured(&audit_ctx, &key, category.as_str());

                // M5: scan for credentials / secrets *before* policy evaluation.
                // The scanner is shadow-only here (logs + audit) — actual
                // blocking is delegated to MemoryPolicyEngine, which can be
                // configured to enforce based on the threat category.  This
                // keeps the security layer composable: scanner reports a fact,
                // policy decides what to do about it.
                let threat = ThreatScanner::with_builtin_patterns().scan(&key, &content);
                if threat.flagged {
                    tracing::warn!(
                        target: "memory.security",
                        key = %key,
                        category = %threat.category,
                        description = %threat.description,
                        "[ThreatScanner] candidate memory write contains a possible secret",
                    );
                    MemoryAuditEmitter::memory_write_decision(
                        &audit_ctx,
                        &key,
                        &PolicyDecision::Prompt,
                        &crate::modules::memory::policy::ReasonCode::ThreatScannerMatch,
                        &format!(
                            "ThreatScanner matched {}: {}",
                            threat.category, threat.description
                        ),
                    );
                }

                // Evaluate write policy.  The engine's enforce_mode is loaded
                // from `~/.if2ai/memory_config.json` on each invocation so UI
                // toggles take effect immediately (see `build_policy_engine`).
                let policy = build_policy_engine();
                let policy_result = policy.evaluate_write(&key, &content, &category, &scope);

                // Emit: write decision (allow / deny / prompt + reason_code).
                MemoryAuditEmitter::memory_write_decision(
                    &audit_ctx,
                    &key,
                    &policy_result.decision,
                    &policy_result.reason_code,
                    &policy_result.message,
                );

                let scope_str = scope_label(&scope);
                let category_str = category.as_str().to_string();
                let reason_code_str = policy_result.reason_code.label();

                // Branch on the policy decision.
                //
                // - `Deny`   → never persist; return a `denied` JSON payload as
                //              `Ok(...)` so the structured fields reach the
                //              frontend untouched.  We deliberately do NOT use
                //              `Err(ToolError::Handler(...))` here because
                //              `ToolError::Handler`'s `Display` impl prefixes
                //              its message with `"tool handler error: "`,
                //              which would break the JSON envelope expected by
                //              `App.tsx::extractMemoryStoreFields` and prevent
                //              the highlighted `MemoryWriteCard` from
                //              rendering.  The LLM still understands this as
                //              a failed write because `status == "denied"`.
                //              In `Shadow` mode the engine downgrades Deny to
                //              Allow so this branch only fires in `Enforce`.
                // - `Prompt` → never persist; return a `pending_approval` JSON
                //              payload so the UI can render the approval card
                //              and the agent knows the write is awaiting user
                //              confirmation (it must NOT retry).
                // - `Allow`  → persist, return a `stored` payload.
                if policy_result.decision == PolicyDecision::Deny {
                    MemoryAuditEmitter::memory_rejected(
                        &audit_ctx,
                        &key,
                        &policy_result.reason_code,
                        &policy_result.message,
                    );
                    let denied = json!({
                        "status": "denied",
                        "key": key,
                        "category": category_str,
                        "scope": scope_str,
                        "policy_decision": decision_label(&policy_result.decision),
                        "reason_code": reason_code_str,
                        "message": policy_result.message,
                    });
                    return Ok(denied.to_string());
                }

                if policy_result.decision == PolicyDecision::Prompt {
                    let pending = json!({
                        "status": "pending_approval",
                        "key": key,
                        "category": category_str,
                        "scope": scope_str,
                        "policy_decision": decision_label(&policy_result.decision),
                        "reason_code": reason_code_str,
                        "message": policy_result.message,
                        "content_preview": content_preview(&content, PENDING_APPROVAL_PREVIEW_CHARS),
                    });
                    return Ok(pending.to_string());
                }

                // MEM-MOD-P4 — Mem0-style update decision tree.  Only
                // consulted when the user has flipped the feature flag
                // AND a utility LLM was injected into this tool.  We
                // recall up to 5 candidate memories whose content
                // overlaps the new fact, ask the LLM whether to NOOP
                // / ADD / UPDATE / DELETE, and act on the verb.  Any
                // failure (LLM unreachable, bad JSON, unknown key)
                // falls back to plain ADD so a pathological classifier
                // can never *drop* a real fact.
                let decision_enabled =
                    crate::modules::runtime::config::current().memory().decision_tree_enabled();
                let decision: DecisionPlan = if decision_enabled {
                    if let Some(ref llm) = utility_llm {
                        let candidates = memory
                            .recall_scoped(&content, None, 5, &scope)
                            .await
                            .unwrap_or_default();
                        decide_via_llm(llm.as_ref(), &content, &candidates)
                            .await
                            .unwrap_or(DecisionPlan::Add)
                    } else {
                        DecisionPlan::Add
                    }
                } else {
                    DecisionPlan::Add
                };

                match decision {
                    DecisionPlan::NoOp { covered_by } => {
                        let payload = json!({
                            "status": "noop",
                            "key": key,
                            "category": category_str,
                            "scope": scope_str,
                            "policy_decision": decision_label(&policy_result.decision),
                            "reason_code": "decision_tree_noop",
                            "message": "memory_store: decision tree judged this fact already covered",
                            "covered_by": covered_by,
                        });
                        return Ok(payload.to_string());
                    }
                    DecisionPlan::Update { existing_key } => {
                        memory.update_content(&existing_key, &content).await.map_err(
                            |e| ToolError::Handler(format!("decision_tree update failed: {e}")),
                        )?;
                        MemoryAuditEmitter::memory_persisted(
                            &audit_ctx,
                            &existing_key,
                            category.as_str(),
                        );
                        let payload = json!({
                            "status": "updated",
                            "key": existing_key,
                            "original_key": key,
                            "category": category_str,
                            "scope": scope_str,
                            "reason_code": "decision_tree_update",
                            "message": "memory_store: decision tree refined an existing entry",
                        });
                        return Ok(payload.to_string());
                    }
                    DecisionPlan::Delete { existing_key } => {
                        let _ = memory.delete(&existing_key).await;
                        // Still persist the new content as the
                        // post-supersession value — Delete means "the
                        // OLD fact is wrong", NOT "drop both".
                        memory
                            .store_scoped(&key, &content, category.clone(), &scope)
                            .await
                            .map_err(|e| {
                                ToolError::Handler(format!("failed to store memory after supersede: {e}"))
                            })?;
                        MemoryAuditEmitter::memory_persisted(
                            &audit_ctx,
                            &key,
                            category.as_str(),
                        );
                        let payload = json!({
                            "status": "superseded",
                            "key": key,
                            "deleted_key": existing_key,
                            "category": category_str,
                            "scope": scope_str,
                            "reason_code": "decision_tree_delete",
                            "message": "memory_store: decision tree retired an obsolete entry then added the new one",
                        });
                        return Ok(payload.to_string());
                    }
                    DecisionPlan::Add => {
                        // fall through to the historical add path
                    }
                }

                memory
                    .store_scoped(&key, &content, category.clone(), &scope)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to store memory: {}", e)))?;

                // Emit: memory successfully persisted.
                MemoryAuditEmitter::memory_persisted(&audit_ctx, &key, category.as_str());

                let stored = json!({
                    "status": "stored",
                    "key": key,
                    "category": category_str,
                    "scope": scope_str,
                    "policy_decision": decision_label(&policy_result.decision),
                    "reason_code": reason_code_str,
                    "message": policy_result.message,
                });
                Ok(stored.to_string())
            })
        },
    );

    ToolEntry {
        name: "memory_store".to_string(),
        toolset: "memory".to_string(),
        description: "Store a fact in long-term memory. Returns a JSON object with `status` \
                      (one of `stored`, `pending_approval`, `denied`), `policy_decision`, \
                      `scope`, `reason_code`, and `message`. \
                      • `stored` — write succeeded. \
                      • `pending_approval` — the write was NOT persisted and is awaiting \
                      user approval; do not retry, surface the pending state to the user \
                      and continue with the rest of the task. \
                      • `denied` — the write was rejected by the memory policy engine and \
                      will not be persisted; do not retry the same content, surface the \
                      `reason_code` / `message` to the user and continue."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Unique key for this memory"
                },
                "content": {
                    "type": "string",
                    "description": "Content to store"
                },
                "category": {
                    "type": "string",
                    "description": "Category: core (durable identity / preferences), daily (logical-day rollups), conversation (per-turn scratch), working (active-task scratch), procedural (how-to recipes), reflection (meta-observations), or any custom string."
                }
            },
            "required": ["key", "content"]
        }),
        max_result_size: Some(2048),
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
    use crate::modules::memory::InMemoryMemoryProvider;
    use crate::modules::tools::context::ToolContext;
    use std::sync::Mutex;

    fn test_memory() -> SharedMemoryProvider {
        Arc::new(InMemoryMemoryProvider::new())
    }

    fn empty_context() -> SharedToolContext {
        Arc::new(Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )))
    }

    #[tokio::test]
    async fn memory_store_tool_entry_has_correct_structure() {
        let entry = entry(test_memory(), None);
        assert_eq!(entry.name, "memory_store");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }

    #[tokio::test]
    async fn allow_path_persists_and_returns_stored_json() {
        let _guard = TestEnforceGuard::set(PolicyEnforceMode::Shadow);
        let memory = test_memory();
        let entry = entry(memory.clone(), None);
        let result = (entry.handler)(
            json!({"key": "k1", "content": "small", "category": "daily"}),
            empty_context(),
        )
        .await
        .expect("allow path returns Ok");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("result is JSON");
        assert_eq!(parsed["status"], "stored");
        assert_eq!(parsed["policy_decision"], "allow");
        assert_eq!(parsed["key"], "k1");
        // ALLOW path persists — recall via the trait API.
        let recalled = memory.recall("k1", None, 10).await.expect("recall ok");
        assert!(
            recalled.iter().any(|e| e.content == "small"),
            "expected `small` in recalled entries, got {recalled:?}",
        );
    }

    #[tokio::test]
    async fn prompt_threshold_does_not_persist_and_returns_pending_approval() {
        let _guard = TestEnforceGuard::set(PolicyEnforceMode::Shadow);
        let memory = test_memory();
        let entry = entry(memory.clone(), None);
        // Build content > 2000 bytes but well under 10000 so we trip the
        // `LengthPromptThreshold` rule.
        let big = "x".repeat(3_000);
        let result = (entry.handler)(
            json!({"key": "long-key", "content": big.clone(), "category": "conversation"}),
            empty_context(),
        )
        .await
        .expect("prompt path still returns Ok (not an error)");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("result is JSON");
        assert_eq!(parsed["status"], "pending_approval");
        assert_eq!(parsed["policy_decision"], "prompt");
        assert_eq!(parsed["reason_code"], "length_prompt_threshold");
        assert!(parsed["content_preview"].as_str().is_some());
        // Critical: prompt MUST NOT persist — recall returns no entries.
        let recalled = memory
            .recall("long-key", None, 10)
            .await
            .expect("recall ok");
        assert!(
            !recalled.iter().any(|e| e.content == big),
            "prompt decisions must not persist; got {recalled:?}",
        );
    }

    #[tokio::test]
    async fn deny_in_shadow_mode_is_downgraded_and_persists() {
        // Pin shadow mode explicitly so this test is isolated from any
        // concurrently-running enforce-mode test (`TestEnforceGuard` is a
        // global override).  In Shadow a content-too-long write is
        // downgraded to Allow and persisted with reason_code=shadow_denied.
        let _guard = TestEnforceGuard::set(PolicyEnforceMode::Shadow);
        let memory = test_memory();
        let entry = entry(memory.clone(), None);
        let huge = "z".repeat(11_000); // > 10_000 byte hard limit.
        let result = (entry.handler)(
            json!({"key": "huge", "content": huge.clone()}),
            empty_context(),
        )
        .await
        .expect("shadow mode downgrades deny to allow");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("result is JSON");
        assert_eq!(parsed["status"], "stored");
        assert_eq!(parsed["policy_decision"], "allow");
        assert_eq!(parsed["reason_code"], "shadow_denied");
    }

    /// In `Enforce` mode a content-too-long write produces a real `Deny`
    /// decision.  Critical regression guard: the handler MUST return
    /// `Ok(json_string)` (not `Err(ToolError::Handler(...))`) so the
    /// `"tool handler error: "` prefix added by `ToolError::Handler::fmt`
    /// does not corrupt the JSON envelope `App.tsx::extractMemoryStoreFields`
    /// relies on to render the highlighted MemoryWriteCard.
    #[tokio::test]
    async fn deny_in_enforce_mode_returns_ok_json_and_does_not_persist() {
        let _guard = TestEnforceGuard::set(PolicyEnforceMode::Enforce);
        let memory = test_memory();
        let entry = entry(memory.clone(), None);
        let huge = "z".repeat(11_000); // > 10_000 byte hard limit triggers Deny.
        let raw = (entry.handler)(
            json!({"key": "deny-huge", "content": huge.clone()}),
            empty_context(),
        )
        .await
        .expect("deny path must return Ok(json_string), NOT Err");

        // Critical: result must parse as JSON without any prefix mangling.
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("deny result must be valid JSON, got {raw:?}: {e}"));
        assert_eq!(parsed["status"], "denied");
        assert_eq!(parsed["policy_decision"], "deny");
        assert_eq!(parsed["reason_code"], "content_too_long");
        assert_eq!(parsed["key"], "deny-huge");

        // Critical: deny MUST NOT persist.
        let recalled = memory
            .recall("deny-huge", None, 10)
            .await
            .expect("recall ok");
        assert!(
            !recalled.iter().any(|e| e.content == huge),
            "deny decisions must not persist; got {recalled:?}",
        );
    }

    /// In `Enforce` mode a `Prompt` decision (>2KB content) should still
    /// behave like the Shadow mode prompt path: not persist, return
    /// `pending_approval` JSON.  Pinned regression so future enforce-mode
    /// tweaks don't accidentally start blocking prompt-class writes.
    #[tokio::test]
    async fn prompt_in_enforce_mode_returns_pending_approval() {
        let _guard = TestEnforceGuard::set(PolicyEnforceMode::Enforce);
        let memory = test_memory();
        let entry = entry(memory.clone(), None);
        let big = "x".repeat(3_000);
        let raw = (entry.handler)(
            json!({"key": "enforce-prompt", "content": big.clone()}),
            empty_context(),
        )
        .await
        .expect("prompt path returns Ok in enforce mode too");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("result is JSON");
        assert_eq!(parsed["status"], "pending_approval");
        assert_eq!(parsed["policy_decision"], "prompt");
        let recalled = memory
            .recall("enforce-prompt", None, 10)
            .await
            .expect("recall ok");
        assert!(!recalled.iter().any(|e| e.content == big));
    }

    #[test]
    fn scope_label_picks_most_specific_binding() {
        assert_eq!(scope_label(&MemoryExecutionScope::global()), "global");

        let project_only = MemoryExecutionScope {
            session_id: None,
            project_id: Some("p".into()),
            workdir: None,
        };
        assert_eq!(scope_label(&project_only), "project");

        let session_scoped = MemoryExecutionScope {
            session_id: Some("s".into()),
            project_id: Some("p".into()),
            workdir: None,
        };
        assert_eq!(scope_label(&session_scoped), "session");
    }

    #[test]
    fn content_preview_truncates_with_ellipsis() {
        assert_eq!(content_preview("hello", 10), "hello");
        assert_eq!(content_preview("hello world", 5), "hello…");
        // Multi-byte safe.
        let cn = "中文一二三四五六七八九十";
        assert_eq!(content_preview(cn, 4), "中文一二…");
    }
}

//! Memory Policy Engine — controls what memory writes are allowed, denied, or require confirmation.
//!
//! The `MemoryPolicyEngine` evaluates write requests against a set of configurable rules
//! and produces a `PolicyDecision` (`allow | deny | prompt`) along with a `reason_code` that
//! can be surfaced in the audit log and the frontend (via MemoryAuditEmitter + MemoryEventBridge).
//!
//! # Shadow Mode
//!
//! In **shadow mode** (the default), all decisions are evaluated and logged, but `deny` decisions
//! are downgraded to `allow` with a `ShadowDenied` reason code. This makes shadow mode safe for
//! gradual rollout: you can observe what *would* have been denied without breaking existing behaviour.
//!
//! Switch to enforce mode via [`MemoryPolicyConfig::enforce_mode`] once audit observations confirm
//! the policy rules are working as intended.

use serde::{Deserialize, Serialize};

use crate::modules::memory::{MemoryCategory, MemoryExecutionScope};

/// The enforcement mode for the policy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PolicyEnforceMode {
    /// Log policy decisions but allow all writes regardless of the decision.
    #[default]
    Shadow,
    /// Block writes that produce a `Deny` decision.
    Enforce,
}

/// The outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    /// The write is allowed.
    Allow,
    /// The write is denied (only active in `Enforce` mode; downgraded in `Shadow`).
    Deny,
    /// The write requires user confirmation before proceeding.
    Prompt,
}

/// Machine-readable reason codes attached to every policy decision.
///
/// Codes are stable strings surfaced in audit events and the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasonCode {
    /// No policy rule matched; write is unconditionally allowed.
    NoRuleMatched,
    /// Content exceeds the maximum allowed length.
    ContentTooLong,
    /// The write was denied in shadow mode; the decision was downgraded to Allow for logging.
    ShadowDenied,
    /// The key already exists and the overwrite policy forbids silent replacement.
    KeyConflict,
    /// The category is on the deny-list for write operations.
    CategoryDenied,
    /// Content length is above the threshold that triggers a user-confirmation prompt.
    LengthPromptThreshold,
    /// M5: a write-time secret/credential pattern was detected by `ThreatScanner`.
    /// The accompanying message carries the matched category (e.g. `api_key`).
    ThreatScannerMatch,
}

impl ReasonCode {
    /// Returns a human-readable label for logging and frontend display.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            ReasonCode::NoRuleMatched => "no_rule_matched",
            ReasonCode::ContentTooLong => "content_too_long",
            ReasonCode::ShadowDenied => "shadow_denied",
            ReasonCode::KeyConflict => "key_conflict",
            ReasonCode::CategoryDenied => "category_denied",
            ReasonCode::LengthPromptThreshold => "length_prompt_threshold",
            ReasonCode::ThreatScannerMatch => "threat_scanner_match",
        }
    }
}

/// The full result of a policy evaluation, including the decision and the reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    /// Whether the write should proceed (`Allow`), be blocked (`Deny`), or require confirmation (`Prompt`).
    pub decision: PolicyDecision,
    /// Machine-readable reason code for logging and frontend display.
    pub reason_code: ReasonCode,
    /// Human-readable explanation for display in the audit log or UI.
    pub message: String,
}

/// Configuration for the `MemoryPolicyEngine`.
#[derive(Debug, Clone)]
pub struct MemoryPolicyConfig {
    /// Enforcement mode (Shadow or Enforce).
    pub enforce_mode: PolicyEnforceMode,
    /// Maximum allowed content length in bytes. Writes exceeding this are denied.
    pub max_content_bytes: usize,
    /// Content length (bytes) above which the engine returns `Prompt` instead of `Allow`.
    ///
    /// Set to 0 to disable prompting.
    pub prompt_threshold_bytes: usize,
    /// Categories that are denied for write operations.
    pub denied_categories: Vec<String>,
}

impl Default for MemoryPolicyConfig {
    fn default() -> Self {
        Self {
            enforce_mode: PolicyEnforceMode::Shadow,
            max_content_bytes: 10_000,
            prompt_threshold_bytes: 2_000,
            denied_categories: vec![],
        }
    }
}

/// Evaluates memory write requests against a set of configurable rules.
///
/// In **shadow mode** (default), `deny` decisions are downgraded to `allow` so existing
/// behaviour is not disrupted while observability is established.
pub struct MemoryPolicyEngine {
    config: MemoryPolicyConfig,
}

impl MemoryPolicyEngine {
    /// Create a new policy engine with the given configuration.
    #[must_use]
    pub fn new(config: MemoryPolicyConfig) -> Self {
        Self { config }
    }

    /// Create a policy engine with default configuration (shadow mode).
    ///
    /// Retained for tests and library consumers that don't yet thread the
    /// `MemoryFeatureConfig`-derived enforce mode through their call sites.
    /// Production callers should use [`MemoryPolicyEngine::new`] with an
    /// explicit [`MemoryPolicyConfig`].
    #[must_use]
    #[allow(dead_code)]
    pub fn default_shadow() -> Self {
        Self::new(MemoryPolicyConfig::default())
    }

    /// Evaluate a write request and return a `PolicyResult`.
    ///
    /// Rules are evaluated in priority order:
    /// 1. Content too long → `Deny` (hard limit)
    /// 2. Category denied → `Deny`
    /// 3. Content above prompt threshold → `Prompt`
    /// 4. No rule matched → `Allow`
    ///
    /// In shadow mode, `Deny` results are downgraded to `Allow` with `ShadowDenied` reason code.
    #[must_use]
    pub fn evaluate_write(
        &self,
        _key: &str,
        content: &str,
        category: &MemoryCategory,
        _scope: &MemoryExecutionScope,
    ) -> PolicyResult {
        // Rule 1: hard content length limit.
        if content.len() > self.config.max_content_bytes {
            return self.apply_mode(PolicyResult {
                decision: PolicyDecision::Deny,
                reason_code: ReasonCode::ContentTooLong,
                message: format!(
                    "content length {} exceeds maximum {} bytes",
                    content.len(),
                    self.config.max_content_bytes
                ),
            });
        }

        // Rule 2: category deny-list.
        if self
            .config
            .denied_categories
            .iter()
            .any(|dc| dc == category.as_str())
        {
            return self.apply_mode(PolicyResult {
                decision: PolicyDecision::Deny,
                reason_code: ReasonCode::CategoryDenied,
                message: format!("category '{}' is on the deny list", category.as_str()),
            });
        }

        // Rule 3: content length prompt threshold (only when prompting is enabled).
        if self.config.prompt_threshold_bytes > 0
            && content.len() > self.config.prompt_threshold_bytes
        {
            return PolicyResult {
                decision: PolicyDecision::Prompt,
                reason_code: ReasonCode::LengthPromptThreshold,
                message: format!(
                    "content length {} exceeds prompt threshold {} bytes — confirm write",
                    content.len(),
                    self.config.prompt_threshold_bytes
                ),
            };
        }

        // Default: allow.
        PolicyResult {
            decision: PolicyDecision::Allow,
            reason_code: ReasonCode::NoRuleMatched,
            message: "write allowed by policy".to_string(),
        }
    }

    /// In shadow mode, downgrade `Deny` to `Allow` and attach `ShadowDenied` reason code.
    fn apply_mode(&self, result: PolicyResult) -> PolicyResult {
        if result.decision == PolicyDecision::Deny
            && self.config.enforce_mode == PolicyEnforceMode::Shadow
        {
            return PolicyResult {
                decision: PolicyDecision::Allow,
                reason_code: ReasonCode::ShadowDenied,
                message: format!(
                    "[shadow] would have denied: {} — {}",
                    result.reason_code.label(),
                    result.message
                ),
            };
        }
        result
    }

    /// Return a reference to the current configuration.
    #[must_use]
    #[allow(dead_code)] // consumed by MemoryAuditEmitter (fix-mcp-audit) and feature flag config (fix-feature-flags)
    pub fn config(&self) -> &MemoryPolicyConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::scope::MemoryScopeResolver;

    fn global_scope() -> MemoryExecutionScope {
        MemoryScopeResolver::resolve(None, None, None)
    }

    #[test]
    fn short_content_is_allowed() {
        let engine = MemoryPolicyEngine::default_shadow();
        let result = engine.evaluate_write(
            "key",
            "short content",
            &MemoryCategory::Core,
            &global_scope(),
        );
        assert_eq!(result.decision, PolicyDecision::Allow);
        assert_eq!(result.reason_code, ReasonCode::NoRuleMatched);
    }

    #[test]
    fn content_exceeding_max_is_shadow_denied_in_shadow_mode() {
        let engine = MemoryPolicyEngine::default_shadow();
        let long_content = "x".repeat(11_000);
        let result =
            engine.evaluate_write("key", &long_content, &MemoryCategory::Core, &global_scope());
        // In shadow mode, deny is downgraded to allow with ShadowDenied.
        assert_eq!(result.decision, PolicyDecision::Allow);
        assert_eq!(result.reason_code, ReasonCode::ShadowDenied);
    }

    #[test]
    fn content_exceeding_max_is_denied_in_enforce_mode() {
        let config = MemoryPolicyConfig {
            enforce_mode: PolicyEnforceMode::Enforce,
            ..Default::default()
        };
        let engine = MemoryPolicyEngine::new(config);
        let long_content = "x".repeat(11_000);
        let result =
            engine.evaluate_write("key", &long_content, &MemoryCategory::Core, &global_scope());
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.reason_code, ReasonCode::ContentTooLong);
    }

    #[test]
    fn denied_category_is_shadow_denied_in_shadow_mode() {
        let config = MemoryPolicyConfig {
            denied_categories: vec!["daily".to_string()],
            ..Default::default()
        };
        let engine = MemoryPolicyEngine::new(config);
        let result = engine.evaluate_write("key", "hello", &MemoryCategory::Daily, &global_scope());
        assert_eq!(result.decision, PolicyDecision::Allow);
        assert_eq!(result.reason_code, ReasonCode::ShadowDenied);
    }

    #[test]
    fn denied_category_is_denied_in_enforce_mode() {
        let config = MemoryPolicyConfig {
            enforce_mode: PolicyEnforceMode::Enforce,
            denied_categories: vec!["daily".to_string()],
            ..Default::default()
        };
        let engine = MemoryPolicyEngine::new(config);
        let result = engine.evaluate_write("key", "hello", &MemoryCategory::Daily, &global_scope());
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.reason_code, ReasonCode::CategoryDenied);
    }

    #[test]
    fn content_above_prompt_threshold_returns_prompt() {
        let engine = MemoryPolicyEngine::default_shadow();
        // Content between prompt threshold (2000) and max (10000) triggers Prompt.
        let medium_content = "x".repeat(3_000);
        let result = engine.evaluate_write(
            "key",
            &medium_content,
            &MemoryCategory::Core,
            &global_scope(),
        );
        assert_eq!(result.decision, PolicyDecision::Prompt);
        assert_eq!(result.reason_code, ReasonCode::LengthPromptThreshold);
    }

    #[test]
    fn reason_code_labels_are_stable_strings() {
        assert_eq!(ReasonCode::NoRuleMatched.label(), "no_rule_matched");
        assert_eq!(ReasonCode::ContentTooLong.label(), "content_too_long");
        assert_eq!(ReasonCode::ShadowDenied.label(), "shadow_denied");
        assert_eq!(ReasonCode::KeyConflict.label(), "key_conflict");
        assert_eq!(ReasonCode::CategoryDenied.label(), "category_denied");
        assert_eq!(
            ReasonCode::LengthPromptThreshold.label(),
            "length_prompt_threshold"
        );
    }
}

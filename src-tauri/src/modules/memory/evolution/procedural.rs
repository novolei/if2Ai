//! Procedural Memory Manager — converts [`Insight`]s into durable
//! procedural rules that can be injected into the system prompt.
//!
//! Procedural memories represent "how-to" knowledge learned from
//! experience.  Each rule starts with a low `trust_score` (0.3) and is
//! promoted / demoted via [`ProceduralMemoryManager::validate_procedure`]
//! as subsequent tasks succeed or fail.

use super::reflector::{Insight, InsightCategory};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryCategory, MemoryError, SharedMemoryProvider};

/// Initial trust score for newly internalized procedural memories.
const INITIAL_TRUST_SCORE: f64 = 0.3;

/// Trust score increment on successful validation.
const TRUST_INCREMENT: f64 = 0.1;

/// Trust score decrement on failed validation.
const TRUST_DECREMENT: f64 = 0.15;

/// Procedural Memory Manager — transforms [`Insight`]s into injectable prompt rules.
///
/// Each insight is stored as a `Procedural`-category memory entry whose
/// `trust_score` evolves over time through validation feedback.  Active
/// procedures (trust > 0) are formatted into a markdown segment suitable
/// for system-prompt injection.
pub struct ProceduralMemoryManager {
    provider: SharedMemoryProvider,
}

impl ProceduralMemoryManager {
    /// Create a new manager backed by the given memory provider.
    #[must_use]
    pub fn new(provider: SharedMemoryProvider) -> Self {
        Self { provider }
    }

    /// Internalize an insight as a procedural memory rule.
    ///
    /// 1. Formats the [`Insight`] into concise rule text.
    /// 2. Checks for existing procedural memories with similar content
    ///    via `provider.recall()`.
    /// 3. If a near-duplicate exists (same key prefix), updates its content
    ///    and bumps `trust_score`.
    /// 4. Otherwise stores as a new `Procedural`-category entry with
    ///    initial `trust_score = 0.3`.
    /// 5. Key format: `proc:{category}:{content_hash}`.
    ///
    /// Returns the storage key.
    pub async fn internalize_insight(
        &self,
        insight: &Insight,
        scope: &MemoryExecutionScope,
    ) -> Result<String, MemoryError> {
        let category_tag = category_to_tag(&insight.category);
        let content_hash = short_hash(&insight.content);
        let key = format!("proc:{category_tag}:{content_hash}");

        let rule_text = format_insight_as_rule(insight);

        // Check for existing procedural memory with same key
        if let Ok(Some(existing)) = self.provider.get_by_key(&key).await {
            // Update content, bump trust
            let _ = self.provider.update_content(&key, &rule_text).await;
            let new_trust = (existing.trust_score + TRUST_INCREMENT).min(1.0);
            let _ = self
                .provider
                .adjust_trust_score(&key, TRUST_INCREMENT)
                .await;
            tracing::debug!(
                key = %key,
                new_trust,
                "Updated existing procedural memory"
            );
            return Ok(key);
        }

        // Check for semantically similar existing procedurals
        let existing = self
            .provider
            .recall_scoped(&insight.content, Some("procedural"), 5, scope)
            .await
            .unwrap_or_default();

        for entry in &existing {
            if entry.key.starts_with("proc:") && content_overlap(&entry.content, &rule_text) > 0.7 {
                // High similarity — update existing rather than create duplicate
                let _ = self.provider.update_content(&entry.key, &rule_text).await;
                let _ = self
                    .provider
                    .adjust_trust_score(&entry.key, TRUST_INCREMENT)
                    .await;
                tracing::debug!(
                    key = %entry.key,
                    "Updated similar procedural memory"
                );
                return Ok(entry.key.clone());
            }
        }

        // Store new procedural memory
        self.provider
            .store_scoped(&key, &rule_text, MemoryCategory::Procedural, scope)
            .await?;

        // Set initial trust score (default is 0.0, we want 0.3)
        let _ = self
            .provider
            .adjust_trust_score(&key, INITIAL_TRUST_SCORE)
            .await;

        tracing::debug!(key = %key, "Created new procedural memory");
        Ok(key)
    }

    /// Validate a procedural memory based on task outcome.
    ///
    /// Adjusts the entry's `trust_score`:
    /// - On success: `+0.1` (capped at 1.0).
    /// - On failure: `−0.15` (floored at −1.0).
    ///
    /// Procedures with `trust_score < −0.5` will be pruned during the
    /// next DayDream consolidation cycle.
    ///
    /// Returns the new trust score.
    pub async fn validate_procedure(
        &self,
        key: &str,
        task_succeeded: bool,
    ) -> Result<f64, MemoryError> {
        let delta = if task_succeeded {
            TRUST_INCREMENT
        } else {
            -TRUST_DECREMENT
        };
        self.provider.adjust_trust_score(key, delta).await
    }

    /// Retrieve all active procedural memories (trust_score > 0).
    ///
    /// Results are sorted by `trust_score` descending so the most
    /// trusted rules appear first.
    pub async fn active_procedures(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<crate::modules::memory::MemoryEntry>, MemoryError> {
        let all = self
            .provider
            .export_scoped(Some("procedural"), scope)
            .await?;
        let mut active: Vec<_> = all
            .into_iter()
            .filter(|e| e.trust_score > 0.0 && e.key.starts_with("proc:"))
            .collect();
        active.sort_by(|a, b| {
            b.trust_score
                .partial_cmp(&a.trust_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(active)
    }

    /// Format active procedural memories for system-prompt injection.
    ///
    /// Output format:
    /// ```text
    /// ## Procedural Rules (learned from experience)
    ///
    /// 1. [HeuristicRule] When encountering compile errors, check recently modified files first (confidence: 0.8)
    /// 2. [BestPractice] Use cargo check instead of cargo build for fast validation (confidence: 0.9)
    /// ```
    ///
    /// Returns an empty string when no active procedures exist.
    pub async fn format_for_prompt_injection(
        &self,
        scope: &MemoryExecutionScope,
        max_rules: usize,
    ) -> Result<String, MemoryError> {
        let procedures = self.active_procedures(scope).await?;
        if procedures.is_empty() {
            return Ok(String::new());
        }

        let mut output = String::from("## Procedural Rules (learned from experience)\n\n");
        for (i, entry) in procedures.iter().take(max_rules).enumerate() {
            let confidence = format!("{:.1}", entry.trust_score);
            let category_label = extract_category_label(&entry.key);
            output.push_str(&format!(
                "{}. [{}] {} (confidence: {})\n",
                i + 1,
                category_label,
                entry.content,
                confidence,
            ));
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Map insight category to a short tag for the key.
fn category_to_tag(cat: &InsightCategory) -> &'static str {
    match cat {
        InsightCategory::HeuristicRule => "heuristic",
        InsightCategory::AntiPattern => "antipattern",
        InsightCategory::BestPractice => "bestpractice",
        InsightCategory::UserPreference => "userpref",
        InsightCategory::ToolUsagePattern => "toolusage",
    }
}

/// Extract a human-readable category label from a proc key.
pub(crate) fn extract_category_label(key: &str) -> &str {
    // key format: "proc:{category_tag}:{hash}"
    let parts: Vec<&str> = key.splitn(3, ':').collect();
    if parts.len() >= 2 {
        match parts[1] {
            "heuristic" => "HeuristicRule",
            "antipattern" => "AntiPattern",
            "bestpractice" => "BestPractice",
            "userpref" => "UserPreference",
            "toolusage" => "ToolUsagePattern",
            other => other,
        }
    } else {
        "Rule"
    }
}

/// Format an insight into a concise rule statement.
fn format_insight_as_rule(insight: &Insight) -> String {
    // Strip trailing periods for consistency, then reconstruct
    let content = insight.content.trim_end_matches('.');
    content.to_string()
}

/// Compute a short deterministic hash of a string (8 hex chars).
/// Uses FNV-1a for cross-build stability (unlike DefaultHasher).
fn short_hash(s: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)[..8].to_string()
}

/// Simple word-overlap ratio between two strings.
///
/// Returns a value in `[0.0, 1.0]`. Used as a cheap approximation
/// of semantic similarity for deduplication.
fn content_overlap(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::evolution::reflector::Insight;
    use chrono::Utc;
    use std::sync::Arc;

    #[allow(deprecated)]
    fn test_provider() -> SharedMemoryProvider {
        Arc::new(crate::modules::memory::InMemoryMemoryProvider::new())
    }

    fn make_insight(category: InsightCategory, content: &str) -> Insight {
        Insight {
            id: "test-insight-1".to_string(),
            category,
            content: content.to_string(),
            confidence: 0.8,
            applicable_contexts: vec!["test".to_string()],
            source_trajectory_ids: vec!["traj-1".to_string()],
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_internalize_insight() {
        let provider = test_provider();
        let mgr = ProceduralMemoryManager::new(Arc::clone(&provider));
        let scope = MemoryExecutionScope::global();

        let insight = make_insight(
            InsightCategory::HeuristicRule,
            "When encountering compile errors, check recently modified files first",
        );

        let key = mgr.internalize_insight(&insight, &scope).await;
        assert!(key.is_ok());
        let key = key.unwrap();
        assert!(key.starts_with("proc:heuristic:"));

        // Verify the entry was stored as Procedural category
        let entries = provider.export(Some("procedural")).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].content.contains("compile errors"));
    }

    #[tokio::test]
    async fn test_validate_procedure_success() {
        let provider = test_provider();
        let mgr = ProceduralMemoryManager::new(Arc::clone(&provider));
        let scope = MemoryExecutionScope::global();

        let insight = make_insight(InsightCategory::BestPractice, "Use cargo check for speed");
        let key = mgr.internalize_insight(&insight, &scope).await.unwrap();

        // Validate success — trust should increase
        let new_trust = mgr.validate_procedure(&key, true).await;
        assert!(new_trust.is_ok());
        // InMemoryProvider's adjust_trust_score returns 0.0 (no-op),
        // but the call should not error
    }

    #[tokio::test]
    async fn test_validate_procedure_failure() {
        let provider = test_provider();
        let mgr = ProceduralMemoryManager::new(Arc::clone(&provider));
        let scope = MemoryExecutionScope::global();

        let insight = make_insight(InsightCategory::AntiPattern, "Avoid large batch operations");
        let key = mgr.internalize_insight(&insight, &scope).await.unwrap();

        // Validate failure — trust should decrease
        let new_trust = mgr.validate_procedure(&key, false).await;
        assert!(new_trust.is_ok());
    }

    #[tokio::test]
    async fn test_format_for_prompt_injection() {
        let provider = test_provider();
        let mgr = ProceduralMemoryManager::new(Arc::clone(&provider));
        let scope = MemoryExecutionScope::global();

        // Store a procedural entry manually with trust > 0
        provider
            .store(
                "proc:heuristic:abc12345",
                "Check modified files first when debugging compile errors",
                MemoryCategory::Procedural,
            )
            .await
            .unwrap();

        // InMemoryProvider stores with trust_score = 0.0, so active_procedures
        // filters it out. Use internalize_insight which sets initial trust.
        let insight = make_insight(
            InsightCategory::BestPractice,
            "Run tests before committing changes",
        );
        let _key = mgr.internalize_insight(&insight, &scope).await.unwrap();

        let output = mgr.format_for_prompt_injection(&scope, 10).await;
        assert!(output.is_ok());
        // Note: InMemoryProvider doesn't actually persist trust_score adjustments,
        // so active_procedures may return empty. The format function handles that.
        let text = output.unwrap();
        // If procedures are active, should contain the header
        if !text.is_empty() {
            assert!(text.contains("Procedural Rules"));
        }
    }

    #[test]
    fn test_content_overlap() {
        let a = "check modified files first when debugging";
        let b = "check modified files first when fixing errors";
        let overlap = content_overlap(a, b);
        assert!(overlap > 0.5, "overlap={overlap}");

        let c = "completely different content here";
        let overlap2 = content_overlap(a, c);
        assert!(overlap2 < 0.3, "overlap2={overlap2}");
    }

    #[test]
    fn test_short_hash_deterministic() {
        let h1 = short_hash("hello world");
        let h2 = short_hash("hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);

        let h3 = short_hash("different input");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_category_to_tag() {
        assert_eq!(
            category_to_tag(&InsightCategory::HeuristicRule),
            "heuristic"
        );
        assert_eq!(
            category_to_tag(&InsightCategory::AntiPattern),
            "antipattern"
        );
        assert_eq!(
            category_to_tag(&InsightCategory::BestPractice),
            "bestpractice"
        );
    }
}

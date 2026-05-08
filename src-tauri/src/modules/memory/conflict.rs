//! Memory conflict detection and resolution.
//!
//! Detects semantic conflicts between a new memory entry and existing entries,
//! then applies automatic resolution strategies based on quality scores and
//! conflict type.
//!
//! The detector is inserted into the `memory_store` pipeline **after** the
//! Mem0-style decision tree and **before** final persistence, so it only
//! fires on writes that the decision tree has already judged as `Add`.

use crate::modules::memory::{MemoryEntry, MemoryError, MemoryProvider};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Conflict detection configuration.
///
/// Controls similarity thresholds, candidate limits, and the quality gap
/// required for automatic resolution without user confirmation.
pub struct ConflictConfig {
    /// Semantic similarity threshold above which two entries are considered
    /// potentially conflicting.  Default `0.75`.
    pub similarity_threshold: f64,
    /// Maximum number of candidate entries to retrieve from the provider
    /// for comparison.  Default `10`.
    pub max_candidates: usize,
    /// Minimum quality-score gap required to auto-resolve a contradiction
    /// in favour of the higher-quality entry.  Default `0.3`.
    pub auto_resolve_threshold: f64,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.75,
            max_candidates: 10,
            // Per spec 2026-05-08-a1-pr3-pr4-forgetting-conflict-design.md §2.2:
            // aggressive auto-resolve while AskUser UI is absent. Silent
            // "keep both" of low-gap conflicts is worse than occasional
            // wrong auto-pick (loser was already similar, access
            // reinforcement re-promotes valid entries). Revisit when
            // AskUser UI ships.
            auto_resolve_threshold: 0.1,
        }
    }
}

// ---------------------------------------------------------------------------
// Conflict types
// ---------------------------------------------------------------------------

/// Describes a single detected conflict between a new memory and an existing
/// entry.
pub struct Conflict {
    /// Key of the new memory being stored.
    pub new_key: String,
    /// Content of the new memory being stored.
    pub new_content: String,
    /// The existing entry that conflicts with the new memory.
    pub existing_entry: MemoryEntry,
    /// Normalised similarity score in `[0.0, 1.0]` between the two entries.
    pub similarity_score: f64,
    /// Classification of the conflict.
    pub conflict_type: ConflictType,
}

/// Classification of how two memory entries conflict.
pub enum ConflictType {
    /// Semantic contradiction — same topic but different (incompatible)
    /// information.
    Contradiction,
    /// Partial overlap — some information is shared, some differs.
    PartialOverlap,
    /// Near-duplicate — content is almost identical.
    Duplicate,
}

// ---------------------------------------------------------------------------
// Resolution strategies
// ---------------------------------------------------------------------------

/// Strategy chosen to resolve a detected conflict.
#[derive(Debug)]
pub enum ConflictResolution {
    /// Keep the newer entry and delete the older one.
    KeepNewer,
    /// Keep the existing entry (it has higher quality); the caller should
    /// skip persisting the new entry.
    KeepExisting,
    /// Keep the new entry and delete the existing one (new has higher quality).
    KeepNew,
    /// Merge both entries into a single combined memory.
    Merge {
        /// The merged content to store in place of both entries.
        merged_content: String,
    },
    /// The conflict cannot be resolved automatically — surface it to the
    /// user for manual resolution.
    AskUser {
        /// Human-readable summary of the conflict.
        conflict_summary: String,
    },
    /// Keep both entries but increment `contradiction_count` on each so
    /// the quality scorer can down-weight them.
    KeepBothWithFlag,
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Summary report produced after conflict detection and resolution.
#[allow(dead_code)]
pub struct ConflictReport {
    /// Total number of conflicts found.
    pub conflicts_found: usize,
    /// Number of conflicts that were auto-resolved.
    pub auto_resolved: usize,
    /// Number of conflicts awaiting user confirmation.
    pub pending_user: usize,
    /// Per-conflict resolution details.
    pub resolutions: Vec<(Conflict, ConflictResolution)>,
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

/// Stateless conflict detector that compares a new memory entry against
/// existing entries retrieved from a [`MemoryProvider`].
pub struct ConflictDetector {
    config: ConflictConfig,
}

impl Default for ConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictDetector {
    /// Create a detector with default configuration.
    pub fn new() -> Self {
        Self {
            config: ConflictConfig::default(),
        }
    }

    /// Create a detector with a custom configuration.
    pub fn with_config(config: ConflictConfig) -> Self {
        Self { config }
    }

    /// Detect conflicts between a new memory and existing entries.
    ///
    /// 1. Uses `provider.recall()` to find semantically similar candidates.
    /// 2. For each candidate above `similarity_threshold`, classifies the
    ///    conflict type.
    /// 3. Returns all detected conflicts (may be empty).
    pub async fn detect_conflicts(
        &self,
        new_key: &str,
        new_content: &str,
        provider: &dyn MemoryProvider,
    ) -> Result<Vec<Conflict>, MemoryError> {
        let candidates = provider
            .recall(new_content, None, self.config.max_candidates)
            .await?;

        let mut conflicts = Vec::new();

        for candidate in candidates {
            // Skip self (same key).
            if candidate.key == new_key {
                continue;
            }

            let similarity = text_similarity(new_content, &candidate.content);

            if similarity < self.config.similarity_threshold {
                continue;
            }

            let conflict_type = classify_conflict(similarity, new_content, &candidate.content);

            conflicts.push(Conflict {
                new_key: new_key.to_string(),
                new_content: new_content.to_string(),
                existing_entry: candidate,
                similarity_score: similarity,
                conflict_type,
            });
        }

        Ok(conflicts)
    }

    /// Choose a resolution strategy for a single conflict.
    ///
    /// Rules:
    /// - **Duplicate** → `KeepNewer`
    /// - **Contradiction** with quality gap > `auto_resolve_threshold` →
    ///   `KeepExisting` or `KeepNew`
    /// - **Contradiction** with quality gap ≤ `auto_resolve_threshold` →
    ///   `KeepBothWithFlag`
    /// - **PartialOverlap** → `KeepBothWithFlag`
    pub fn resolve(&self, conflict: &Conflict, new_quality: f64) -> ConflictResolution {
        match conflict.conflict_type {
            ConflictType::Duplicate => ConflictResolution::KeepNewer,

            ConflictType::Contradiction => {
                let quality_gap = (new_quality - conflict.existing_entry.quality_score).abs();
                if quality_gap > self.config.auto_resolve_threshold {
                    if new_quality > conflict.existing_entry.quality_score {
                        ConflictResolution::KeepNew
                    } else {
                        ConflictResolution::KeepExisting
                    }
                } else {
                    ConflictResolution::KeepBothWithFlag
                }
            }

            ConflictType::PartialOverlap => ConflictResolution::KeepBothWithFlag,
        }
    }

    /// Apply a chosen resolution to the provider.
    ///
    /// - `KeepNewer` — delete the existing entry.
    /// - `KeepExisting` — no-op on the provider side; the caller must skip
    ///   persisting the new entry.
    /// - `KeepNew` — delete the existing entry.
    /// - `Merge` — delete the existing entry (the caller stores the merged
    ///   content as the new entry).
    /// - `KeepBothWithFlag` — bump `contradiction_count` on the existing
    ///   entry by updating its content (content unchanged, side-effect only
    ///   possible through the trait surface).
    /// - `AskUser` — no-op; the caller surfaces the conflict to the user.
    pub async fn apply_resolution(
        &self,
        conflict: &Conflict,
        resolution: &ConflictResolution,
        provider: &dyn MemoryProvider,
    ) -> Result<(), MemoryError> {
        match resolution {
            ConflictResolution::KeepNewer => {
                // Delete the existing (older) entry.
                match provider.delete(&conflict.existing_entry.key).await {
                    Ok(()) => {}
                    // Tolerate already-deleted entries.
                    Err(MemoryError::KeyNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }

            ConflictResolution::KeepExisting => {
                // The existing entry has higher quality — do nothing here.
                // The caller is responsible for skipping persistence of the
                // new entry.
            }

            ConflictResolution::KeepNew => {
                // The new entry has higher quality — delete the existing one.
                match provider.delete(&conflict.existing_entry.key).await {
                    Ok(()) => {}
                    Err(MemoryError::KeyNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }

            ConflictResolution::Merge { .. } => {
                // Delete the existing entry; the caller stores the merged content.
                match provider.delete(&conflict.existing_entry.key).await {
                    Ok(()) => {}
                    Err(MemoryError::KeyNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }

            ConflictResolution::KeepBothWithFlag => {
                // Spec §2.3 Hybrid C — bump contradiction_count on the
                // EXISTING entry so future conflict-review UI can
                // surface "your existing memory has been challenged".
                // Best-effort: log on error but don't fail the write.
                if let Err(err) = provider
                    .increment_contradiction_count(&conflict.existing_entry.key)
                    .await
                {
                    tracing::warn!(
                        target: "memory.conflict",
                        existing_key = %conflict.existing_entry.key,
                        new_key = %conflict.new_key,
                        error = %err,
                        "failed to bump contradiction_count; logging only",
                    );
                } else {
                    tracing::info!(
                        target: "memory.conflict",
                        existing_key = %conflict.existing_entry.key,
                        new_key = %conflict.new_key,
                        similarity = conflict.similarity_score,
                        "conflict detected — both entries kept; contradiction_count bumped on existing",
                    );
                }
            }

            ConflictResolution::AskUser { conflict_summary } => {
                tracing::info!(
                    target: "memory.conflict",
                    summary = %conflict_summary,
                    "conflict requires user resolution",
                );
                // No storage mutation — caller surfaces this to the user.
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute a simple normalised similarity score between two strings using
/// bigram overlap (Sørensen–Dice coefficient).  This is a lightweight
/// heuristic — the real semantic similarity comes from the vector provider's
/// recall ranking, but we need a secondary score to classify conflict type.
fn text_similarity(a: &str, b: &str) -> f64 {
    let bigrams_a = char_bigrams(a);
    let bigrams_b = char_bigrams(b);

    if bigrams_a.is_empty() && bigrams_b.is_empty() {
        return 1.0; // both empty → identical
    }
    if bigrams_a.is_empty() || bigrams_b.is_empty() {
        return 0.0;
    }

    let intersection = bigrams_a.iter().filter(|bg| bigrams_b.contains(bg)).count();

    (2.0 * intersection as f64) / (bigrams_a.len() + bigrams_b.len()) as f64
}

/// Extract character bigrams from a lowercased string.
fn char_bigrams(s: &str) -> Vec<(char, char)> {
    let lower: Vec<char> = s.to_lowercase().chars().collect();
    lower.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Classify a conflict based on similarity score and content comparison.
fn classify_conflict(similarity: f64, _new_content: &str, _existing_content: &str) -> ConflictType {
    if similarity > 0.95 {
        ConflictType::Duplicate
    } else if similarity > 0.85 {
        ConflictType::PartialOverlap
    } else {
        // similarity is between threshold (0.75) and 0.85 — likely a
        // contradiction (same topic, different information).
        ConflictType::Contradiction
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::{InMemoryMemoryProvider, MemoryCategory};
    use std::sync::Arc;

    #[allow(deprecated)]
    fn make_provider() -> Arc<InMemoryMemoryProvider> {
        Arc::new(InMemoryMemoryProvider::new())
    }

    fn make_entry_with(key: &str, content: &str, quality: f64) -> MemoryEntry {
        let now = chrono::Utc::now();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
            quality_score: quality,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: 4,
            context_tags: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_detect_no_conflicts() {
        let provider = make_provider();
        // Store an entry that is completely unrelated.
        provider
            .store("weather", "It is sunny today", MemoryCategory::Daily)
            .await
            .expect("store ok");

        let detector = ConflictDetector::new();
        let conflicts = detector
            .detect_conflicts("food", "I love sushi", &*provider)
            .await
            .expect("detect ok");

        assert!(
            conflicts.is_empty(),
            "unrelated entries should produce no conflicts"
        );
    }

    #[test]
    fn test_resolve_duplicate() {
        let detector = ConflictDetector::new();
        let existing = make_entry_with("k1", "the sky is blue", 0.5);
        let conflict = Conflict {
            new_key: "k2".to_string(),
            new_content: "the sky is blue".to_string(),
            existing_entry: existing,
            similarity_score: 0.98,
            conflict_type: ConflictType::Duplicate,
        };

        let resolution = detector.resolve(&conflict, 0.6);
        assert!(
            matches!(resolution, ConflictResolution::KeepNewer),
            "duplicate should resolve to KeepNewer"
        );
    }

    #[test]
    fn test_resolve_contradiction_high_quality_gap() {
        let detector = ConflictDetector::new();
        let existing = make_entry_with("k1", "Rust is slow", 0.3);
        let conflict = Conflict {
            new_key: "k2".to_string(),
            new_content: "Rust is fast".to_string(),
            existing_entry: existing,
            similarity_score: 0.80,
            conflict_type: ConflictType::Contradiction,
        };

        // new_quality=0.8, existing=0.3 → gap=0.5 > 0.3 threshold
        let resolution = detector.resolve(&conflict, 0.8);
        assert!(
            matches!(resolution, ConflictResolution::KeepNew),
            "new entry has higher quality, should resolve to KeepNew"
        );
    }

    #[test]
    fn test_resolve_contradiction_small_gap() {
        let detector = ConflictDetector::new();
        let existing = make_entry_with("k1", "Rust is slow", 0.5);
        let conflict = Conflict {
            new_key: "k2".to_string(),
            new_content: "Rust is fast".to_string(),
            existing_entry: existing,
            similarity_score: 0.80,
            conflict_type: ConflictType::Contradiction,
        };

        // new_quality=0.6, existing=0.5 → gap=0.1 ≤ 0.3 threshold
        let resolution = detector.resolve(&conflict, 0.6);
        assert!(
            matches!(resolution, ConflictResolution::KeepBothWithFlag),
            "small quality gap should resolve to KeepBothWithFlag"
        );
    }

    #[test]
    fn text_similarity_identical_strings() {
        let score = text_similarity("hello world", "hello world");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn text_similarity_completely_different() {
        let score = text_similarity("abc", "xyz");
        assert!(score < 0.1, "score={score}");
    }

    #[test]
    fn classify_high_similarity_as_duplicate() {
        let ct = classify_conflict(0.97, "a", "b");
        assert!(matches!(ct, ConflictType::Duplicate));
    }

    #[test]
    fn classify_medium_similarity_as_partial_overlap() {
        let ct = classify_conflict(0.90, "a", "b");
        assert!(matches!(ct, ConflictType::PartialOverlap));
    }

    #[test]
    fn classify_lower_similarity_as_contradiction() {
        let ct = classify_conflict(0.78, "a", "b");
        assert!(matches!(ct, ConflictType::Contradiction));
    }

    #[tokio::test]
    async fn keep_both_increments_contradiction_count_via_sqlite_override() {
        // Spec §5.2: SqliteMemoryProvider's override actually bumps the
        // contradiction_count column. Best-effort: missing key returns
        // Ok(()) per the trait contract.
        use crate::modules::memory::providers::SqliteMemoryProvider;
        use crate::modules::memory::{MemoryCategory, MemoryProvider};
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("test_memory.db");
        let provider = SqliteMemoryProvider::new(db_path).expect("provider init");

        let key_a = "fact-shanghai";
        provider
            .store(key_a, "User lives in Shanghai", MemoryCategory::Core)
            .await
            .expect("store a");

        // Direct override call -- verifies the SQL UPDATE works.
        provider
            .increment_contradiction_count(key_a)
            .await
            .expect("increment ok");
        provider
            .increment_contradiction_count(key_a)
            .await
            .expect("increment ok 2nd time");

        // Read back via export and confirm the column was bumped twice.
        let entries = provider.export(None).await.expect("export ok");
        let entry = entries
            .iter()
            .find(|e| e.key == key_a)
            .expect("entry present");
        assert_eq!(
            entry.contradiction_count, 2,
            "expected 2 increments to land on contradiction_count, got {}",
            entry.contradiction_count
        );

        // Unknown key should not error.
        provider
            .increment_contradiction_count("does-not-exist")
            .await
            .expect("missing-key should be Ok per spec contract");
    }

    #[tokio::test]
    async fn resolver_keep_both_with_flag_bumps_contradiction_count() {
        // Spec §5.2: drive the full ConflictResolver path against a real
        // SqliteMemoryProvider. Store A; classify a conflict that produces
        // KeepBothWithFlag; assert the column on A is incremented.
        use crate::modules::memory::providers::SqliteMemoryProvider;
        use crate::modules::memory::{MemoryCategory, MemoryProvider};
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("test_memory.db");
        let provider = SqliteMemoryProvider::new(db_path).expect("provider init");

        let key_a = "fact-shanghai";
        provider
            .store(key_a, "User lives in Shanghai", MemoryCategory::Core)
            .await
            .expect("store a");

        let existing_entry = provider
            .export(None)
            .await
            .expect("export")
            .into_iter()
            .find(|e| e.key == key_a)
            .expect("entry present");
        // Force the existing entry's quality_score so the gap is < 0.1.
        let mut existing_for_test = existing_entry.clone();
        existing_for_test.quality_score = 0.50;

        let conflict = Conflict {
            new_key: "fact-tokyo".to_string(),
            new_content: "User lives in Tokyo".to_string(),
            existing_entry: existing_for_test,
            conflict_type: ConflictType::Contradiction,
            similarity_score: 0.78,
        };

        let resolver = ConflictDetector::with_config(ConflictConfig::default());
        let resolution = resolver.resolve(&conflict, 0.55);
        assert!(
            matches!(resolution, ConflictResolution::KeepBothWithFlag),
            "expected KeepBothWithFlag for gap < 0.1 Contradiction, got {resolution:?}"
        );
        resolver
            .apply_resolution(&conflict, &resolution, &provider)
            .await
            .expect("apply ok");

        let after = provider
            .export(None)
            .await
            .expect("export")
            .into_iter()
            .find(|e| e.key == key_a)
            .expect("entry present after");
        assert_eq!(
            after.contradiction_count, 1,
            "KeepBothWithFlag should bump contradiction_count by 1, got {}",
            after.contradiction_count
        );
    }

    #[test]
    fn conflict_default_threshold_is_01() {
        let cfg = ConflictConfig::default();
        // Spec 2026-05-08-a1-pr3-pr4-forgetting-conflict-design.md §2.2:
        // aggressive auto-resolve while AskUser UI is absent. Regression
        // guard against silent re-tuning.
        assert!(
            (cfg.auto_resolve_threshold - 0.1).abs() < f64::EPSILON,
            "default auto_resolve_threshold should be 0.1 per spec, got {}",
            cfg.auto_resolve_threshold
        );
        assert!(
            (cfg.similarity_threshold - 0.75).abs() < f64::EPSILON,
            "default similarity_threshold should be 0.75, got {}",
            cfg.similarity_threshold
        );
        assert_eq!(cfg.max_candidates, 10);
    }
}

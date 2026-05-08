//! Merge step — finds near-duplicate memory entries by text similarity and
//! consolidates them via an LLM fan-in call.

use chrono::Utc;

use crate::modules::memory::conflict::text_similarity;
use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::quality::QualityScorer;
use crate::modules::memory::{MemoryEntry, SharedMemoryProvider};

use super::report::{StepError, StepOutcome};

// ---------------------------------------------------------------------------
// Public constants
// ---------------------------------------------------------------------------

/// Sørensen–Dice similarity threshold above which two entries are considered
/// duplicates and eligible for LLM consolidation.
pub const DUPLICATE_THRESHOLD: f64 = 0.85;

/// Token budget used for the `balanced` consolidation strategy.
pub const BALANCED_MERGE_TOKEN_CAP: usize = 2000;

/// Token budget used for the `aggressive` consolidation strategy.
pub const AGGRESSIVE_MERGE_TOKEN_CAP: usize = 5000;

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

/// Controls how many entries and tokens the merge step is allowed to consume
/// in a single daydream cycle.
pub struct MergeBudget {
    /// Maximum number of provider entries to examine (after this the step
    /// truncates the candidate list).
    pub max_entries: usize,
    /// Cumulative token estimate cap across all LLM calls.  When the running
    /// total would exceed this the step short-circuits and records a
    /// `token_cap` error.
    pub token_cap: usize,
}

// ---------------------------------------------------------------------------
// Pure helper — testable without async
// ---------------------------------------------------------------------------

/// Find all index pairs `(i, j)` where `i < j` and the text similarity between
/// `entries[i].content` and `entries[j].content` is ≥ `threshold`.
///
/// Returns pairs in the order they are discovered (row-major scan).
pub fn find_duplicate_pairs(entries: &[MemoryEntry], threshold: f64) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let sim = text_similarity(&entries[i].content, &entries[j].content);
            if sim >= threshold {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

// ---------------------------------------------------------------------------
// Async step entry-point
// ---------------------------------------------------------------------------

/// Run the merge step of a daydream cycle.
///
/// 1. Enumerates all entries from the provider (up to `budget.max_entries`).
/// 2. Identifies near-duplicate pairs using bigram similarity.
/// 3. For each pair, calls the LLM to consolidate the two facts into one,
///    storing the merged content under the higher-quality entry's key and
///    deleting the lower-quality duplicate.
///
/// Returns a [`StepOutcome`] that records how many entries were examined and
/// mutated, plus any short-circuit error.
pub async fn run(
    provider: &SharedMemoryProvider,
    scorer: &QualityScorer,
    llm: &dyn UtilityLlm,
    budget: &MergeBudget,
) -> StepOutcome {
    let started = std::time::Instant::now();

    // 1. Enumerate
    let all_entries = match provider.as_ref().export(None).await {
        Ok(e) => e,
        Err(err) => {
            return StepOutcome {
                step: "merge".into(),
                examined: 0,
                mutated: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                error: Some(StepError {
                    kind: "provider".into(),
                    message: format!("export failed: {err}"),
                }),
                extracted_count: None,
                rejected_count: None,
            };
        }
    };

    // 2. Truncate to budget
    let entries: Vec<MemoryEntry> = all_entries.into_iter().take(budget.max_entries).collect();
    let examined = entries.len();

    // 3. Find duplicate pairs
    let pairs = find_duplicate_pairs(&entries, DUPLICATE_THRESHOLD);

    // 4. Process each pair
    let now = Utc::now();
    let mut tokens_spent: usize = 0;
    let mut mutated: usize = 0;

    for (i, j) in pairs {
        let a = &entries[i];
        let b = &entries[j];

        // Estimate token cost for this LLM call
        let estimated = (a.content.len() + b.content.len()) / 4 + 200;
        if tokens_spent + estimated > budget.token_cap {
            return StepOutcome {
                step: "merge".into(),
                examined,
                mutated,
                duration_ms: started.elapsed().as_millis() as u64,
                error: Some(StepError {
                    kind: "token_cap".into(),
                    message: format!(
                        "token budget exhausted: spent {tokens_spent}, would need ~{estimated} more (cap {})",
                        budget.token_cap
                    ),
                }),
                extracted_count: None,
                rejected_count: None,
            };
        }

        // Pick winner (higher quality score) and loser
        let (winner, loser) = if scorer.score(a, now) >= scorer.score(b, now) {
            (a, b)
        } else {
            (b, a)
        };

        // Build LLM prompt
        let system = "You are a memory consolidator. Merge the two given memory facts into a \
                      single concise statement. Preserve all unique information. Output only the \
                      merged fact, no preamble.";
        let user = format!("Fact A:\n{}\n\nFact B:\n{}", winner.content, loser.content);

        match llm.complete(system, &user, 256, 0.2).await {
            Ok(merged) => {
                tokens_spent += estimated;

                // Store merged content under winner's key
                if let Err(err) = provider
                    .as_ref()
                    .store(&winner.key, merged.trim(), winner.category.clone())
                    .await
                {
                    return StepOutcome {
                        step: "merge".into(),
                        examined,
                        mutated,
                        duration_ms: started.elapsed().as_millis() as u64,
                        error: Some(StepError {
                            kind: "provider".into(),
                            message: format!("store failed for key '{}': {err}", winner.key),
                        }),
                        extracted_count: None,
                        rejected_count: None,
                    };
                }

                // Delete the loser
                if let Err(err) = provider.as_ref().delete(&loser.key).await {
                    return StepOutcome {
                        step: "merge".into(),
                        examined,
                        mutated,
                        duration_ms: started.elapsed().as_millis() as u64,
                        error: Some(StepError {
                            kind: "provider".into(),
                            message: format!("delete failed for key '{}': {err}", loser.key),
                        }),
                        extracted_count: None,
                        rejected_count: None,
                    };
                }

                mutated += 1;
            }
            Err(err) => {
                return StepOutcome {
                    step: "merge".into(),
                    examined,
                    mutated,
                    duration_ms: started.elapsed().as_millis() as u64,
                    error: Some(StepError {
                        kind: "llm".into(),
                        message: format!("LLM consolidation failed: {err}"),
                    }),
                    extracted_count: None,
                    rejected_count: None,
                };
            }
        }
    }

    StepOutcome {
        step: "merge".into(),
        examined,
        mutated,
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
        extracted_count: None,
        rejected_count: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::{MemoryCategory, MemoryEntry};
    use chrono::Utc;

    fn make_entry(key: &str, content: &str) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: key.into(),
            content: content.into(),
            category: MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.5,
            session_id: None,
            project_id: None,
            quality_score: 0.5,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: 2,
            context_tags: vec![],
        }
    }

    #[test]
    fn merge_step_constants_match_spec() {
        assert_eq!(BALANCED_MERGE_TOKEN_CAP, 2000);
        assert_eq!(AGGRESSIVE_MERGE_TOKEN_CAP, 5000);
        assert!((DUPLICATE_THRESHOLD - 0.85).abs() < 1e-9);
    }

    #[test]
    fn finds_pair_for_identical_content() {
        let a = make_entry("a", "the cat sat on the mat");
        let b = make_entry("b", "the cat sat on the mat");
        let c = make_entry("c", "completely different content here");
        let pairs = find_duplicate_pairs(&[a, b, c], DUPLICATE_THRESHOLD);
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn finds_no_pair_when_below_threshold() {
        let a = make_entry("a", "the cat sat on the mat");
        let b = make_entry("b", "elephants love peanuts");
        assert!(find_duplicate_pairs(&[a, b], DUPLICATE_THRESHOLD).is_empty());
    }
}

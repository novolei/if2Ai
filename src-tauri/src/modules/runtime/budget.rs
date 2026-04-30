//! Token budget allocation and context slot tracking
//!
//! Manages how the LLM context window (default 4000 tokens) is distributed
//! across System, Episodic, Semantic, and Working memory slots.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::memory::MemoryEntry;

/// Error type for budget validation
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("percentages must sum to 1.0, got {0}")]
    PercentagesMustSumToOne(f32),

    #[error("slot not found: {0}")]
    SlotNotFound(String),

    #[error("budget exceeded: used {used} of {budget} tokens")]
    BudgetExceeded { used: usize, budget: usize },
}

/// Token slot type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotType {
    System,
    Episodic,
    Semantic,
    Working,
}

impl SlotType {
    /// Returns the string representation of this slot type
    pub fn as_str(&self) -> &str {
        match self {
            SlotType::System => "system",
            SlotType::Episodic => "episodic",
            SlotType::Semantic => "semantic",
            SlotType::Working => "working",
        }
    }
}

/// A single context slot tracking token usage and memory entries
#[derive(Debug, Clone)]
pub struct Slot {
    pub budget: usize,
    pub used: usize,
    pub entries: Vec<MemoryEntry>,
}

impl Slot {
    /// Create a new slot with the given token budget
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            entries: Vec::new(),
        }
    }

    /// Remaining tokens available in this slot
    pub fn available(&self) -> usize {
        self.budget.saturating_sub(self.used)
    }

    /// Add a memory entry to the slot, updating used token count
    pub fn add(&mut self, entry: MemoryEntry, token_count: usize) {
        self.used += token_count;
        self.entries.push(entry);
    }

    /// Remove entries from the front until used tokens are within budget
    pub fn evict<F>(&mut self, entry_token_fn: F)
    where
        F: Fn(&MemoryEntry) -> usize,
    {
        while self.used > self.budget && !self.entries.is_empty() {
            let first = self.entries.remove(0);
            let tokens = entry_token_fn(&first);
            self.used = self.used.saturating_sub(tokens);
        }
    }

    /// Remove entries with the lowest priority until within budget
    pub fn evict_lowest_priority<F>(&mut self, entry_token_fn: F)
    where
        F: Fn(&MemoryEntry) -> usize,
    {
        while self.used > self.budget && !self.entries.is_empty() {
            let mut min_idx = 0;
            for (i, entry) in self.entries.iter().enumerate().skip(1) {
                if entry.importance < self.entries[min_idx].importance {
                    min_idx = i;
                }
            }
            let removed = self.entries.remove(min_idx);
            let tokens = entry_token_fn(&removed);
            self.used = self.used.saturating_sub(tokens);
        }
    }
}

// ── Per-request preflight budget constants ──────────────────────
// Originally defined inline next to the streaming spawn body in
// `commands/agent.rs`, then carried along the MIG-001-c/d
// extraction into `application/turn_service/stream_task.rs`.
// Centralized here in budget.rs so the canonical "what limits a
// chat request" knob set lives next to `ContextBudget` /
// `estimate_tokens` rather than scattered across the streaming
// task file. Consumed by `application::prompt_planner::governor`
// (preflight trim) and by `stream_task::run_stream_task` (per-
// iteration request budgeting).

/// Hard cap on the number of messages sent to the provider in
/// any single chat request. Anything beyond is dropped from the
/// oldest end by the preflight governor.
pub const MAX_REQUEST_MESSAGE_COUNT: usize = 180;

/// Hard cap on the total character budget for one provider
/// request payload. Used by the preflight governor to drop /
/// truncate messages until the remaining payload fits.
pub const MAX_REQUEST_CHAR_BUDGET: usize = 120_000;

/// Soft estimate of the per-request output-token budget the
/// preflight governor reserves for the model's reply. Treated
/// as a lower-bound floor that the dynamic context budget will
/// always honour.
pub const MAX_REQUEST_TOKEN_BUDGET_ESTIMATE: usize = 30_000;

// ── 5-tier hard budget defaults (FEAT-TE-001) ──────────────────
// Per-tier token caps consumed by
// [`crate::modules::runtime::context_compression::TierBudgetAllocation`].
// Sum is intentionally near `MAX_REQUEST_TOKEN_BUDGET_ESTIMATE` so the
// new tier-aware preflight does not loosen the existing global ceiling.

/// Frozen identity / constitution / tool guides budget. Module A tier 1.
pub const TIER_SYSTEM_TOKENS: usize = 4_000;
/// LLM-digested history tier (populated by FEAT-TE-002). Module A tier 2.
pub const TIER_COMPRESSED_HISTORY_TOKENS: usize = 6_000;
/// `<key_info>` working checkpoint tier (FEAT-DK-002). Module A tier 3.
pub const TIER_WORKING_CHECKPOINT_TOKENS: usize = 2_000;
/// Uncompressed recent messages tier. Module A tier 4.
pub const TIER_RECENT_MESSAGES_TOKENS: usize = 12_000;
/// Current-turn tool-result buffer tier. Module A tier 5.
pub const TIER_TOOL_BUFFER_TOKENS: usize = 6_000;

/// Default streaming tier-compression budget when no env override is set.
/// Lowered from the historical 30_000 (Phase 4 T1) so typical sessions
/// (~10K tokens of history) start triggering compression. The streaming
/// preflight constructs its `TierBudgetAllocation` from this value via
/// [`streaming_tier_budget`]; callers in non-streaming paths (e.g. tests)
/// continue to use [`crate::modules::runtime::context_compression::TierBudgetAllocation::default`]
/// which still totals 30K.
pub const DEFAULT_STREAM_TIER_BUDGET: usize = 8_000;

/// Read the streaming tier-compression budget, honoring the
/// `IF2AI_STREAM_TIER_BUDGET` env var if set (parses as `usize`).
/// Falls back to [`DEFAULT_STREAM_TIER_BUDGET`].
///
/// Why this knob exists:
///   - Default 8K allows ~last 4-6 turns of typical chat to pass through uncompressed.
///   - Bump higher (e.g. 16000) if your sessions need more long-range context.
///   - Bump lower (e.g. 4000) for token-cost-sensitive deployments.
///   - Values below the 1024-token sanity floor are ignored.
pub fn streaming_tier_budget() -> usize {
    std::env::var("IF2AI_STREAM_TIER_BUDGET")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1024)
        .unwrap_or(DEFAULT_STREAM_TIER_BUDGET)
}

/// Maximum number of times the streaming task will retry a
/// `provider.stream(...)` call after a network-timeout class
/// error before giving up and surfacing the failure to the
/// frontend as `degraded_reason="timeout"`.
pub const MAX_STREAM_RETRY_ON_TIMEOUT: usize = 1;

/// Context budget configuration specifying how tokens are distributed
///
/// Default allocation (4000 tokens total):
/// - System: 10% (400 tokens) — Frozen snapshot
/// - Episodic: 20% (800 tokens) — Rolling LLM summary
/// - Semantic: 30% (1200 tokens) — Vector + FTS
/// - Working: 40% (1600 tokens) — Sliding window (8 turns)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Total budget in tokens (default: 4000)
    pub total: usize,
    /// System slot percentage (default: 10%)
    pub system_pct: f32,
    /// Episodic slot percentage (default: 20%)
    pub episodic_pct: f32,
    /// Semantic slot percentage (default: 30%)
    pub semantic_pct: f32,
    /// Working slot percentage (default: 40%)
    pub working_pct: f32,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            total: 4000,
            system_pct: 0.10,
            episodic_pct: 0.20,
            semantic_pct: 0.30,
            working_pct: 0.40,
        }
    }
}

impl ContextBudget {
    /// Create a new budget with custom total tokens, keeping default percentages
    pub fn with_total(total: usize) -> Self {
        Self {
            total,
            ..Default::default()
        }
    }

    /// System slot token allocation
    pub fn system_tokens(&self) -> usize {
        (self.total as f32 * self.system_pct) as usize
    }

    /// Episodic slot token allocation
    pub fn episodic_tokens(&self) -> usize {
        (self.total as f32 * self.episodic_pct) as usize
    }

    /// Semantic slot token allocation
    pub fn semantic_tokens(&self) -> usize {
        (self.total as f32 * self.semantic_pct) as usize
    }

    /// Working slot token allocation
    pub fn working_tokens(&self) -> usize {
        (self.total as f32 * self.working_pct) as usize
    }

    /// Validate that percentages sum to 1.0 (within floating-point tolerance)
    pub fn validate(&self) -> Result<(), BudgetError> {
        let sum = self.system_pct + self.episodic_pct + self.semantic_pct + self.working_pct;
        if (sum - 1.0).abs() > 0.001 {
            return Err(BudgetError::PercentagesMustSumToOne(sum));
        }
        Ok(())
    }
}

/// On-disk budget configuration loaded from YAML (M3).
///
/// Schema (all fields optional — missing keys fall back to [`ContextBudget::default`]):
///
/// ```yaml
/// total: 8000
/// system_pct: 0.10
/// episodic_pct: 0.20
/// semantic_pct: 0.30
/// working_pct: 0.40
/// ```
///
/// Use [`BudgetConfig::load_or_default`] to read from a path with graceful
/// degradation: missing file, malformed YAML, or invalid percentage sums all
/// log a warning and return [`ContextBudget::default`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct BudgetConfig {
    pub(crate) total: Option<usize>,
    pub(crate) system_pct: Option<f32>,
    pub(crate) episodic_pct: Option<f32>,
    pub(crate) semantic_pct: Option<f32>,
    pub(crate) working_pct: Option<f32>,
}

impl BudgetConfig {
    /// Merge this partial config onto `defaults`, producing a full
    /// [`ContextBudget`]. Validation is the caller's responsibility.
    #[must_use]
    pub(crate) fn into_budget(self, defaults: ContextBudget) -> ContextBudget {
        ContextBudget {
            total: self.total.unwrap_or(defaults.total),
            system_pct: self.system_pct.unwrap_or(defaults.system_pct),
            episodic_pct: self.episodic_pct.unwrap_or(defaults.episodic_pct),
            semantic_pct: self.semantic_pct.unwrap_or(defaults.semantic_pct),
            working_pct: self.working_pct.unwrap_or(defaults.working_pct),
        }
    }

    /// Read a YAML file and produce a validated [`ContextBudget`].
    ///
    /// Behaviour:
    /// - Missing file → returns `ContextBudget::default()` (no warning).
    /// - YAML parse / validation error → logs `warn` and returns default.
    pub(crate) fn load_or_default(path: &std::path::Path) -> ContextBudget {
        if !path.exists() {
            return ContextBudget::default();
        }
        match std::fs::read_to_string(path) {
            Ok(raw) => match serde_yaml::from_str::<BudgetConfig>(&raw) {
                Ok(cfg) => {
                    let candidate = cfg.into_budget(ContextBudget::default());
                    match candidate.validate() {
                        Ok(()) => {
                            tracing::info!(
                                "[budget] loaded BudgetConfig from {path:?}: total={}, sys={:.2}, epi={:.2}, sem={:.2}, work={:.2}",
                                candidate.total,
                                candidate.system_pct,
                                candidate.episodic_pct,
                                candidate.semantic_pct,
                                candidate.working_pct,
                            );
                            candidate
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[budget] {path:?} percentages invalid ({e}); falling back to default"
                            );
                            ContextBudget::default()
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[budget] failed to parse {path:?} as YAML ({e}); falling back to default"
                    );
                    ContextBudget::default()
                }
            },
            Err(e) => {
                tracing::warn!("[budget] failed to read {path:?} ({e}); falling back to default");
                ContextBudget::default()
            }
        }
    }
}

/// Context slots tracking actual token usage per slot
#[derive(Debug, Clone)]
pub struct ContextSlots {
    pub system: Slot,
    pub episodic: Slot,
    pub semantic: Slot,
    pub working: Slot,
}

impl ContextSlots {
    /// Create slots from a budget configuration
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            system: Slot::new(budget.system_tokens()),
            episodic: Slot::new(budget.episodic_tokens()),
            semantic: Slot::new(budget.semantic_tokens()),
            working: Slot::new(budget.working_tokens()),
        }
    }

    /// Remaining tokens available in a named slot
    pub fn available(&self, slot_type: SlotType) -> usize {
        match slot_type {
            SlotType::System => self.system.available(),
            SlotType::Episodic => self.episodic.available(),
            SlotType::Semantic => self.semantic.available(),
            SlotType::Working => self.working.available(),
        }
    }

    /// Total remaining tokens across all slots
    pub fn total_available(&self) -> usize {
        self.system.available()
            + self.episodic.available()
            + self.semantic.available()
            + self.working.available()
    }

    /// Get a mutable reference to a slot by type
    pub fn slot_mut(&mut self, slot_type: SlotType) -> Result<&mut Slot, BudgetError> {
        match slot_type {
            SlotType::System => Ok(&mut self.system),
            SlotType::Episodic => Ok(&mut self.episodic),
            SlotType::Semantic => Ok(&mut self.semantic),
            SlotType::Working => Ok(&mut self.working),
        }
    }
}

/// Estimate token count for a text string.
///
/// M2: prefers an exact `cl100k_base` BPE tokeniser (via `tiktoken-rs`) so the
/// returned value lines up with the budgeting math used by GPT-4 / GPT-3.5.
/// The BPE is lazily initialised behind a `OnceLock`; if construction ever
/// fails (e.g. in a stripped-down test build) we fall back to the historical
/// `char_len / 4 + 1` heuristic so callers never panic.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    use std::sync::OnceLock;
    use tiktoken_rs::CoreBPE;

    static BPE: OnceLock<Option<CoreBPE>> = OnceLock::new();
    let bpe = BPE.get_or_init(|| tiktoken_rs::cl100k_base().ok());

    if let Some(bpe) = bpe {
        // `encode_with_special_tokens` matches OpenAI's accounting (BOS / role
        // tokens count). For arbitrary user/assistant strings without role
        // markers this produces the same number as `encode_ordinary`.
        bpe.encode_with_special_tokens(text).len()
    } else {
        text.len() / 4 + 1
    }
}

/// Estimate token count for a memory entry
#[must_use]
pub fn estimate_entry_tokens(entry: &MemoryEntry) -> usize {
    estimate_tokens(&entry.key) + estimate_tokens(&entry.content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_entry(key: &str, content: &str, importance: f64) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: crate::modules::memory::MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance,
            access_count: 0,
            trust_score: 0.0,
            session_id: None,
            project_id: None,
        }
    }

    #[test]
    fn default_budget_values() {
        let budget = ContextBudget::default();
        assert_eq!(budget.total, 4000);
        assert_eq!(budget.system_tokens(), 400);
        assert_eq!(budget.episodic_tokens(), 800);
        assert_eq!(budget.semantic_tokens(), 1200);
        assert_eq!(budget.working_tokens(), 1600);
    }

    #[test]
    fn custom_total_budget() {
        let budget = ContextBudget::with_total(8000);
        assert_eq!(budget.total, 8000);
        assert_eq!(budget.system_tokens(), 800);
        assert_eq!(budget.working_tokens(), 3200);
    }

    #[test]
    fn validate_accepts_valid_percentages() {
        let budget = ContextBudget::default();
        assert!(budget.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_percentages() {
        let budget = ContextBudget {
            total: 4000,
            system_pct: 0.10,
            episodic_pct: 0.20,
            semantic_pct: 0.30,
            working_pct: 0.50, // sums to 1.1
        };
        let err = budget.validate().unwrap_err();
        assert!(matches!(err, BudgetError::PercentagesMustSumToOne(_)));
    }

    #[test]
    fn slots_initial_budgets() {
        let budget = ContextBudget::default();
        let slots = ContextSlots::new(budget);
        assert_eq!(slots.system.budget, 400);
        assert_eq!(slots.episodic.budget, 800);
        assert_eq!(slots.semantic.budget, 1200);
        assert_eq!(slots.working.budget, 1600);
        assert_eq!(slots.system.used, 0);
    }

    #[test]
    fn slot_available_tokens() {
        let mut slot = Slot::new(100);
        assert_eq!(slot.available(), 100);
        slot.used = 30;
        assert_eq!(slot.available(), 70);
    }

    #[test]
    fn slots_available_by_type() {
        let budget = ContextBudget::default();
        let slots = ContextSlots::new(budget);
        assert_eq!(slots.available(SlotType::System), 400);
        assert_eq!(slots.available(SlotType::Working), 1600);
        assert_eq!(slots.total_available(), 4000);
    }

    #[test]
    fn slot_add_entries() {
        let mut slot = Slot::new(1000);
        let entry = test_entry("k1", "Hello world", 0.5);
        let tokens = estimate_entry_tokens(&entry);
        slot.add(entry, tokens);
        assert_eq!(slot.entries.len(), 1);
        assert_eq!(slot.used, tokens);
    }

    #[test]
    fn slot_evict_from_front() {
        let mut slot = Slot::new(50);
        let e1 = test_entry("k1", &"a".repeat(100), 0.8);
        let e2 = test_entry("k2", &"b".repeat(100), 0.5);
        slot.add(e1, 26);
        slot.add(e2, 26);
        assert_eq!(slot.used, 52);
        assert!(slot.used > slot.budget);

        slot.evict(estimate_entry_tokens);
        assert_eq!(slot.entries.len(), 1);
        assert!(slot.used <= slot.budget);
    }

    #[test]
    fn slot_evict_lowest_priority() {
        let mut slot = Slot::new(60);
        let e1 = test_entry("k1", &"a".repeat(100), 0.9);
        let e2 = test_entry("k2", &"b".repeat(100), 0.3);
        let e3 = test_entry("k3", &"c".repeat(100), 0.7);
        slot.add(e1, 26);
        slot.add(e2, 26);
        slot.add(e3, 26);
        assert_eq!(slot.used, 78);

        slot.evict_lowest_priority(estimate_entry_tokens);
        // Should have removed e2 (lowest importance 0.3)
        assert_eq!(slot.entries.len(), 2);
        assert_eq!(slot.entries[0].key, "k1");
        assert_eq!(slot.entries[1].key, "k3");
    }

    #[test]
    fn estimate_tokens_basic() {
        // M2: estimate_tokens now uses cl100k_base. Exact counts depend on the
        // BPE; we just assert sensible bounds rather than the old char/4 values.
        assert_eq!(estimate_tokens(""), 0);
        assert!(estimate_tokens("hello").ge(&1));
        let long = estimate_tokens("abcdefghijklmnop");
        assert!(
            (1..=16).contains(&long),
            "expected 1..=16 tokens, got {long}"
        );
    }

    #[test]
    fn budget_config_defaults_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("budget.yaml");
        let cfg = BudgetConfig::load_or_default(&p);
        let default = ContextBudget::default();
        assert_eq!(cfg.total, default.total);
        assert!((cfg.system_pct - default.system_pct).abs() < 1e-6);
    }

    #[test]
    fn budget_config_loads_partial_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("budget.yaml");
        std::fs::write(&p, "total: 8000\nworking_pct: 0.50\nsemantic_pct: 0.20\n").unwrap();

        let partial: BudgetConfig =
            serde_yaml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let merged = partial.into_budget(ContextBudget::default());
        assert_eq!(merged.total, 8000);
        assert!((merged.working_pct - 0.50).abs() < 1e-6);
        assert!((merged.semantic_pct - 0.20).abs() < 1e-6);
        // sys+epi unchanged from default 0.10 + 0.20 = 0.30; total = 0.30+0.20+0.50 = 1.0 ✅
        assert!(merged.validate().is_ok());
    }

    #[test]
    fn budget_config_invalid_sum_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("budget.yaml");
        // 0.5 + 0.5 + 0.5 + 0.5 = 2.0 — invalid.
        std::fs::write(
            &p,
            "system_pct: 0.5\nepisodic_pct: 0.5\nsemantic_pct: 0.5\nworking_pct: 0.5\n",
        )
        .unwrap();

        let cfg = BudgetConfig::load_or_default(&p);
        // Should fall back to default (validates).
        assert!(cfg.validate().is_ok());
        assert!((cfg.system_pct - 0.10).abs() < 1e-6);
    }

    #[test]
    fn budget_config_malformed_yaml_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("budget.yaml");
        std::fs::write(&p, ":::: not yaml ::::").unwrap();
        let cfg = BudgetConfig::load_or_default(&p);
        assert!(cfg.validate().is_ok());
    }

    // ── streaming_tier_budget() env-var override ──────────────────
    // Note: these tests mutate process-global env state. They share a
    // single `Mutex` so they don't race each other inside the same test
    // binary even when cargo runs the module in parallel with others.
    use std::sync::Mutex;
    static STREAM_BUDGET_ENV_LOCK: Mutex<()> = Mutex::new(());
    const STREAM_BUDGET_VAR: &str = "IF2AI_STREAM_TIER_BUDGET";

    #[test]
    fn streaming_tier_budget_default_when_unset() {
        let _g = STREAM_BUDGET_ENV_LOCK.lock().unwrap();
        std::env::remove_var(STREAM_BUDGET_VAR);
        assert_eq!(streaming_tier_budget(), DEFAULT_STREAM_TIER_BUDGET);
    }

    #[test]
    fn streaming_tier_budget_honors_env_var() {
        let _g = STREAM_BUDGET_ENV_LOCK.lock().unwrap();
        std::env::set_var(STREAM_BUDGET_VAR, "12345");
        assert_eq!(streaming_tier_budget(), 12345);
        std::env::remove_var(STREAM_BUDGET_VAR);
    }

    #[test]
    fn streaming_tier_budget_rejects_below_floor() {
        let _g = STREAM_BUDGET_ENV_LOCK.lock().unwrap();
        std::env::set_var(STREAM_BUDGET_VAR, "500");
        assert_eq!(streaming_tier_budget(), DEFAULT_STREAM_TIER_BUDGET);
        std::env::remove_var(STREAM_BUDGET_VAR);
    }

    #[test]
    fn streaming_tier_budget_ignores_non_numeric() {
        let _g = STREAM_BUDGET_ENV_LOCK.lock().unwrap();
        std::env::set_var(STREAM_BUDGET_VAR, "not-a-number");
        assert_eq!(streaming_tier_budget(), DEFAULT_STREAM_TIER_BUDGET);
        std::env::remove_var(STREAM_BUDGET_VAR);
    }

    #[test]
    fn slot_type_as_str() {
        assert_eq!(SlotType::System.as_str(), "system");
        assert_eq!(SlotType::Episodic.as_str(), "episodic");
        assert_eq!(SlotType::Semantic.as_str(), "semantic");
        assert_eq!(SlotType::Working.as_str(), "working");
    }
}

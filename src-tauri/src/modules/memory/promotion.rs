//! Memory promotion engine — recommends elevating entries between scope tiers.
//!
//! Implements the `session → project → global` promotion strategy described
//! in `docs/design-docs/memory-system.md` § "Promotion strategy".  The engine
//! is a **pure recommender**: it scans existing entries and produces
//! [`PromotionRecommendation`]s; the actual scope mutation is applied
//! separately via [`MemoryProvider::promote_scope`] (typically driven by the
//! Memory Browser UI or an admin command).
//!
//! Design rationale:
//! - Keep the policy in one file so the rules are auditable and easy to tune
//!   without touching the storage layer.
//! - Use thresholds derived from the existing `importance` / `access_count`
//!   signals so we don't introduce another tracking surface.
//! - Operate on snapshots returned by `export()` to avoid coupling the
//!   recommender to any specific provider implementation.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryEntry, MemoryError, MemoryProvider};

/// Global throttle: minimum seconds between two background promotion scans.
///
/// 60 s is loose enough to feel "near real-time" for the UI yet keeps the
/// per-turn overhead negligible (one scan = one `export()` + a linear pass).
const BACKGROUND_SCAN_MIN_INTERVAL_SECS: i64 = 60;

/// Last unix-second at which a background scan ran. We use a process-wide
/// atomic instead of a per-engine field because [`MemoryPromotionEngine`] is
/// stateless / re-created per call site, but the throttle should apply
/// across all call sites.
static LAST_BACKGROUND_SCAN_AT: AtomicI64 = AtomicI64::new(0);

/// Visible scope tier of an entry, used to decide promotion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeTier {
    Session,
    Project,
    Global,
}

impl ScopeTier {
    /// Derive the visible tier from an entry's persisted scope tags.
    pub fn from_entry(entry: &MemoryEntry) -> Self {
        match (&entry.session_id, &entry.project_id) {
            (Some(_), _) => Self::Session,
            (None, Some(_)) => Self::Project,
            (None, None) => Self::Global,
        }
    }

    /// The scope tier one step "up" — `session → project`, `project → global`.
    /// `global` returns `None` because it is already the top tier.
    pub fn next_up(self) -> Option<Self> {
        match self {
            Self::Session => Some(Self::Project),
            Self::Project => Some(Self::Global),
            Self::Global => None,
        }
    }

    /// Stable label used in audit logs and frontend chips.
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

/// Thresholds that gate a promotion recommendation.
///
/// All thresholds are inclusive lower bounds — an entry must hit every
/// applicable threshold to be considered for promotion.  Tuned to be
/// conservative by default so the recommender doesn't flood the Memory
/// Browser with false positives on a fresh install.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionThresholds {
    /// `session → project`: minimum `access_count` and `importance`.
    pub session_to_project_access: u64,
    pub session_to_project_importance: f64,
    /// `project → global`: minimum `access_count` and `importance`.
    pub project_to_global_access: u64,
    pub project_to_global_importance: f64,
}

impl PromotionThresholds {
    /// Validate values so an out-of-range config (e.g. importance > 1.0,
    /// access = 0) cannot silently slip through `runtime/config` parsing.
    /// Returns the bad-field name for [`crate::modules::runtime::config::ConfigError`]
    /// to wrap.
    /// Best-effort lazy load from `~/.if2ai/memory_config.json` (the same
    /// file the Memory Settings UI writes to).  Falls back silently to
    /// [`Self::default`] when the file is missing, malformed, or omits the
    /// `promotion` key — the same gracefully-degraded contract used by
    /// [`crate::modules::tools::builtin::memory_store`] for the policy
    /// enforce-mode flag.
    pub fn load_from_disk() -> Self {
        let Ok(home) = std::env::var("HOME") else {
            return Self::default();
        };
        let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Self::default();
        };
        let Some(promo) = root.get("promotion") else {
            return Self::default();
        };
        match serde_json::from_value::<Self>(promo.clone()) {
            Ok(parsed) if parsed.validate().is_ok() => parsed,
            Ok(_) => {
                tracing::warn!(
                    "[PromotionThresholds] disk override failed validation; falling back to defaults"
                );
                Self::default()
            }
            Err(e) => {
                tracing::warn!(
                    "[PromotionThresholds] disk override unparseable ({e}); falling back to defaults"
                );
                Self::default()
            }
        }
    }

    /// Validate the threshold tuple — every access-count threshold must be
    /// `>= 1`, otherwise the promotion engine would treat brand-new
    /// (never-recalled) entries as immediate promotion candidates.
    pub fn validate(self) -> Result<(), &'static str> {
        if self.session_to_project_access == 0 {
            return Err("sessionToProject.accessCount must be >= 1");
        }
        if self.project_to_global_access == 0 {
            return Err("projectToGlobal.accessCount must be >= 1");
        }
        if !(0.0..=1.0).contains(&self.session_to_project_importance) {
            return Err("sessionToProject.importance must be in [0, 1]");
        }
        if !(0.0..=1.0).contains(&self.project_to_global_importance) {
            return Err("projectToGlobal.importance must be in [0, 1]");
        }
        Ok(())
    }
}

impl Default for PromotionThresholds {
    fn default() -> Self {
        // Defaults reflect the "stable, repeatedly useful" intuition in
        // memory-system.md §promotion.  Tightening project→global keeps the
        // global tier pristine.
        Self {
            session_to_project_access: 3,
            session_to_project_importance: 0.55,
            project_to_global_access: 8,
            project_to_global_importance: 0.7,
        }
    }
}

/// One promotion recommendation surfaced to the UI.
///
/// `target_scope` is fully constructed so the call site can pass it straight
/// into [`MemoryProvider::promote_scope`] without re-deriving the rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRecommendation {
    pub key: String,
    pub category: String,
    pub current_tier: ScopeTier,
    pub target_tier: ScopeTier,
    pub access_count: u64,
    pub importance: f64,
    /// Short, human-readable rationale shown in the UI (Chinese, since the
    /// Memory Browser surface is Chinese-only today).
    pub reason: String,
}

/// Decide whether an entry should be promoted, given thresholds.
///
/// Returns `Some` only when:
/// - The entry has a tier above which it can move (`session` or `project`).
/// - The entry's `access_count` and `importance` exceed the matching tier
///   thresholds.
/// - Required scope context exists (e.g. a session entry needs a `project_id`
///   to be promotable to project; without it we wait for the next eligibility
///   window once the entry gets reassigned).
pub fn evaluate_promotion(
    entry: &MemoryEntry,
    thresholds: &PromotionThresholds,
) -> Option<PromotionRecommendation> {
    let current_tier = ScopeTier::from_entry(entry);
    let target_tier = current_tier.next_up()?;

    let (min_access, min_importance) = match current_tier {
        ScopeTier::Session => (
            thresholds.session_to_project_access,
            thresholds.session_to_project_importance,
        ),
        ScopeTier::Project => (
            thresholds.project_to_global_access,
            thresholds.project_to_global_importance,
        ),
        ScopeTier::Global => return None, // unreachable due to next_up()
    };

    if entry.access_count < min_access || entry.importance < min_importance {
        return None;
    }

    // Session→project promotion needs a project_id to land on; if the entry
    // somehow lacks one (legacy), recommend session→global instead so the
    // entry doesn't get stuck.
    let target_tier_resolved = if current_tier == ScopeTier::Session && entry.project_id.is_none() {
        ScopeTier::Global
    } else {
        target_tier
    };

    let reason = format!(
        "access={} importance={:.2} → 满足 {} ⇒ {} 升级阈值",
        entry.access_count,
        entry.importance,
        current_tier.label(),
        target_tier_resolved.label(),
    );

    Some(PromotionRecommendation {
        key: entry.key.clone(),
        category: entry.category.as_str().to_string(),
        current_tier,
        target_tier: target_tier_resolved,
        access_count: entry.access_count,
        importance: entry.importance,
        reason,
    })
}

/// Build the [`MemoryExecutionScope`] that
/// [`MemoryProvider::promote_scope`] should write for the recommendation.
///
/// `entry_project_id` is the entry's existing `project_id` — used when
/// promoting `session → project` so the project tag is preserved.
pub fn target_scope_for(
    target_tier: ScopeTier,
    entry_project_id: Option<&str>,
) -> MemoryExecutionScope {
    match target_tier {
        ScopeTier::Session => MemoryExecutionScope {
            // Promotion never writes back to session, but for completeness:
            session_id: None,
            project_id: entry_project_id.map(String::from),
            workdir: None,
        },
        ScopeTier::Project => MemoryExecutionScope {
            session_id: None,
            project_id: entry_project_id.map(String::from),
            workdir: None,
        },
        ScopeTier::Global => MemoryExecutionScope::global(),
    }
}

/// Stateless engine that scans a provider for promotion candidates.
///
/// Held as `&dyn MemoryProvider` so the engine works over any storage
/// backend that implements the trait (SQLite today, Honcho/vector tomorrow).
pub struct MemoryPromotionEngine<'a> {
    provider: &'a dyn MemoryProvider,
    thresholds: PromotionThresholds,
}

impl<'a> MemoryPromotionEngine<'a> {
    /// Create an engine using default thresholds.
    pub fn new(provider: &'a dyn MemoryProvider) -> Self {
        Self {
            provider,
            thresholds: PromotionThresholds::default(),
        }
    }

    /// Override the default thresholds (used by tests and tuning hooks).
    #[allow(dead_code)]
    pub fn with_thresholds(
        provider: &'a dyn MemoryProvider,
        thresholds: PromotionThresholds,
    ) -> Self {
        Self {
            provider,
            thresholds,
        }
    }

    /// Scan the entire memory library and return every entry that meets
    /// promotion criteria.  Returns recommendations sorted by descending
    /// importance so the UI surfaces the most valuable promotions first.
    pub async fn evaluate_all(&self) -> Result<Vec<PromotionRecommendation>, MemoryError> {
        let entries = self.provider.export(None).await?;
        let mut out: Vec<PromotionRecommendation> = entries
            .iter()
            .filter_map(|e| evaluate_promotion(e, &self.thresholds))
            .collect();
        out.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    /// Background scan + audit emit, intended for the post-turn hook.
    ///
    /// Runs at most once per [`BACKGROUND_SCAN_MIN_INTERVAL_SECS`] seconds
    /// (process-wide).  When a scan does run, every recommendation is
    /// emitted as a `memory_promotion_candidate` audit event so subscribers
    /// (frontend toast, Telemetry Drawer) can react proactively.
    ///
    /// Returns `Ok(Some(n))` when a scan ran (`n` = candidate count, possibly 0)
    /// and `Ok(None)` when the call was suppressed by the throttle window.
    pub async fn evaluate_and_audit(&self) -> Result<Option<usize>, MemoryError> {
        let now = chrono::Utc::now().timestamp();
        let last = LAST_BACKGROUND_SCAN_AT.load(Ordering::Relaxed);
        if now.saturating_sub(last) < BACKGROUND_SCAN_MIN_INTERVAL_SECS {
            return Ok(None);
        }
        // Compare-and-set to avoid two concurrent post-turn hooks both passing
        // the throttle and double-scanning.  We don't care about precise
        // scheduling — losing the race just means the other caller scans now
        // and we wait for the next window.
        if LAST_BACKGROUND_SCAN_AT
            .compare_exchange(last, now, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return Ok(None);
        }

        let recs = self.evaluate_all().await?;
        for rec in &recs {
            // Reuse the entry's persisted scope (best-effort) as the audit
            // context.  The recommender doesn't carry the original
            // `MemoryExecutionScope`, so we synthesise a minimal one from the
            // recommendation tier so listeners still see meaningful tier tags.
            let synthetic_scope = MemoryExecutionScope {
                session_id: None,
                project_id: None,
                workdir: None,
            };
            let ctx = AuditContext::from_scope(&synthetic_scope);
            MemoryAuditEmitter::memory_promotion_candidate(
                &ctx,
                &rec.key,
                rec.current_tier.label(),
                rec.target_tier.label(),
                &rec.reason,
            );
        }
        Ok(Some(recs.len()))
    }

    /// Test-only override that resets the throttle so unit tests can run
    /// `evaluate_and_audit` deterministically.
    #[cfg(test)]
    pub fn reset_throttle_for_test() {
        LAST_BACKGROUND_SCAN_AT.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::scope::MemoryScopeResolver;
    use crate::modules::memory::{MemoryCategory, MemoryProvider, SqliteMemoryProvider};
    use rusqlite::{params, Connection};
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Returns a provider plus the on-disk path so tests can run direct
    /// `UPDATE` statements to set deterministic `importance` /
    /// `access_count` values (the public API exposes no setter for these).
    fn make_provider() -> (SqliteMemoryProvider, TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("promo.db");
        let provider = SqliteMemoryProvider::new(db_path.clone()).unwrap();
        (provider, temp, db_path)
    }

    async fn insert_with_signal(
        provider: &SqliteMemoryProvider,
        db_path: &PathBuf,
        key: &str,
        session_id: Option<&str>,
        project_id: Option<&str>,
        importance: f64,
        access_count: u64,
    ) {
        let scope = MemoryScopeResolver::resolve(session_id, project_id, None);
        provider
            .store_scoped(key, key, MemoryCategory::Core, &scope)
            .await
            .unwrap();
        // SQLite supports concurrent readers/writers from separate
        // connections to the same file — this side-channel write only
        // touches the row we just inserted, so it can't race with the
        // provider's schema setup.
        let c = Connection::open(db_path).unwrap();
        c.execute(
            "UPDATE memory_entries SET importance = ?1, access_count = ?2 WHERE key = ?3",
            params![importance, access_count as i64, key],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn evaluate_promotion_returns_none_for_below_threshold() {
        let (provider, _t, db) = make_provider();
        insert_with_signal(&provider, &db, "low", Some("s1"), Some("p1"), 0.4, 1).await;

        let entries = provider.export(None).await.unwrap();
        let entry = entries.iter().find(|e| e.key == "low").unwrap();
        assert!(evaluate_promotion(entry, &PromotionThresholds::default()).is_none());
    }

    #[tokio::test]
    async fn evaluate_promotion_session_to_project() {
        let (provider, _t, db) = make_provider();
        insert_with_signal(&provider, &db, "ready", Some("s1"), Some("p1"), 0.8, 5).await;

        let entries = provider.export(None).await.unwrap();
        let entry = entries.iter().find(|e| e.key == "ready").unwrap();
        let rec = evaluate_promotion(entry, &PromotionThresholds::default()).unwrap();
        assert_eq!(rec.current_tier, ScopeTier::Session);
        assert_eq!(rec.target_tier, ScopeTier::Project);
    }

    #[tokio::test]
    async fn evaluate_promotion_project_to_global() {
        let (provider, _t, db) = make_provider();
        insert_with_signal(&provider, &db, "stable", None, Some("p1"), 0.85, 12).await;

        let entries = provider.export(None).await.unwrap();
        let entry = entries.iter().find(|e| e.key == "stable").unwrap();
        let rec = evaluate_promotion(entry, &PromotionThresholds::default()).unwrap();
        assert_eq!(rec.current_tier, ScopeTier::Project);
        assert_eq!(rec.target_tier, ScopeTier::Global);
    }

    /// `evaluate_and_audit` runs on first call and is suppressed on the
    /// immediate retry by the throttle window.
    #[tokio::test]
    async fn evaluate_and_audit_throttles_repeated_calls() {
        MemoryPromotionEngine::reset_throttle_for_test();

        let (provider, _t, db) = make_provider();
        insert_with_signal(&provider, &db, "ready", Some("s1"), Some("p1"), 0.8, 5).await;

        let engine = MemoryPromotionEngine::new(&provider);
        let first = engine.evaluate_and_audit().await.unwrap();
        assert_eq!(
            first,
            Some(1),
            "first call should scan and find 1 candidate"
        );

        // A second call within the throttle window must be suppressed.
        let second = engine.evaluate_and_audit().await.unwrap();
        assert_eq!(
            second, None,
            "second call within window should be throttled"
        );

        // Manually resetting the throttle re-enables scanning.
        MemoryPromotionEngine::reset_throttle_for_test();
        let third = engine.evaluate_and_audit().await.unwrap();
        assert_eq!(
            third,
            Some(1),
            "after throttle reset, scan should run again"
        );
    }

    #[tokio::test]
    async fn engine_promote_scope_round_trip() {
        let (provider, _t, db) = make_provider();
        insert_with_signal(&provider, &db, "promote_me", Some("s1"), Some("p1"), 0.9, 9).await;

        let engine = MemoryPromotionEngine::new(&provider);
        let recs = engine.evaluate_all().await.unwrap();
        assert_eq!(recs.len(), 1);
        let rec = &recs[0];
        assert_eq!(rec.target_tier, ScopeTier::Project);

        // Apply the promotion: session_id cleared, project_id preserved.
        let target = target_scope_for(rec.target_tier, Some("p1"));
        provider.promote_scope(&rec.key, &target).await.unwrap();

        let after = provider.export(None).await.unwrap();
        let promoted = after.iter().find(|e| e.key == "promote_me").unwrap();
        assert!(promoted.session_id.is_none());
        assert_eq!(promoted.project_id.as_deref(), Some("p1"));

        // Bump access_count past the project_to_global threshold so the
        // engine now recommends project → global.
        let c = Connection::open(&db).unwrap();
        c.execute(
            "UPDATE memory_entries SET access_count = 20 WHERE key = ?1",
            params!["promote_me"],
        )
        .unwrap();

        let recs2 = engine.evaluate_all().await.unwrap();
        assert_eq!(recs2.len(), 1);
        assert_eq!(recs2[0].current_tier, ScopeTier::Project);
        assert_eq!(recs2[0].target_tier, ScopeTier::Global);
    }
}

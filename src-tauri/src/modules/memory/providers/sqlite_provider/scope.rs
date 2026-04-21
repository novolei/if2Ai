//! Scope-aware visibility / params / ordering helpers for SQLite memory queries.
//!
//! Extracted from `sqlite_provider/mod.rs` in GFR-T1-D-1 (pure structural
//! move; function bodies byte-identical).

use crate::modules::memory::MemoryCategory;

/// Build the SQL `WHERE` fragment that enforces the three-tier visibility rules
/// described on `recall_scoped`.
///
/// Returns a fragment that uses positional parameters `?1`..`?N`, where `N`
/// matches the length of the slice returned by [`scope_visibility_params`].
/// The returned fragment is intentionally wrapped in parentheses by the caller
/// so it can be combined with additional `AND` clauses (e.g. category filter).
pub(super) fn scope_visibility_clause(
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> String {
    match (session_id, project_id) {
        // session + project: own session entries OR project-level entries OR global.
        (Some(_), Some(_)) => "session_id = ?1 \
             OR (session_id IS NULL AND project_id = ?2) \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // session only: own session entries OR global (legacy callers without project).
        (Some(_), None) => "session_id = ?1 \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // project only: project-level entries OR global. No session entries leak across.
        (None, Some(_)) => "(session_id IS NULL AND project_id = ?1) \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // global: only truly unscoped entries.
        (None, None) => "session_id IS NULL AND project_id IS NULL".to_string(),
    }
}

/// Build the bound parameter list aligned with [`scope_visibility_clause`].
pub(super) fn scope_visibility_params(
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> Vec<String> {
    match (session_id, project_id) {
        (Some(s), Some(p)) => vec![s.to_string(), p.to_string()],
        (Some(s), None) => vec![s.to_string()],
        (None, Some(p)) => vec![p.to_string()],
        (None, None) => Vec::new(),
    }
}

/// SQL `ORDER BY` fragment that ranks recall results by **scope tier first**,
/// then by importance / access_count / recency.  Lower tier number wins.
///
/// Tiering (must match `scope_visibility_clause` so every visible row has a
/// well-defined tier):
///   - 0 = session-owned (`session_id` matches the caller)
///   - 1 = project-owned (`session_id IS NULL AND project_id` matches)
///   - 2 = global / legacy (both NULL)
///
/// Within a tier we surface the most "trusted-and-frequently-used" memory
/// first: `importance DESC, access_count DESC, updated_at DESC`.
///
/// Returns a clause without the leading `ORDER BY` keyword so the caller can
/// inline it after `WHERE (...)`.
pub(super) fn scope_priority_order_by(
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> String {
    // The CASE expression is parameter-free and safe to inline because both
    // operands come from already-bound `?N` slots; we just *reference* them.
    let case_expr = match (session_id, project_id) {
        (Some(_), Some(_)) => {
            // ?1 = session, ?2 = project — same parameter positions used by
            // `scope_visibility_clause`.
            "CASE \
                 WHEN session_id = ?1 THEN 0 \
                 WHEN session_id IS NULL AND project_id = ?2 THEN 1 \
                 ELSE 2 \
             END"
        }
        (Some(_), None) => {
            // ?1 = session.  No project, so tier 1 collapses into tier 2.
            "CASE WHEN session_id = ?1 THEN 0 ELSE 2 END"
        }
        (None, Some(_)) => {
            // ?1 = project.  No session-tier rows are visible; collapse to 1/2.
            "CASE WHEN session_id IS NULL AND project_id = ?1 THEN 1 ELSE 2 END"
        }
        (None, None) => {
            // Only globals are visible; constant tier.
            "2"
        }
    };
    format!(
        "{case_expr} ASC, \
         importance DESC, \
         access_count DESC, \
         updated_at DESC"
    )
}

/// Parse a category string, handling both known and custom categories
pub(super) fn parse_category(s: &str) -> MemoryCategory {
    match s {
        "core" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
        other => MemoryCategory::Custom(other.to_string()),
    }
}

//! MEM-MOD-P7 — cross-session learned traits.
//!
//! What this is for: by the end of a session the agent has accumulated
//! a handful of `Reflection`-category memories (see MEM-MOD-P5) like
//! "the user prefers terse, code-first answers".  Most of those don't
//! survive cross-session noise — they're tied to one task.  P7 distills
//! the recurring observations into a small, durable list of *traits*
//! the agent treats as foundational on every future turn:
//!
//! ```text
//! "你之前观察到：RL prefers terse answers; RL ships at 3 AM ..."
//! ```
//!
//! Storage: SQLite `learned_traits` table (migration v4).  Reads are
//! covered by a partial index `WHERE disagreed_at IS NULL` so the
//! prompt-block hot path stays cheap.
//!
//! Lifecycle:
//!   1. `extract_from_session(llm, session_id, reflections)` runs at
//!      session-end.  It asks a small LLM call to compress the
//!      reflections into 1–3 single-line traits, then upserts them.
//!   2. `list_active(limit)` is the read API consumed by the
//!      `Learned Traits` prompt block (priority 93).
//!   3. `disagree(id)` is the user's "I don't agree" button — sets
//!      `disagreed_at`, evicting the trait from future prompts.
//!
//! All SQL runs through the same `Arc<Mutex<Connection>>` the
//! `SqliteMemoryProvider` already owns, so we share the transaction
//! semantics + don't open a second DB handle.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::MemoryError;

/// One row of the `learned_traits` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnedTrait {
    pub id: i64,
    pub trait_text: String,
    pub evidence_count: i64,
    pub confidence: f64,
    pub first_seen_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub disagreed_at: Option<DateTime<Utc>>,
    pub source_session: Option<String>,
}

/// Thin store that wraps the shared SQLite connection.  Held by
/// `AppState` so commands + the ticker share a single instance.
#[derive(Clone)]
pub struct LearnedTraitsStore {
    conn: Arc<Mutex<Connection>>,
}

impl LearnedTraitsStore {
    /// Wrap an existing connection.  Caller is responsible for having
    /// run `migrations::memory_migrations()` first (otherwise the v4
    /// table will not exist).
    #[must_use]
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// MEM-MOD-P7 — open a fresh SQLite handle against `db_path` and
    /// run the memory migration set so the `learned_traits` table is
    /// guaranteed to exist.  Used by bootstrap to wire a store
    /// alongside the `SqliteMemoryProvider` without sharing its
    /// private connection.
    pub fn open(db_path: &std::path::Path) -> Result<Self, MemoryError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| MemoryError::Generic(format!("create_dir_all failed: {e}")))?;
        }
        let conn = Connection::open(db_path)
            .map_err(|e| MemoryError::Generic(format!("open db failed: {e}")))?;
        crate::modules::memory::migrations::run_migrations(
            &conn,
            crate::modules::memory::migrations::memory_migrations(),
            Some("memory_entries"),
        )
        .map_err(|e| MemoryError::Generic(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Insert a brand-new trait or bump `evidence_count` + `confidence`
    /// when the same `trait_text` already exists (case-sensitive
    /// matching — the LLM normalises wording before this layer sees it).
    pub fn upsert(
        &self,
        trait_text: &str,
        source_session: Option<&str>,
    ) -> Result<i64, MemoryError> {
        let c = self
            .conn
            .lock()
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let existing: Option<(i64, i64, f64)> = c
            .query_row(
                "SELECT id, evidence_count, confidence
                 FROM learned_traits
                 WHERE trait_text = ?1 AND disagreed_at IS NULL",
                params![trait_text],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();
        if let Some((id, count, conf)) = existing {
            // Confidence saturates near 1.0 — every additional sighting
            // shrinks the gap by 30 %.
            let new_conf = (conf + (1.0 - conf) * 0.3).min(0.99);
            c.execute(
                "UPDATE learned_traits
                 SET evidence_count = ?1, confidence = ?2, last_updated_at = ?3
                 WHERE id = ?4",
                params![count + 1, new_conf, now, id],
            )
            .map_err(|e| MemoryError::Generic(format!("trait bump failed: {e}")))?;
            return Ok(id);
        }
        c.execute(
            "INSERT INTO learned_traits
                (trait_text, evidence_count, confidence,
                 first_seen_at, last_updated_at, source_session)
             VALUES (?1, 1, 0.5, ?2, ?2, ?3)",
            params![trait_text, now, source_session],
        )
        .map_err(|e| MemoryError::Generic(format!("trait insert failed: {e}")))?;
        Ok(c.last_insert_rowid())
    }

    /// Newest non-disagreed traits first (used by both UI + prompt).
    pub fn list_active(&self, limit: usize) -> Result<Vec<LearnedTrait>, MemoryError> {
        let c = self
            .conn
            .lock()
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        let mut stmt = c
            .prepare(
                "SELECT id, trait_text, evidence_count, confidence,
                        first_seen_at, last_updated_at, disagreed_at, source_session
                 FROM learned_traits
                 WHERE disagreed_at IS NULL
                 ORDER BY last_updated_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let parse = |i: usize| -> Result<DateTime<Utc>, rusqlite::Error> {
                    let s: String = row.get(i)?;
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|_| {
                            rusqlite::Error::InvalidColumnType(
                                i,
                                "rfc3339".into(),
                                rusqlite::types::Type::Text,
                            )
                        })
                };
                let disagreed = row.get::<_, Option<String>>(6)?.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                });
                Ok(LearnedTrait {
                    id: row.get(0)?,
                    trait_text: row.get(1)?,
                    evidence_count: row.get(2)?,
                    confidence: row.get(3)?,
                    first_seen_at: parse(4)?,
                    last_updated_at: parse(5)?,
                    disagreed_at: disagreed,
                    source_session: row.get(7)?,
                })
            })
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows.flatten() {
            out.push(r);
        }
        Ok(out)
    }

    /// Mark a trait as disagreed-with so the prompt block stops
    /// surfacing it.  We keep the row (audit trail), only set
    /// `disagreed_at`.
    pub fn disagree(&self, id: i64) -> Result<(), MemoryError> {
        let c = self
            .conn
            .lock()
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        let rows = c
            .execute(
                "UPDATE learned_traits SET disagreed_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), id],
            )
            .map_err(|e| MemoryError::Generic(format!("disagree failed: {e}")))?;
        if rows == 0 {
            return Err(MemoryError::KeyNotFound(format!("trait id {id}")));
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────
// Extraction (LLM)
// ────────────────────────────────────────────────────────────────────

/// Build the (system, user) prompt the trait extractor uses.  The
/// reflections list comes from `MemoryProvider::recall("",
/// Some("reflection"), N)` filtered by session.
#[must_use]
pub fn build_extractor_prompt(reflections: &[String]) -> (String, String) {
    let system = "You are the agent's trait distiller. Read the recent reflection notes and \
extract 1–3 DURABLE observations about the user (preferences, habits, goals \
that will likely hold true tomorrow). Output ONE JSON array of strings, no \
prose. Each string MUST be a single short sentence (≤ 100 chars), \
self-contained, written in the agent's first-person voice. \
If nothing rises to that bar, return an empty array []."
        .to_string();

    let mut user = String::new();
    user.push_str("REFLECTIONS:\n");
    if reflections.is_empty() {
        user.push_str("(none)\n");
    } else {
        for (i, r) in reflections.iter().enumerate() {
            user.push_str(&format!("{i}. {}\n", r.trim()));
        }
    }
    user.push_str("\nReturn the JSON array now.");
    (system, user)
}

/// Parse the LLM's JSON-array response into trimmed trait strings.
/// Returns an empty vec on any parse failure (no panic — extractor is
/// best-effort).
#[must_use]
pub fn parse_traits(raw: &str) -> Vec<String> {
    let body = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```");
    let body = body.trim_end_matches("```").trim();
    serde_json::from_str::<Vec<String>>(body)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().count() <= 200)
        .collect()
}

/// End-to-end: distill `reflections` via `llm` and upsert each trait.
/// Returns the number of traits persisted (new or bumped).
pub async fn extract_and_persist<L: UtilityLlm + ?Sized>(
    llm: &L,
    store: &LearnedTraitsStore,
    session_id: &str,
    reflections: &[String],
) -> Result<usize, MemoryError> {
    if reflections.is_empty() {
        return Ok(0);
    }
    let (sys, usr) = build_extractor_prompt(reflections);
    let raw = llm.complete(&sys, &usr, 384, 0.3).await?;
    let traits = parse_traits(&raw);
    let mut written = 0usize;
    for t in &traits {
        if store.upsert(t, Some(session_id)).is_ok() {
            written += 1;
        }
    }
    Ok(written)
}

// ────────────────────────────────────────────────────────────────────
// Prompt block rendering
// ────────────────────────────────────────────────────────────────────

/// Render the `Learned Traits` prompt block from the active trait
/// list.  Returns `None` when no traits are active (so the planner
/// can skip the block entirely instead of emitting an empty header).
#[must_use]
pub fn render_learned_traits_block(traits: &[LearnedTrait]) -> Option<String> {
    if traits.is_empty() {
        return None;
    }
    let mut out = String::from("# Learned Traits · 关于用户的累积观察\n");
    out.push_str("(累积自跨 session 反思；如错请用 Settings → 我不同意 撤回。)\n\n");
    for t in traits.iter().take(8) {
        out.push_str(&format!(
            "- {} (置信度 {:.2}, 证据 ×{})\n",
            t.trait_text, t.confidence, t.evidence_count
        ));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn make_store() -> LearnedTraitsStore {
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::memory::migrations::run_migrations(
            &conn,
            crate::modules::memory::migrations::memory_migrations(),
            None,
        )
        .unwrap();
        LearnedTraitsStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn upsert_inserts_then_bumps() {
        let s = make_store();
        let a = s
            .upsert("RL prefers terse replies", Some("sess-1"))
            .unwrap();
        let b = s
            .upsert("RL prefers terse replies", Some("sess-2"))
            .unwrap();
        assert_eq!(a, b, "second upsert MUST hit the same row");
        let traits = s.list_active(10).unwrap();
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].evidence_count, 2);
        assert!(traits[0].confidence > 0.5, "confidence must rise on bump");
    }

    #[test]
    fn list_active_excludes_disagreed() {
        let s = make_store();
        let id = s.upsert("RL hates emojis", None).unwrap();
        s.disagree(id).unwrap();
        assert!(s.list_active(10).unwrap().is_empty());
    }

    #[test]
    fn disagree_unknown_id_returns_not_found() {
        let s = make_store();
        let err = s.disagree(999).unwrap_err();
        assert!(err.to_string().contains("trait id 999"));
    }

    #[test]
    fn parse_traits_strips_fence_and_filters_long_strings() {
        let raw = "```json\n[\"RL ships at 3 AM\", \"\", \"   \"]\n```";
        let parsed = parse_traits(raw);
        assert_eq!(parsed, vec!["RL ships at 3 AM".to_string()]);
    }

    #[test]
    fn parse_traits_returns_empty_on_garbage() {
        assert!(parse_traits("not a json array").is_empty());
    }

    #[test]
    fn build_prompt_includes_reflections_or_none_marker() {
        let (_sys, usr) = build_extractor_prompt(&["a".into(), "b".into()]);
        assert!(usr.contains("0. a"));
        assert!(usr.contains("1. b"));

        let (_sys2, usr2) = build_extractor_prompt(&[]);
        assert!(usr2.contains("(none)"));
    }

    #[test]
    fn render_block_returns_none_when_empty() {
        assert!(render_learned_traits_block(&[]).is_none());
    }

    #[test]
    fn render_block_includes_each_trait_with_stats() {
        let now = Utc::now();
        let t = LearnedTrait {
            id: 1,
            trait_text: "RL prefers terse".into(),
            evidence_count: 3,
            confidence: 0.78,
            first_seen_at: now,
            last_updated_at: now,
            disagreed_at: None,
            source_session: None,
        };
        let block = render_learned_traits_block(&[t]).unwrap();
        assert!(block.contains("RL prefers terse"));
        assert!(block.contains("0.78"));
        assert!(block.contains("×3"));
    }

    #[tokio::test]
    async fn extract_and_persist_writes_n_traits() {
        use crate::modules::memory::llm::MockUtilityLlm;
        let s = make_store();
        let llm = MockUtilityLlm::new(vec![
            r#"["RL ships at 3 AM", "RL prefers code-first"]"#.to_string()
        ]);
        let n = extract_and_persist(&llm, &s, "sess-x", &["a".into(), "b".into()])
            .await
            .unwrap();
        assert_eq!(n, 2);
        let traits = s.list_active(10).unwrap();
        assert_eq!(traits.len(), 2);
    }

    #[tokio::test]
    async fn extract_skips_when_reflections_empty() {
        use crate::modules::memory::llm::MockUtilityLlm;
        let s = make_store();
        let llm = MockUtilityLlm::new(vec![]);
        let n = extract_and_persist(&llm, &s, "sess-x", &[]).await.unwrap();
        assert_eq!(n, 0);
    }
}

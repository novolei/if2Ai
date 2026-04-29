//! FEAT-DK-001 — Domain knowledge repository (in-memory).
//!
//! Three knowledge kinds (Module G in
//! `.qoder/specs/if2ai-agent-evolution-report.md`):
//!
//! - **WebsiteDomain** — URL patterns, selectors, gotchas for one site.
//! - **InteractionPrimitive** — UI patterns (dialogs, tabs, uploads).
//! - **TaskSOP** — task-level Standard Operating Procedure.
//!
//! Storage abstraction: [`KnowledgeStore`] async trait. Production
//! callers will wrap sqlite / lancedb (deferred to a wiring Pack);
//! tests + DK-003 use the in-file [`mock::MockKnowledgeStore`].
//!
//! Draft-only by Pack contract: no `~/.if2ai/` writes, no
//! work_loop / prompt_planner integration in this Pack.

#![allow(dead_code)]

pub mod contributor;

#[allow(unused_imports)]
pub use contributor::{
    extract_domain_knowledge_candidates, verify_knowledge_safety, KnowledgeVerdict,
};

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stability annotation for one selector. `Stable` selectors survive
/// version bumps; `Fragile` ones break on minor UI shifts; `Untested`
/// have not been re-verified since contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorStability {
    Stable,
    Fragile,
    Untested,
}

/// One CSS / XPath / role-based selector entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorEntry {
    pub selector: String,
    pub purpose: String,
    pub stability: SelectorStability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<DateTime<Utc>>,
}

/// One step inside a [`DomainKnowledgeKind::TaskSOP`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SOPStep {
    pub tool_name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args_template: Option<String>,
    pub expected_result: String,
}

/// Categorical kind of one [`DomainKnowledgeEntry`]. Closed enum so
/// downstream consumers can pattern-match exhaustively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainKnowledgeKind {
    WebsiteDomain {
        domain: String,
        url_patterns: Vec<String>,
        selectors: Vec<SelectorEntry>,
        gotchas: Vec<String>,
    },
    InteractionPrimitive {
        category: String,
        problem_statement: String,
        solution_code: String,
        tradeoffs: Vec<String>,
    },
    TaskSOP {
        task_type: String,
        prerequisites: Vec<String>,
        key_pitfalls: Vec<String>,
        execution_steps: Vec<SOPStep>,
    },
}

impl DomainKnowledgeKind {
    /// Stable label used by [`KnowledgeStore::lookup`]'s `kind_filter`.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            DomainKnowledgeKind::WebsiteDomain { .. } => "website_domain",
            DomainKnowledgeKind::InteractionPrimitive { .. } => "interaction_primitive",
            DomainKnowledgeKind::TaskSOP { .. } => "task_sop",
        }
    }

    /// Free-text payload used for substring matching by the in-memory
    /// store. Production stores will replace this with embedding-based
    /// similarity.
    #[must_use]
    pub fn search_text(&self) -> String {
        match self {
            DomainKnowledgeKind::WebsiteDomain {
                domain, url_patterns, selectors, gotchas,
            } => {
                let mut s = domain.clone();
                s.push(' ');
                s.push_str(&url_patterns.join(" "));
                for sel in selectors {
                    s.push(' ');
                    s.push_str(&sel.selector);
                    s.push(' ');
                    s.push_str(&sel.purpose);
                }
                s.push(' ');
                s.push_str(&gotchas.join(" "));
                s
            }
            DomainKnowledgeKind::InteractionPrimitive {
                category,
                problem_statement,
                solution_code,
                tradeoffs,
            } => format!(
                "{category} {problem_statement} {solution_code} {}",
                tradeoffs.join(" ")
            ),
            DomainKnowledgeKind::TaskSOP {
                task_type,
                prerequisites,
                key_pitfalls,
                execution_steps,
            } => {
                let mut s = format!(
                    "{task_type} {} {}",
                    prerequisites.join(" "),
                    key_pitfalls.join(" ")
                );
                for step in execution_steps {
                    s.push(' ');
                    s.push_str(&step.tool_name);
                    s.push(' ');
                    s.push_str(&step.description);
                }
                s
            }
        }
    }
}

/// Provenance of a [`DomainKnowledgeEntry`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnowledgeAuthor {
    Agent { session_id: String },
    User,
    Imported { source: String },
}

/// One unit of stored domain knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainKnowledgeEntry {
    pub id: String,
    pub kind: DomainKnowledgeKind,
    pub created_by: KnowledgeAuthor,
    pub confidence: f64,
    pub access_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl DomainKnowledgeEntry {
    /// Fresh entry with id = uuid v4, access_count = 0, created_at = now.
    #[must_use]
    pub fn new(kind: DomainKnowledgeKind, created_by: KnowledgeAuthor, confidence: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            created_by,
            confidence,
            access_count: 0,
            last_used: None,
            created_at: Utc::now(),
        }
    }
}

/// Async storage abstraction. `lookup` MAY mutate per-entry stats
/// (access_count / last_used) — implementations decide whether to
/// surface that as I/O or in-memory bookkeeping.
#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    async fn upsert(&self, entry: DomainKnowledgeEntry);
    async fn lookup(
        &self,
        query: &str,
        kind_filter: Option<&str>,
    ) -> Vec<DomainKnowledgeEntry>;
}

/// Top-level lookup helper. Returned vec is empty for unknown
/// queries; callers decide whether to fall back to vector search
/// (FEAT-SE-004) or domain-knowledge contribution (FEAT-DK-003).
pub async fn lookup_domain_knowledge(
    query: &str,
    kind_filter: Option<&str>,
    store: &dyn KnowledgeStore,
) -> Vec<DomainKnowledgeEntry> {
    store.lookup(query, kind_filter).await
}

// ---------------------------------------------------------------------------
// Process-level KnowledgeStore singleton (DW-004 / WU-008 deeper-wiring).
// ---------------------------------------------------------------------------

use std::sync::OnceLock;

static GLOBAL_KNOWLEDGE_STORE: OnceLock<Arc<dyn KnowledgeStore>> = OnceLock::new();

/// Returns the process-wide [`KnowledgeStore`] singleton.
///
/// Initialised on first call with an in-memory [`mock::MockKnowledgeStore`]
/// that survives for the process lifetime.  DK contributor (`stream_finalize`)
/// upserts every extracted entry here; DK lookup (`work_loop`) reads from the
/// same instance — so knowledge gained in one turn is visible in the next.
///
/// A future deeper-wiring Pack can swap the backing impl (SQLite / LanceDB)
/// by calling `GLOBAL_KNOWLEDGE_STORE.set(...)` **before** the first
/// `global_knowledge_store()` call (typically in `setup.rs`).
pub fn global_knowledge_store() -> Arc<dyn KnowledgeStore> {
    Arc::clone(
        GLOBAL_KNOWLEDGE_STORE
            .get_or_init(|| Arc::new(mock::MockKnowledgeStore::new())),
    )
}

// ---------------------------------------------------------------------------
// Test-only in-memory backend (also reused by FEAT-DK-003 tests).
// ---------------------------------------------------------------------------

pub mod mock {
    //! In-memory `KnowledgeStore` impl. Substring-matches `search_text()`
    //! against the query; tracks access_count + last_used.

    use super::*;
    use std::sync::Mutex;

    pub struct MockKnowledgeStore {
        entries: Mutex<Vec<DomainKnowledgeEntry>>,
    }

    impl MockKnowledgeStore {
        #[must_use]
        pub fn new() -> Self {
            Self {
                entries: Mutex::new(Vec::new()),
            }
        }

        #[must_use]
        pub fn len(&self) -> usize {
            self.entries.lock().map(|g| g.len()).unwrap_or(0)
        }

        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    impl Default for MockKnowledgeStore {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl KnowledgeStore for MockKnowledgeStore {
        async fn upsert(&self, entry: DomainKnowledgeEntry) {
            let Ok(mut g) = self.entries.lock() else {
                return;
            };
            if let Some(existing) = g.iter_mut().find(|e| e.id == entry.id) {
                *existing = entry;
            } else {
                g.push(entry);
            }
        }

        async fn lookup(
            &self,
            query: &str,
            kind_filter: Option<&str>,
        ) -> Vec<DomainKnowledgeEntry> {
            let Ok(mut g) = self.entries.lock() else {
                return Vec::new();
            };
            let q = query.to_lowercase();
            let mut hits: Vec<DomainKnowledgeEntry> = Vec::new();
            for entry in g.iter_mut() {
                if let Some(label) = kind_filter {
                    if entry.kind.label() != label {
                        continue;
                    }
                }
                let body = entry.kind.search_text().to_lowercase();
                if q.is_empty() || body.contains(&q) {
                    entry.access_count = entry.access_count.saturating_add(1);
                    entry.last_used = Some(Utc::now());
                    hits.push(entry.clone());
                }
            }
            hits
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that `global_knowledge_store()` returns the *same* backing
    /// store on every call: an entry upserted through one handle is visible
    /// via a fresh handle obtained after the upsert.
    #[tokio::test]
    async fn global_store_shared_across_handles() {
        let entry = DomainKnowledgeEntry::new(
            DomainKnowledgeKind::TaskSOP {
                task_type: "test_task_singleton".to_string(),
                prerequisites: Vec::new(),
                key_pitfalls: Vec::new(),
                execution_steps: Vec::new(),
            },
            KnowledgeAuthor::User,
            1.0,
        );

        global_knowledge_store().upsert(entry.clone()).await;

        let hits = global_knowledge_store()
            .lookup("test_task_singleton", None)
            .await;
        assert!(
            hits.iter().any(|e| e.id == entry.id),
            "entry upserted via one handle must be visible through a second handle"
        );
    }
}

//! Memory contract v1 skeleton (Phase M0.3).
//!
//! Defines the canonical wire types for the `memory` entity (see
//! canonical domain model §3.6). The full `MemoryCoordinator` /
//! `WritePolicy` / `QualityGate` / `RecallAssembler` design lands in
//! M3; this module only fixes the public-facing types that frontend
//! projection and harness trace consumers will rely on.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::common::CorrelationIds;

/// Canonical memory kinds.
///
/// Closed set in v1. Adding a kind is a contract break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// Working memory active in the current run / turn.
    Working,
    /// Per-session rolling summary (`SessionSummary*` in code).
    SessionSummary,
    /// Episodic recall surfaced from prior sessions.
    Episodic,
    /// User-pinned memory item (`PinnedStore` in code).
    Pinned,
    /// Compiled / aggregated memory (`MemoryCompiler` output).
    Compiled,
    /// Reflection / self-model note produced by `LearningModule`.
    Reflection,
}

/// Canonical scope under which a memory item is stored / recalled.
///
/// Mirrors the existing three-tier model in
/// `modules::memory::scope::MemoryExecutionScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    /// Bound to a single `session`.
    Session,
    /// Bound to a `project` (multi-session).
    Project,
    /// Cross-project / app-global.
    Global,
}

/// Verdict produced by the (future) `WritePolicy` / `QualityGate`
/// chain when deciding whether a candidate memory should be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDecisionVerdict {
    /// Candidate accepted and persisted.
    Persisted,
    /// Candidate accepted but currently held in working memory only.
    HeldInWorking,
    /// Rejected by the threat scanner / quality gate.
    Rejected,
    /// Promoted up a scope (e.g. session -> project).
    Promoted,
    /// Demoted down a scope.
    Demoted,
    /// Marked for purge / expiry.
    Expired,
}

/// Wire-level decision record. Frontends render this as a
/// `MemoryWriteCard` / `MemoryChip` explainer.
//
// `Eq` intentionally omitted: `score: Option<f32>` does not satisfy
// `Eq` (f32 is only `PartialEq`). Compare with `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDecision {
    /// Stable id of the candidate memory item.
    pub memory_id: String,
    /// Decision verdict.
    pub verdict: MemoryDecisionVerdict,
    /// Kind of memory the decision applies to.
    pub kind: MemoryKind,
    /// Scope under which the decision was evaluated.
    pub scope: MemoryScope,
    /// Stable reason codes (e.g. `"pii_detected"`,
    /// `"low_quality_score"`, `"importance_above_threshold"`). Map
    /// to frontend i18n.
    #[serde(default)]
    pub reason_codes: Vec<String>,
    /// Optional quality / importance score (0.0..=1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    /// Stable policy version (`WritePolicy` / `QualityGate`).
    pub policy_version: String,
    /// Correlation ids.
    #[serde(default)]
    pub correlation: CorrelationIds,
    /// Decision timestamp, RFC3339.
    pub decided_at: String,
}

/// Canonical memory item projection consumed by the frontend.
///
/// `body_preview` deliberately holds only a short preview; the full
/// content is fetched via the `memory_*` IPC commands when the user
/// expands the item, so noisy stream traffic stays bounded.
//
// `Eq` intentionally omitted: `importance_score: Option<f32>` does
// not satisfy `Eq` (f32 is only `PartialEq`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryProjection {
    /// Stable id of the memory item.
    pub memory_id: String,
    /// Kind of memory.
    pub kind: MemoryKind,
    /// Scope holding this item.
    pub scope: MemoryScope,
    /// Short preview safe to render inline (truncated server-side).
    pub body_preview: String,
    /// Optional importance score (0.0..=1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance_score: Option<f32>,
    /// Origin tag (e.g. `"rolling_summary"`, `"user_pin"`,
    /// `"reflection"`, `"compiled_today"`). Open string to keep the
    /// skeleton flexible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Tags for filtering / grouping in the UI.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Created at, RFC3339.
    pub created_at: String,
    /// Last touched (recalled / promoted), RFC3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_touched_at: Option<String>,
    /// Correlation ids when projected as part of a recall event.
    #[serde(default)]
    pub correlation: CorrelationIds,
}

/// Lightweight per-recall memory item projection emitted on the
/// agent stream `stream_complete` event.
///
/// Distinct from [`MemoryProjection`] in that it carries only the
/// fields the chat-side `MemoryChip` needs (id, content, scope,
/// optional score / timestamp). The richer [`MemoryProjection`]
/// stays for the dedicated memory browser surface.
///
/// Defined here (instead of inside `application::memory_injection_service`)
/// so [`crate::modules::runtime::stream_emitter`] does not need to
/// reach back into the application layer — the runtime layer must
/// not depend on the application layer.
//
// `Eq` intentionally omitted: `relevance_score: Option<f32>` does
// not satisfy `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItemProjection {
    /// Stable id of the memory item.
    pub id: String,
    /// Item content as surfaced to the model and the UI.
    pub content: String,
    /// One of `"global" | "project" | "session"`.
    pub scope: String,
    /// Optional retrieval relevance score (0.0..=1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f32>,
    /// RFC3339 timestamp the entry was created at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stored_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_projection_round_trips() {
        let p = MemoryItemProjection {
            id: "m-3".into(),
            content: "hello".into(),
            scope: "session".into(),
            relevance_score: Some(0.7),
            stored_at: Some("2026-04-20T00:00:00+00:00".into()),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: MemoryItemProjection = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
        // Wire field names match the legacy
        // commands/agent.rs::MemoryContextItemPayload shape so the
        // frontend MemoryChip stays compatible.
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["id"], "m-3");
        assert_eq!(v["scope"], "session");
        assert!(v["relevance_score"].is_number());
    }

    #[test]
    fn projection_round_trips() {
        let p = MemoryProjection {
            memory_id: "m-1".into(),
            kind: MemoryKind::Pinned,
            scope: MemoryScope::Project,
            body_preview: "hello".into(),
            importance_score: Some(0.9),
            origin: Some("user_pin".into()),
            tags: vec!["important".into()],
            created_at: "2026-04-20T00:00:00Z".into(),
            last_touched_at: None,
            correlation: CorrelationIds::default(),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: MemoryProjection = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
        assert_eq!(back.kind, MemoryKind::Pinned);
        assert_eq!(back.scope, MemoryScope::Project);
    }

    #[test]
    fn decision_round_trips() {
        let d = MemoryDecision {
            memory_id: "m-2".into(),
            verdict: MemoryDecisionVerdict::Rejected,
            kind: MemoryKind::Working,
            scope: MemoryScope::Session,
            reason_codes: vec!["pii_detected".into()],
            score: Some(0.1),
            policy_version: "wp@2026-04-20".into(),
            correlation: CorrelationIds::default(),
            decided_at: "2026-04-20T00:00:00Z".into(),
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: MemoryDecision = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
    }
}

//! Public types for the pinned-memory subsystem (Phase 8A.9 / T-F1).
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-F1 + §0.5 Δ-17, pinned items are a small (≤ 50 per
//! scope, ≤ 500 chars each), hand-curated set of "always-on" memories
//! injected into the system prompt by 8A.11.

#![allow(dead_code)] // first production consumer lands in 8A.10 (pin/unpin tools)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Visibility scope of a pinned memory item.
///
/// Mirrors [`crate::modules::memory::scope::MemoryExecutionScope`] but
/// constrained to the two tiers a user can explicitly pin to — sessions
/// are intentionally excluded because they are too ephemeral for
/// "always-on" injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinScope {
    /// Visible only inside the current project.
    Project,
    /// Visible across every session and project.
    Global,
}

impl PinScope {
    /// Wire label used in SQLite / audit payloads.  Stable across releases.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }

    /// Reverse of [`Self::as_str`].  Lossy on purpose: any value that is
    /// not exactly `"global"` decays to [`Self::Project`] so legacy or
    /// hand-edited rows do not break the read path.
    pub(crate) fn from_str_lossy(s: &str) -> Self {
        match s {
            "global" => Self::Global,
            _ => Self::Project,
        }
    }
}

/// Origin of a pin: explicit user action, or implicit tool call.
///
/// The Telemetry Drawer + PinnedMemoryEditor UI render different chips
/// for each variant so users can always tell whether the agent or they
/// themselves added a given pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinSource {
    /// Pin came from a user clicking the "add" button in the future
    /// PinnedMemoryEditor UI (8A.12).
    User,
    /// Pin came from the agent invoking the `pin_memory` tool (8A.10).
    /// Carries the originating tool name + session id so the
    /// "agent-pinned this turn" badge in the TelemetryDrawer can link
    /// back to the conversation.
    Tool {
        /// Name of the tool that emitted the pin (e.g. `"pin_memory"`).
        tool_name: String,
        /// Session id from which the tool call originated.
        session_id: String,
    },
}

/// One pinned memory item, the wire / storage shape used by
/// [`crate::modules::memory::pinned::PinnedStore`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedItem {
    /// ULID — sortable + monotonic so the markdown sidecar renders in
    /// creation order without an explicit `row_order` column.
    pub id: String,
    /// User-facing markdown.  PII is scrubbed at write time
    /// (v2 §0.5 Δ-2) so this field is safe to render verbatim.
    pub content: String,
    /// Visibility tier.
    pub scope: PinScope,
    /// `Some(project_id)` when `scope == PinScope::Project`; `None`
    /// when `scope == PinScope::Global`.  Stored separately from
    /// `scope` for SQL filtering efficiency.
    pub project_id: Option<String>,
    /// Wall-clock UTC at first insertion (UPSERT preserves on dedup).
    pub created_at: DateTime<Utc>,
    /// Provenance of the pin.
    pub created_by: PinSource,
}

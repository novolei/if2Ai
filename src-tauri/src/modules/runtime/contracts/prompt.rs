//! Prompt diagnostics summary contract for frontend-facing projection.
//!
//! This contract intentionally exposes only a structured summary of
//! prompt assembly decisions, not raw prompt text. It is primarily
//! projected through `stream_complete` payloads so diagnostics UIs and
//! future prompt control panels can explain *why* a turn used certain
//! prompt layers without leaking the underlying prompt contents.

use serde::{Deserialize, Serialize};

/// Lightweight lane summary for prompt diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptDiagnosticsLaneSummary {
    /// Stable lane id (for example `identity`, `scenario`,
    /// `tool_policy`).
    pub lane: String,
    /// Lane activation state.
    pub status: String,
    /// Number of entries considered active for this lane.
    pub entry_count: usize,
}

/// One activated prompt entry projected to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptDiagnosticsActivatedEntry {
    pub entry_id: String,
    pub lane: String,
    pub source: String,
}

/// One suppressed prompt entry projected to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptDiagnosticsSuppressedEntry {
    pub entry_id: String,
    pub lane: String,
    pub reason_code: String,
}

/// One activation reason projected to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptDiagnosticsActivationReason {
    pub entry_id: String,
    pub lane: String,
    pub reason_code: String,
    pub detail: String,
}

/// Compact prompt diagnostics summary safe to emit to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptDiagnosticsSummary {
    /// Prompt plan trace id for explainability / trace correlation.
    pub trace_id: String,
    /// Number of prompt blocks assembled into the final plan.
    pub block_count: usize,
    /// Number of active lanes in `lane_summaries`.
    pub active_lane_count: usize,
    /// One summary row per known lane.
    pub lane_summaries: Vec<PromptDiagnosticsLaneSummary>,
    /// Entry ids the coordinator activated for this turn.
    pub activated_entry_ids: Vec<String>,
    /// Structured activated entry details for explainability.
    pub activated_entries: Vec<PromptDiagnosticsActivatedEntry>,
    /// Entry ids the coordinator considered but suppressed.
    pub suppressed_entry_ids: Vec<String>,
    /// Structured suppressed entry details for explainability.
    pub suppressed_entries: Vec<PromptDiagnosticsSuppressedEntry>,
    /// Stable activation reason codes (deduplicated, order-preserving).
    pub activation_reason_codes: Vec<String>,
    /// Structured activation reasons with details.
    pub activation_reasons: Vec<PromptDiagnosticsActivationReason>,
}

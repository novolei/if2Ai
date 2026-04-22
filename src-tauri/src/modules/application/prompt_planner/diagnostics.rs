//! Diagnostic types for prompt planning.
//!
//! MIG-007: Extracted from mod.rs for better modularity.

use super::block::PromptBlockKind;

/// Validation issue encountered during prompt plan construction.
///
/// Used to record warnings or errors without failing the build
/// (unless strict validation mode is enabled in future phases).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PromptValidationIssue {
    /// Machine-readable issue code (e.g., "forbidden_sensitive_external_block").
    pub code: String,
    /// Human-readable issue description.
    pub message: String,
}

/// Diagnostic metadata for a [`super::PromptPlan`].
///
/// Provides traceability and debugging information without exposing
/// sensitive prompt content. Used by harness traces and diagnostics UIs.
///
/// MIG-005: Added `validation_issues` field to record warnings/errors
/// encountered during plan construction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptPlanDiagnostics {
    /// Unique trace identifier for this plan.
    pub trace_id: String,
    /// Ordered list of block kinds in the plan.
    pub block_kinds: Vec<PromptBlockKind>,
    /// Total number of blocks.
    pub block_count: usize,
    /// Redacted preview of each block (first 48 chars or "[REDACTED]" for sensitive content).
    pub redacted_preview: Vec<String>,
    /// Validation issues encountered during plan construction.
    pub validation_issues: Vec<PromptValidationIssue>,
}

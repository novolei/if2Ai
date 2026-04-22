//! Diagnostic types for prompt planning.
//!
//! MIG-007: Extracted from mod.rs for better modularity.

use crate::modules::application::prompt_coordinator::{
    PromptActivationReason, PromptLaneDecision, SuppressedPromptEntry,
};
use crate::modules::runtime::contracts::{
    PromptDiagnosticsActivatedEntry, PromptDiagnosticsActivationReason,
    PromptDiagnosticsLaneSummary, PromptDiagnosticsSummary, PromptDiagnosticsSuppressedEntry,
};

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
    /// Lane-level prompt control-plane decisions when available.
    pub lane_decisions: Vec<PromptLaneDecision>,
    /// Activated prompt entry ids selected by the coordinator.
    pub activated_entry_ids: Vec<String>,
    /// Suppressed prompt entry ids considered by the coordinator.
    pub suppressed_entry_ids: Vec<String>,
    /// Suppressed prompt entries with reason codes.
    pub suppressed_entries: Vec<SuppressedPromptEntry>,
    /// Activation reasons emitted by the coordinator.
    pub activation_reasons: Vec<PromptActivationReason>,
}

impl PromptPlanDiagnostics {
    /// Build a frontend-safe prompt diagnostics summary.
    #[must_use]
    pub fn to_summary(&self) -> PromptDiagnosticsSummary {
        let lane_summaries: Vec<PromptDiagnosticsLaneSummary> = self
            .lane_decisions
            .iter()
            .map(|decision| PromptDiagnosticsLaneSummary {
                lane: lane_label(decision),
                status: lane_status_label(decision),
                entry_count: decision.entry_count,
            })
            .collect();
        let active_lane_count = lane_summaries
            .iter()
            .filter(|summary| summary.status == "active")
            .count();
        let activation_reason_codes =
            self.activation_reasons
                .iter()
                .fold(Vec::<String>::new(), |mut codes, reason| {
                    if !codes.iter().any(|code| code == &reason.reason_code) {
                        codes.push(reason.reason_code.clone());
                    }
                    codes
                });

        PromptDiagnosticsSummary {
            trace_id: self.trace_id.clone(),
            block_count: self.block_count,
            active_lane_count,
            lane_summaries,
            activated_entry_ids: self.activated_entry_ids.clone(),
            activated_entries: self.activation_reasons.iter().fold(
                Vec::<PromptDiagnosticsActivatedEntry>::new(),
                |mut entries, reason| {
                    if let Some(entry) = self
                        .activated_entry_ids
                        .iter()
                        .find(|entry_id| *entry_id == &reason.entry_id)
                    {
                        let exists = entries.iter().any(|candidate| candidate.entry_id == *entry);
                        if !exists {
                            let source = self
                                .activated_entry_source(entry)
                                .unwrap_or_else(|| "unknown".to_string());
                            entries.push(PromptDiagnosticsActivatedEntry {
                                entry_id: entry.clone(),
                                lane: lane_label_from_reason(reason),
                                source,
                            });
                        }
                    }
                    entries
                },
            ),
            suppressed_entry_ids: self.suppressed_entry_ids.clone(),
            suppressed_entries: self
                .suppressed_entries
                .iter()
                .map(|entry_id| PromptDiagnosticsSuppressedEntry {
                    entry_id: entry_id.entry_id.clone(),
                    lane: lane_label_from_suppressed(entry_id),
                    reason_code: entry_id.reason_code.clone(),
                })
                .collect(),
            activation_reason_codes,
            activation_reasons: self
                .activation_reasons
                .iter()
                .map(|reason| PromptDiagnosticsActivationReason {
                    entry_id: reason.entry_id.clone(),
                    lane: lane_label_from_reason(reason),
                    reason_code: reason.reason_code.clone(),
                    detail: reason.detail.clone(),
                })
                .collect(),
        }
    }

    fn activated_entry_source(&self, entry_id: &str) -> Option<String> {
        if self.activation_reasons.iter().any(|reason| {
            reason.entry_id == entry_id && reason.reason_code == "custom_identity_pack_applied"
        }) {
            return Some("custom_identity_pack".to_string());
        }
        self.activation_reasons.iter().find_map(|reason| {
            if reason.entry_id == entry_id {
                Some(match reason.reason_code.as_str() {
                    "resolved_identity_present" | "resolved_persona_present" => {
                        "resolved_identity".to_string()
                    }
                    "scenario_profile_hint_present" => "request_intelligence".to_string(),
                    "default_scenario_profile_applied" => "control_plane_defaults".to_string(),
                    "web_tool_family_available" => "tool_registry".to_string(),
                    "memory_injection_present" => "memory_coordinator".to_string(),
                    "complex_orchestration_required" => "execution_mode".to_string(),
                    _ => "control_plane".to_string(),
                })
            } else {
                None
            }
        })
    }
}

fn lane_label(decision: &PromptLaneDecision) -> String {
    match decision.lane {
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Identity => {
            "identity".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Scenario => {
            "scenario".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::ToolPolicy => {
            "tool_policy".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Memory => {
            "memory".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Utility => {
            "utility".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Coordinator => {
            "coordinator".to_string()
        }
    }
}

fn lane_status_label(decision: &PromptLaneDecision) -> String {
    match decision.status {
        crate::modules::application::prompt_coordinator::PromptLaneStatus::Active => {
            "active".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptLaneStatus::Suppressed => {
            "suppressed".to_string()
        }
    }
}

fn lane_label_from_reason(reason: &PromptActivationReason) -> String {
    match reason.lane {
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Identity => {
            "identity".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Scenario => {
            "scenario".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::ToolPolicy => {
            "tool_policy".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Memory => {
            "memory".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Utility => {
            "utility".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Coordinator => {
            "coordinator".to_string()
        }
    }
}

fn lane_label_from_suppressed(entry: &SuppressedPromptEntry) -> String {
    match entry.lane {
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Identity => {
            "identity".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Scenario => {
            "scenario".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::ToolPolicy => {
            "tool_policy".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Memory => {
            "memory".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Utility => {
            "utility".to_string()
        }
        crate::modules::application::prompt_coordinator::PromptAssemblyLane::Coordinator => {
            "coordinator".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::application::prompt_coordinator::{PromptAssemblyLane, PromptLaneStatus};

    #[test]
    fn prompt_diagnostics_summary_is_structured_and_deduplicated() {
        let diagnostics = PromptPlanDiagnostics {
            trace_id: "trace_abc".to_string(),
            block_kinds: vec![PromptBlockKind::System, PromptBlockKind::Scenario],
            block_count: 2,
            redacted_preview: vec!["System: [REDACTED]".to_string()],
            validation_issues: Vec::new(),
            lane_decisions: vec![
                PromptLaneDecision {
                    lane: PromptAssemblyLane::Identity,
                    status: PromptLaneStatus::Active,
                    entry_count: 2,
                },
                PromptLaneDecision {
                    lane: PromptAssemblyLane::Utility,
                    status: PromptLaneStatus::Suppressed,
                    entry_count: 0,
                },
            ],
            activated_entry_ids: vec!["identity:persona@staff-architect".to_string()],
            suppressed_entry_ids: vec!["utility:none".to_string()],
            suppressed_entries: vec![SuppressedPromptEntry {
                entry_id: "utility:none".to_string(),
                lane: PromptAssemblyLane::Utility,
                reason_code: "utility_lane_not_used_for_chat_turn".to_string(),
            }],
            activation_reasons: vec![
                PromptActivationReason {
                    entry_id: "identity:persona@staff-architect".to_string(),
                    lane: PromptAssemblyLane::Identity,
                    reason_code: "resolved_persona_present".to_string(),
                    detail: "persona present".to_string(),
                },
                PromptActivationReason {
                    entry_id: "identity:soul@if2ai-core".to_string(),
                    lane: PromptAssemblyLane::Identity,
                    reason_code: "resolved_identity_present".to_string(),
                    detail: "soul present".to_string(),
                },
                PromptActivationReason {
                    entry_id: "identity:persona@staff-architect".to_string(),
                    lane: PromptAssemblyLane::Identity,
                    reason_code: "resolved_persona_present".to_string(),
                    detail: "persona present again".to_string(),
                },
            ],
        };

        let summary = diagnostics.to_summary();

        assert_eq!(summary.trace_id, "trace_abc");
        assert_eq!(summary.block_count, 2);
        assert_eq!(summary.active_lane_count, 1);
        assert_eq!(summary.lane_summaries.len(), 2);
        assert_eq!(
            summary.activation_reason_codes,
            vec![
                "resolved_persona_present".to_string(),
                "resolved_identity_present".to_string(),
            ]
        );
        assert_eq!(summary.activated_entries.len(), 1);
        assert_eq!(summary.suppressed_entries.len(), 1);
        assert_eq!(summary.activation_reasons.len(), 3);
    }
}

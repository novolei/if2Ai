//! Prompt control plane coordinator foundation.
//!
//! This module is the first "policy brain" above `prompt_planner`.
//! It decides which prompt lanes are active for one turn, records
//! activation reasons for diagnostics, and returns normalized inputs
//! that the planner can assemble into a `PromptPlan`.

use crate::modules::identity::{
    persona_has_customization, read_identity_customization_pack, soul_has_customization,
    IdentityCustomizationPack, ResolvedIdentity,
};
use crate::modules::runtime::contracts::execution_mode::{
    ExecutionMode, ExecutionModeDecision, ScenarioProfileHint,
};

use super::prompt_planner::{PromptBuildMode, PromptContribution};

/// Stable prompt-assembly lanes owned by the control plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptAssemblyLane {
    Identity,
    Scenario,
    ToolPolicy,
    Memory,
    Utility,
    Coordinator,
}

impl PromptAssemblyLane {
    /// Closed set of lanes that FEAT-PCP-001 manages.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Identity,
            Self::Scenario,
            Self::ToolPolicy,
            Self::Memory,
            Self::Utility,
            Self::Coordinator,
        ]
    }
}

/// Lane-level status inside a prompt assembly decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptLaneStatus {
    Active,
    Suppressed,
}

/// One lane summary row inside the assembly decision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptLaneDecision {
    pub lane: PromptAssemblyLane,
    pub status: PromptLaneStatus,
    pub entry_count: usize,
}

/// One activated prompt entry selected by the coordinator.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActivatedPromptEntry {
    pub entry_id: String,
    pub lane: PromptAssemblyLane,
    pub source: String,
}

/// One suppressed prompt entry that was considered but not used.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SuppressedPromptEntry {
    pub entry_id: String,
    pub lane: PromptAssemblyLane,
    pub reason_code: String,
}

/// Why one prompt entry was activated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptActivationReason {
    pub entry_id: String,
    pub lane: PromptAssemblyLane,
    pub reason_code: String,
    pub detail: String,
}

/// Structured control-plane decision for one turn.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptAssemblyDecision {
    pub lane_decisions: Vec<PromptLaneDecision>,
    pub activated_entries: Vec<ActivatedPromptEntry>,
    pub suppressed_entries: Vec<SuppressedPromptEntry>,
    pub activation_reasons: Vec<PromptActivationReason>,
}

impl PromptAssemblyDecision {
    /// Return true when a lane is active in this decision.
    #[must_use]
    pub fn lane_is_active(&self, lane: PromptAssemblyLane) -> bool {
        self.lane_decisions
            .iter()
            .any(|decision| decision.lane == lane && decision.status == PromptLaneStatus::Active)
    }
}

/// Inputs consumed by the prompt coordinator for one turn.
#[derive(Debug, Clone)]
pub struct PromptCoordinatorRequest {
    pub resolved_identity: Option<ResolvedIdentity>,
    pub scenario_profile: Option<ScenarioProfileHint>,
    pub default_scenario_profile: Option<ScenarioProfileHint>,
    pub execution_mode_decision: Option<ExecutionModeDecision>,
    pub registered_tool_names: Vec<String>,
    pub memory_injection_present: bool,
    pub active_strategy_overlay_present: bool,
    pub active_skill_ids: Vec<String>,
}

/// Normalized prompt inputs returned by the coordinator.
#[derive(Debug, Clone)]
pub struct CoordinatedPromptInputs {
    pub mode: PromptBuildMode,
    pub resolved_identity: Option<ResolvedIdentity>,
    pub scenario_profile: Option<ScenarioProfileHint>,
    pub active_skill_ids: Vec<String>,
    pub external_contributions: Vec<PromptContribution>,
}

/// Coordinator output: a normalized planner input view plus the
/// explainable assembly decision.
#[derive(Debug, Clone)]
pub struct PromptCoordinatorOutput {
    pub coordinated_inputs: CoordinatedPromptInputs,
    pub decision: PromptAssemblyDecision,
}

/// First-cut prompt coordinator.
#[derive(Debug, Default, Clone, Copy)]
pub struct PromptCoordinator;

impl PromptCoordinator {
    /// Build a prompt assembly decision and normalized planner inputs.
    #[must_use]
    pub fn coordinate(&self, request: PromptCoordinatorRequest) -> PromptCoordinatorOutput {
        let mut activated_entries = Vec::new();
        let mut suppressed_entries = Vec::new();
        let mut activation_reasons = Vec::new();
        let mut lane_decisions = Vec::new();
        let customization_pack = read_identity_customization_pack().unwrap_or_else(|error| {
            tracing::warn!(
                "[prompt_coordinator] failed to load identity customization pack: {}; using built-in identity definitions",
                error
            );
            IdentityCustomizationPack::default()
        });

        let effective_scenario_profile = request
            .scenario_profile
            .or(request.default_scenario_profile);

        let mode = effective_scenario_profile
            .map(PromptBuildMode::from_scenario_hint)
            .unwrap_or_default();

        let identity_entry_count = usize::from(request.resolved_identity.is_some())
            + usize::from(
                request
                    .resolved_identity
                    .as_ref()
                    .and_then(|identity| identity.persona_id.as_ref())
                    .is_some(),
            );
        if let Some(ref identity) = request.resolved_identity {
            let soul_entry_id = format!("identity:soul@{}", identity.soul_id);
            activated_entries.push(ActivatedPromptEntry {
                entry_id: soul_entry_id.clone(),
                lane: PromptAssemblyLane::Identity,
                source: "resolved_identity".to_string(),
            });
            activation_reasons.push(PromptActivationReason {
                entry_id: soul_entry_id.clone(),
                lane: PromptAssemblyLane::Identity,
                reason_code: "resolved_identity_present".to_string(),
                detail: "resolved soul was available for this turn".to_string(),
            });
            if soul_has_customization(&customization_pack, &identity.soul_id) {
                activation_reasons.push(PromptActivationReason {
                    entry_id: soul_entry_id.clone(),
                    lane: PromptAssemblyLane::Identity,
                    reason_code: "custom_identity_pack_applied".to_string(),
                    detail:
                        "this soul uses user-defined overrides from ~/.if2ai/prompt/identity-pack.json"
                            .to_string(),
                });
            }
            if let Some(persona_id) = identity.persona_id.as_deref() {
                let persona_entry_id = format!("identity:persona@{persona_id}");
                activated_entries.push(ActivatedPromptEntry {
                    entry_id: persona_entry_id.clone(),
                    lane: PromptAssemblyLane::Identity,
                    source: "resolved_identity".to_string(),
                });
                activation_reasons.push(PromptActivationReason {
                    entry_id: persona_entry_id.clone(),
                    lane: PromptAssemblyLane::Identity,
                    reason_code: "resolved_persona_present".to_string(),
                    detail: "resolved persona was available for this turn".to_string(),
                });
                if persona_has_customization(&customization_pack, persona_id) {
                    activation_reasons.push(PromptActivationReason {
                        entry_id: persona_entry_id,
                        lane: PromptAssemblyLane::Identity,
                        reason_code: "custom_identity_pack_applied".to_string(),
                        detail:
                            "this persona uses user-defined overrides from ~/.if2ai/prompt/identity-pack.json"
                                .to_string(),
                    });
                }
            }
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Identity,
                status: PromptLaneStatus::Active,
                entry_count: identity_entry_count,
            });
        } else {
            suppressed_entries.push(SuppressedPromptEntry {
                entry_id: "identity:none".to_string(),
                lane: PromptAssemblyLane::Identity,
                reason_code: "no_resolved_identity".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Identity,
                status: PromptLaneStatus::Suppressed,
                entry_count: 0,
            });
        }

        if let Some(scenario_profile) = effective_scenario_profile {
            let entry_id = format!("scenario:{:?}", scenario_profile).to_lowercase();
            activated_entries.push(ActivatedPromptEntry {
                entry_id: entry_id.clone(),
                lane: PromptAssemblyLane::Scenario,
                source: if request.scenario_profile.is_some() {
                    "request_intelligence".to_string()
                } else {
                    "control_plane_defaults".to_string()
                },
            });
            activation_reasons.push(PromptActivationReason {
                entry_id,
                lane: PromptAssemblyLane::Scenario,
                reason_code: if request.scenario_profile.is_some() {
                    "scenario_profile_hint_present".to_string()
                } else {
                    "default_scenario_profile_applied".to_string()
                },
                detail: if request.scenario_profile.is_some() {
                    "request intelligence surfaced an advisory scenario profile".to_string()
                } else {
                    "control plane default scenario profile filled the empty scenario slot"
                        .to_string()
                },
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Scenario,
                status: PromptLaneStatus::Active,
                entry_count: 1,
            });
        } else {
            suppressed_entries.push(SuppressedPromptEntry {
                entry_id: "scenario:default_chat".to_string(),
                lane: PromptAssemblyLane::Scenario,
                reason_code: "no_scenario_profile_hint".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Scenario,
                status: PromptLaneStatus::Suppressed,
                entry_count: 0,
            });
        }

        let tool_policy_entries = build_tool_policy_catalog(&request.registered_tool_names);
        if tool_policy_entries.is_empty() {
            suppressed_entries.push(SuppressedPromptEntry {
                entry_id: "tool_policy:none".to_string(),
                lane: PromptAssemblyLane::ToolPolicy,
                reason_code: "no_supported_tool_family".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::ToolPolicy,
                status: PromptLaneStatus::Suppressed,
                entry_count: 0,
            });
        } else {
            for entry in &tool_policy_entries {
                activated_entries.push(ActivatedPromptEntry {
                    entry_id: entry.entry_id.clone(),
                    lane: PromptAssemblyLane::ToolPolicy,
                    source: "tool_registry".to_string(),
                });
                activation_reasons.push(PromptActivationReason {
                    entry_id: entry.entry_id.clone(),
                    lane: PromptAssemblyLane::ToolPolicy,
                    reason_code: entry.reason_code.clone(),
                    detail: entry.detail.clone(),
                });
            }
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::ToolPolicy,
                status: PromptLaneStatus::Active,
                entry_count: tool_policy_entries.len(),
            });
        }

        if request.memory_injection_present {
            let entry_id = "memory:injection".to_string();
            activated_entries.push(ActivatedPromptEntry {
                entry_id: entry_id.clone(),
                lane: PromptAssemblyLane::Memory,
                source: "memory_coordinator".to_string(),
            });
            activation_reasons.push(PromptActivationReason {
                entry_id,
                lane: PromptAssemblyLane::Memory,
                reason_code: "memory_injection_present".to_string(),
                detail: "memory coordinator returned prompt sections".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Memory,
                status: PromptLaneStatus::Active,
                entry_count: 1,
            });
        } else {
            suppressed_entries.push(SuppressedPromptEntry {
                entry_id: "memory:none".to_string(),
                lane: PromptAssemblyLane::Memory,
                reason_code: "no_memory_injection".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Memory,
                status: PromptLaneStatus::Suppressed,
                entry_count: 0,
            });
        }

        suppressed_entries.push(SuppressedPromptEntry {
            entry_id: "utility:default".to_string(),
            lane: PromptAssemblyLane::Utility,
            reason_code: "utility_lane_not_used_for_chat_turn".to_string(),
        });
        lane_decisions.push(PromptLaneDecision {
            lane: PromptAssemblyLane::Utility,
            status: PromptLaneStatus::Suppressed,
            entry_count: 0,
        });

        let coordinator_active = request.active_strategy_overlay_present
            || request
                .execution_mode_decision
                .as_ref()
                .is_some_and(|decision| {
                    matches!(
                        decision.execution_mode,
                        ExecutionMode::PlanThenConfirm | ExecutionMode::AutoPlanExecute
                    )
                });
        if coordinator_active {
            let entry_id = "coordinator:complex_turn".to_string();
            activated_entries.push(ActivatedPromptEntry {
                entry_id: entry_id.clone(),
                lane: PromptAssemblyLane::Coordinator,
                source: "execution_mode".to_string(),
            });
            activation_reasons.push(PromptActivationReason {
                entry_id,
                lane: PromptAssemblyLane::Coordinator,
                reason_code: "complex_orchestration_required".to_string(),
                detail: "execution mode or overlay indicates a multi-step turn".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Coordinator,
                status: PromptLaneStatus::Active,
                entry_count: 1,
            });
        } else {
            suppressed_entries.push(SuppressedPromptEntry {
                entry_id: "coordinator:none".to_string(),
                lane: PromptAssemblyLane::Coordinator,
                reason_code: "simple_turn_no_orchestration_overlay".to_string(),
            });
            lane_decisions.push(PromptLaneDecision {
                lane: PromptAssemblyLane::Coordinator,
                status: PromptLaneStatus::Suppressed,
                entry_count: 0,
            });
        }

        PromptCoordinatorOutput {
            coordinated_inputs: CoordinatedPromptInputs {
                mode,
                resolved_identity: request.resolved_identity,
                scenario_profile: effective_scenario_profile,
                active_skill_ids: request.active_skill_ids,
                external_contributions: Vec::new(),
            },
            decision: PromptAssemblyDecision {
                lane_decisions,
                activated_entries,
                suppressed_entries,
                activation_reasons,
            },
        }
    }
}

#[derive(Debug, Clone)]
struct ToolPolicyCatalogEntry {
    entry_id: String,
    reason_code: String,
    detail: String,
}

fn build_tool_policy_catalog(registered_tool_names: &[String]) -> Vec<ToolPolicyCatalogEntry> {
    let mut entries = Vec::new();

    if registered_tool_names
        .iter()
        .any(|name| name == "web_search")
    {
        entries.push(ToolPolicyCatalogEntry {
            entry_id: "tool_policy:web_search".to_string(),
            reason_code: "web_search_tool_available".to_string(),
            detail: "registered tools include web_search, so search attribution policy is active"
                .to_string(),
        });
    }

    if registered_tool_names
        .iter()
        .any(|name| matches!(name.as_str(), "web_fetch" | "web_research"))
    {
        entries.push(ToolPolicyCatalogEntry {
            entry_id: "tool_policy:web_fetch".to_string(),
            reason_code: "web_fetch_tool_available".to_string(),
            detail:
                "registered tools include fetch-style web access, so source-grounded retrieval policy is active"
                    .to_string(),
        });
    }

    if registered_tool_names.iter().any(|name| name == "browser") {
        entries.push(ToolPolicyCatalogEntry {
            entry_id: "tool_policy:browser".to_string(),
            reason_code: "browser_tool_available".to_string(),
            detail:
                "registered tools include browser automation, so higher-risk navigation policy is active"
                    .to_string(),
        });
    }

    if !entries.is_empty() {
        entries.push(ToolPolicyCatalogEntry {
            entry_id: "tool_policy:tool_result_hygiene".to_string(),
            reason_code: "tool_result_hygiene_enabled".to_string(),
            detail:
                "tool policy lane enforces source-aware interpretation of tool outputs and limits blind trust in tool results"
                    .to_string(),
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use crate::modules::runtime::contracts::execution_mode::{
        ComplexityLevel, RiskLevel, RouteHint,
    };

    use super::*;
    use crate::modules::identity::IdentitySource;

    fn sample_identity() -> ResolvedIdentity {
        ResolvedIdentity {
            soul_id: "if2ai-core".to_string(),
            soul_version: "1".to_string(),
            persona_id: Some("staff-architect".to_string()),
            persona_version: Some("1".to_string()),
            source: IdentitySource::GlobalDefault,
        }
    }

    fn sample_decision() -> ExecutionModeDecision {
        ExecutionModeDecision {
            execution_mode: ExecutionMode::PlanThenConfirm,
            risk_level: RiskLevel::Medium,
            complexity_level: ComplexityLevel::Moderate,
            complexity_score: 0.5,
            reason_codes: vec![],
            route_hint: Some(RouteHint("browser".to_string())),
            requires_plan: true,
            scenario_profile_hint: Some(ScenarioProfileHint::Planning),
            classifier_policy_version: "test".to_string(),
            classifier_matched_rule_ids: vec![],
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        }
    }

    #[test]
    fn coordinator_builds_assembly_decision() {
        let coordinator = PromptCoordinator;
        let output = coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: Some(sample_identity()),
            scenario_profile: Some(ScenarioProfileHint::Planning),
            default_scenario_profile: None,
            execution_mode_decision: Some(sample_decision()),
            registered_tool_names: vec!["web_search".to_string(), "web_fetch".to_string()],
            memory_injection_present: true,
            active_strategy_overlay_present: false,
            active_skill_ids: Vec::new(),
        });

        assert!(!output.decision.activated_entries.is_empty());
        assert!(!output.decision.activation_reasons.is_empty());
        assert_eq!(output.coordinated_inputs.mode, PromptBuildMode::Planning);
    }

    #[test]
    fn decision_contains_all_core_lanes() {
        let coordinator = PromptCoordinator;
        let output = coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: None,
            scenario_profile: None,
            default_scenario_profile: None,
            execution_mode_decision: None,
            registered_tool_names: Vec::new(),
            memory_injection_present: false,
            active_strategy_overlay_present: false,
            active_skill_ids: Vec::new(),
        });

        let lanes: Vec<PromptAssemblyLane> = output
            .decision
            .lane_decisions
            .iter()
            .map(|decision| decision.lane)
            .collect();
        assert_eq!(lanes.len(), PromptAssemblyLane::all().len());
        for lane in PromptAssemblyLane::all() {
            assert!(lanes.contains(&lane));
        }
    }

    #[test]
    fn decision_records_activation_reasons() {
        let coordinator = PromptCoordinator;
        let output = coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: Some(sample_identity()),
            scenario_profile: Some(ScenarioProfileHint::Research),
            default_scenario_profile: None,
            execution_mode_decision: Some(sample_decision()),
            registered_tool_names: vec!["browser".to_string()],
            memory_injection_present: true,
            active_strategy_overlay_present: true,
            active_skill_ids: Vec::new(),
        });

        assert!(output
            .decision
            .activation_reasons
            .iter()
            .any(|reason| reason.reason_code == "resolved_identity_present"));
        assert!(output
            .decision
            .activation_reasons
            .iter()
            .any(|reason| reason.reason_code == "scenario_profile_hint_present"));
        assert!(output
            .decision
            .activation_reasons
            .iter()
            .any(|reason| reason.reason_code == "complex_orchestration_required"));
    }

    #[test]
    fn coordinator_applies_default_scenario_profile() {
        let coordinator = PromptCoordinator;
        let output = coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: Some(sample_identity()),
            scenario_profile: None,
            default_scenario_profile: Some(ScenarioProfileHint::Review),
            execution_mode_decision: None,
            registered_tool_names: Vec::new(),
            memory_injection_present: false,
            active_strategy_overlay_present: false,
            active_skill_ids: Vec::new(),
        });

        assert_eq!(
            output.coordinated_inputs.scenario_profile,
            Some(ScenarioProfileHint::Review)
        );
        assert!(output
            .decision
            .activation_reasons
            .iter()
            .any(|reason| reason.reason_code == "default_scenario_profile_applied"));
    }

    #[test]
    fn coordinator_builds_granular_tool_policy_catalog() {
        let coordinator = PromptCoordinator;
        let output = coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: Some(sample_identity()),
            scenario_profile: Some(ScenarioProfileHint::Research),
            default_scenario_profile: None,
            execution_mode_decision: None,
            registered_tool_names: vec![
                "web_search".to_string(),
                "web_fetch".to_string(),
                "browser".to_string(),
            ],
            memory_injection_present: false,
            active_strategy_overlay_present: false,
            active_skill_ids: Vec::new(),
        });

        let tool_entries: Vec<&ActivatedPromptEntry> = output
            .decision
            .activated_entries
            .iter()
            .filter(|entry| entry.lane == PromptAssemblyLane::ToolPolicy)
            .collect();

        assert!(tool_entries
            .iter()
            .any(|entry| entry.entry_id == "tool_policy:web_search"));
        assert!(tool_entries
            .iter()
            .any(|entry| entry.entry_id == "tool_policy:web_fetch"));
        assert!(tool_entries
            .iter()
            .any(|entry| entry.entry_id == "tool_policy:browser"));
        assert!(tool_entries
            .iter()
            .any(|entry| entry.entry_id == "tool_policy:tool_result_hygiene"));
    }
}

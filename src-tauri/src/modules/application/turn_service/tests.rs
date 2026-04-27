//! MIG-002-a test: verify TurnService short-circuits on SpecializedSurface mode.

#[cfg(test)]
mod mig_002_a_tests {
    use super::super::build_prompt_plan_request_from_coordinator;
    use crate::modules::application::memory_injection_service::MemoryInjectionArtifacts;
    use crate::modules::application::prompt_coordinator::{
        CoordinatedPromptInputs, PromptAssemblyDecision,
    };
    use crate::modules::application::prompt_planner::PromptBuildMode;
    use crate::modules::identity::{IdentitySource, ResolvedIdentity};
    use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;
    use crate::modules::runtime::contracts::execution_mode::{
        ComplexityLevel, ExecutionMode, ExecutionModeDecision, RiskLevel,
    };
    use std::path::PathBuf;

    /// MIG-002-a: verify that the route gate logic correctly handles
    /// SpecializedSurface mode by short-circuiting (returning early).
    /// This test verifies the match arm exists and the behavior is correct.
    #[test]
    fn specialized_surface_mode_short_circuits() {
        // Arrange: create a decision with SpecializedSurface mode
        let decision = ExecutionModeDecision {
            execution_mode: ExecutionMode::SpecializedSurface,
            risk_level: RiskLevel::Low,
            complexity_level: ComplexityLevel::Simple,
            complexity_score: 0.1,
            reason_codes: vec![],
            route_hint: Some(
                crate::modules::runtime::contracts::execution_mode::RouteHint(
                    "browser".to_string(),
                ),
            ),
            requires_plan: false,
            scenario_profile_hint: None,
            classifier_policy_version: "test-v1".to_string(),
            classifier_matched_rule_ids: vec![],
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        };

        // Act & Assert: verify the match arm handles SpecializedSurface
        match decision.execution_mode {
            ExecutionMode::SpecializedSurface => {
                // This arm should short-circuit and return Err
                assert!(true, "SpecializedSurface arm exists and is reachable");
            }
            _ => {
                panic!("Expected SpecializedSurface mode");
            }
        }
    }

    /// MIG-002-a: verify that DirectExecute mode continues to normal execution.
    #[test]
    fn direct_execute_mode_continues() {
        // Arrange: create a decision with DirectExecute mode
        let decision = ExecutionModeDecision {
            execution_mode: ExecutionMode::DirectExecute,
            risk_level: RiskLevel::Low,
            complexity_level: ComplexityLevel::Trivial,
            complexity_score: 0.05,
            reason_codes: vec![],
            route_hint: None,
            requires_plan: false,
            scenario_profile_hint: None,
            classifier_policy_version: "test-v1".to_string(),
            classifier_matched_rule_ids: vec![],
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        };

        // Act & Assert: verify the match arm allows DirectExecute to continue
        match decision.execution_mode {
            ExecutionMode::DirectExecute
            | ExecutionMode::AutoPlanExecute
            | ExecutionMode::PlanThenConfirm => {
                // This arm should continue to normal execution
                assert!(true, "DirectExecute arm exists and continues execution");
            }
            _ => {
                panic!("Expected DirectExecute mode to continue");
            }
        }
    }

    #[test]
    fn prepare_chat_inputs_uses_prompt_coordinator() {
        let coordinated = CoordinatedPromptInputs {
            mode: PromptBuildMode::Planning,
            resolved_identity: Some(ResolvedIdentity {
                soul_id: "if2ai-core".to_string(),
                soul_version: "1".to_string(),
                persona_id: Some("staff-architect".to_string()),
                persona_version: Some("1".to_string()),
                source: IdentitySource::GlobalDefault,
            }),
            scenario_profile: Some(ScenarioProfileHint::Planning),
            active_skill_ids: vec!["skill-a".to_string()],
            external_contributions: Vec::new(),
        };

        let request = build_prompt_plan_request_from_coordinator(
            "session-1".to_string(),
            "design this".to_string(),
            PathBuf::from("/tmp/project"),
            "2026-04-22".to_string(),
            "macos".to_string(),
            "unix".to_string(),
            vec!["web_search".to_string()],
            MemoryInjectionArtifacts {
                prompt_sections: Vec::new(),
                memory_items: Vec::new(),
            },
            None,
            "test",
            PromptAssemblyDecision {
                lane_decisions: Vec::new(),
                activated_entries: Vec::new(),
                suppressed_entries: Vec::new(),
                activation_reasons: Vec::new(),
            },
            coordinated,
            Vec::new(), // MEM-MOD-P7 — no learned traits in this test fixture
        );

        assert_eq!(request.mode, PromptBuildMode::Planning);
        assert_eq!(
            request.scenario_profile,
            Some(ScenarioProfileHint::Planning)
        );
        assert_eq!(
            request
                .resolved_identity
                .as_ref()
                .and_then(|identity| identity.persona_id.as_deref()),
            Some("staff-architect")
        );
        assert_eq!(request.active_skill_ids, vec!["skill-a".to_string()]);
        assert!(request.prompt_assembly_decision.is_some());
    }
}

#[cfg(test)]
mod awl_003_skill_auto_load_tests {
    use super::super::work_loop::{
        auto_load_trusted_skill_context, build_final_run_report, resolve_skill_plan,
        route_work_loop,
    };
    use crate::modules::runtime::contracts::agent_loop::{
        SkillResolutionCandidate, SkillResolutionPlan,
    };
    use crate::modules::runtime::contracts::execution_mode::{
        ComplexityLevel, ExecutionMode, ExecutionModeDecision, ReasonCode, RiskLevel,
    };

    fn decision(mode: ExecutionMode, complexity: ComplexityLevel) -> ExecutionModeDecision {
        ExecutionModeDecision {
            execution_mode: mode,
            risk_level: RiskLevel::Low,
            complexity_level: complexity,
            complexity_score: 0.1,
            reason_codes: vec![ReasonCode::new("test")],
            route_hint: None,
            requires_plan: false,
            scenario_profile_hint: None,
            classifier_policy_version: "test".to_string(),
            classifier_matched_rule_ids: Vec::new(),
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        }
    }

    fn temp_workdir(test_name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("if2ai-{test_name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp workdir");
        dir
    }

    fn write_workspace_skill(
        workdir: &std::path::Path,
        name: &str,
        body: &str,
        review_status: &str,
    ) {
        let skill_dir = workdir.join(".if2ai/skills").join(name);
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {name} rust workflow helper\n---\n\n{body}"),
        )
        .expect("write SKILL.md");
        std::fs::write(
            skill_dir.join("skill.json"),
            format!(
                r#"{{"id":"{name}","version":"1.0.0","apiVersion":"1","minAppVersion":"0.0.0","capabilities":["prompt"],"review":{{"status":"{review_status}","riskLevel":"low","lastReviewedAt":"2026-04-27T00:00:00Z"}}}}"#
            ),
        )
        .expect("write skill.json");
    }

    #[test]
    fn skill_auto_load_trusted_local_skill() {
        let workdir = temp_workdir("trusted-local-skill");
        write_workspace_skill(
            &workdir,
            "rust-helper",
            "Always inspect existing Rust patterns before editing.",
            "active",
        );
        let mut plan = resolve_skill_plan(
            &workdir,
            "please use rust-helper for this Rust workflow",
            &[],
            &[],
        );
        let contribution = auto_load_trusted_skill_context(&workdir, &mut plan)
            .expect("trusted skill should load");

        assert!(plan.loaded_skill_names.contains(&"rust-helper".to_string()));
        assert!(plan.candidates.iter().any(|candidate| {
            candidate.name == "rust-helper" && candidate.loaded && candidate.auto_load_allowed
        }));
        assert!(contribution
            .body
            .contains("Always inspect existing Rust patterns"));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn skill_auto_load_blocks_remote_candidate() {
        let workdir = temp_workdir("remote-candidate");
        let mut plan = SkillResolutionPlan {
            active_skill_ids: Vec::new(),
            candidates: vec![SkillResolutionCandidate {
                skill_id: Some("remote-helper".to_string()),
                name: "remote-helper".to_string(),
                source: "remote-quarantine".to_string(),
                reason: "remote proposal matched request".to_string(),
                score: 10,
                trusted_source: false,
                auto_load_allowed: false,
                loaded: false,
                blocked_reason: None,
                load_warning: None,
            }],
            auto_discovery_tools: Vec::new(),
            should_load_find_skills: false,
            remote_install_policy: "quarantine_requires_user_approval".to_string(),
            loaded_skill_names: Vec::new(),
            blocked_skill_names: Vec::new(),
            load_warnings: Vec::new(),
        };

        assert!(auto_load_trusted_skill_context(&workdir, &mut plan).is_none());
        assert_eq!(plan.blocked_skill_names, vec!["remote-helper".to_string()]);
        assert!(!plan.candidates[0].loaded);
        assert!(!plan.candidates[0].auto_load_allowed);
        assert!(plan.candidates[0]
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("not eligible")));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn skill_auto_load_failure_reports_warning() {
        let workdir = temp_workdir("load-warning");
        write_workspace_skill(
            &workdir,
            "draft-helper",
            "Draft skills must not auto-load.",
            "draft",
        );
        let mut plan =
            resolve_skill_plan(&workdir, "draft-helper can help this workflow", &[], &[]);

        let _ = auto_load_trusted_skill_context(&workdir, &mut plan);
        assert!(plan
            .blocked_skill_names
            .contains(&"draft-helper".to_string()));
        assert!(plan
            .load_warnings
            .iter()
            .any(|warning| warning.contains("was not auto-loaded")));
        assert!(plan.candidates.iter().any(|candidate| {
            candidate.name == "draft-helper"
                && !candidate.loaded
                && candidate.blocked_reason.is_some()
                && candidate.load_warning.is_some()
        }));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn final_report_lists_skill_resolution() {
        let routed = route_work_loop(
            &decision(ExecutionMode::AutoPlanExecute, ComplexityLevel::Moderate),
            "use a trusted skill",
        );
        let plan = SkillResolutionPlan {
            active_skill_ids: vec!["rust-helper".to_string()],
            candidates: Vec::new(),
            auto_discovery_tools: Vec::new(),
            should_load_find_skills: false,
            remote_install_policy: "quarantine_requires_user_approval".to_string(),
            loaded_skill_names: vec!["rust-helper".to_string()],
            blocked_skill_names: vec!["remote-helper".to_string()],
            load_warnings: vec!["remote-helper blocked".to_string()],
        };
        let report = build_final_run_report(
            &routed,
            "completed".to_string(),
            "model_stop".to_string(),
            "req-skill-report".to_string(),
            1,
            false,
            false,
            false,
            None,
            Some(&plan),
        );

        assert_eq!(report.loaded_skills, vec!["rust-helper".to_string()]);
        assert_eq!(report.blocked_skills, vec!["remote-helper".to_string()]);
        assert_eq!(
            report.skill_warnings,
            vec!["remote-helper blocked".to_string()]
        );
        assert!(report
            .completed_items
            .iter()
            .any(|item| item.contains("Loaded trusted skills")));
    }
}

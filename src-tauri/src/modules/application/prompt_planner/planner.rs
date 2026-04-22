//! Core prompt planning logic.
//!
//! MIG-007: Extracted from mod.rs for better modularity.

use super::block::{PromptBlock, PromptBlockKind, PromptBlockSource, PromptContribution};
use super::build_request::BuildPromptPlanRequest;
use super::diagnostics::{PromptPlanDiagnostics, PromptValidationIssue};
use super::merge::merge_external_contributions;
use crate::modules::application::memory_injection_service::MemoryInjectionSectionKind;
use crate::modules::identity::{render_persona_block, render_soul_block, IdentityRegistry};
use crate::modules::runtime::prompt::{load_system_prompt, PromptBuildError, SystemPromptBuilder};
use crate::modules::runtime::prompt_tools_guide::web_tools_routing_block;

/// Ordered collection of [`PromptBlock`]s representing the static
/// prompt for one turn.
///
/// Use [`PromptPlan::join_into_text`] to render the canonical newline
/// separator (single `"\n"`) used by the legacy assembly path.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptPlan {
    /// Unique trace identifier computed from session context and block hash.
    pub trace_id: String,
    /// SHA256 hash of all block content for plan identity.
    pub block_hash: String,
    /// Diagnostic metadata for traceability.
    pub diagnostics: PromptPlanDiagnostics,
    pub blocks: Vec<PromptBlock>,
}

impl PromptPlan {
    /// Render the plan into a single string with the legacy
    /// `"\n"` separator (matches the previous `Vec<String>::join`
    /// in `commands/agent.rs`).
    #[must_use]
    pub fn join_into_text(&self) -> String {
        let parts: Vec<&str> = self.blocks.iter().map(|b| b.content.as_str()).collect();
        parts.join("\n")
    }

    /// Number of blocks. Useful for harness trace summaries.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Output of [`build_prompt_plan`] — both the structured plan and
/// its rendered text, so callers do not have to render twice.
pub struct PromptPlanResult {
    pub plan: PromptPlan,
    pub text: String,
}

/// Errors surfaced by the planner.
#[derive(Debug, thiserror::Error)]
pub enum PromptPlannerError {
    #[error("failed to build system prompt: {0}")]
    Build(#[from] PromptBuildError),
}

/// Build the chat-turn prompt plan.
///
/// MIG-006: Now accepts external_contributions parameter for subsystem
/// contributions (Memory, MCP, Skills, Learning).
///
/// Block order (matches legacy `commands/agent.rs` assembly):
///
/// 1. `System` — `load_system_prompt` lines joined by `"\n"`.
/// 2. `Soul` — optional, from resolved_identity.
/// 3. `Persona` — optional, from resolved_identity.
/// 4. `Scenario` — optional, from scenario_profile or mode.
/// 5. `WebToolsRoutingGuide` — only when at least two web tools are
///    registered.
/// 6. Memory injection sections in the order produced by
///    [`crate::modules::application::memory_injection_service::prepare_memory_injection`]:
///    Pinned → Compiled → Rules → Retrieved (any subset may be
///    absent).
/// 7. `Skill` — optional, from active_skill_ids.
/// 8. External contributions merged by kind.
///
/// On `load_system_prompt` failure the planner falls back to the
/// minimal `SystemPromptBuilder::new().render()` output, identical
/// to the legacy fallback path.
pub async fn build_prompt_plan(
    request: BuildPromptPlanRequest,
    external_contributions: Vec<PromptContribution>,
) -> Result<PromptPlanResult, PromptPlannerError> {
    let mut blocks: Vec<PromptBlock> = Vec::new();

    // 1. System prompt — preserve the legacy fallback shape.
    let system_lines = match load_system_prompt(
        request.workdir.clone(),
        request.current_date.clone(),
        request.os_name.clone(),
        request.os_family.clone(),
    ) {
        Ok(lines) => lines,
        Err(e) => {
            tracing::warn!(
                caller = request.caller,
                "[prompt_planner] Failed to build system prompt: {}, using fallback",
                e
            );
            vec![SystemPromptBuilder::new().render()]
        }
    };
    blocks.push(PromptBlock {
        id: "system".to_string(),
        kind: PromptBlockKind::System,
        title: "system".to_string(),
        content: system_lines.join("\n"),
        source: PromptBlockSource {
            subsystem: "system_prompt".to_string(),
            reference: None,
        },
        priority: 100,
        is_sensitive: true,
    });

    // 1b. Identity blocks stay outside the base system prompt so they
    // remain observable, hashable, and independently evolvable.
    if let Some(ref resolved_identity) = request.resolved_identity {
        let registry = IdentityRegistry::builtin();
        if let Some(soul) = registry.soul(&resolved_identity.soul_id) {
            blocks.push(PromptBlock {
                id: "soul".to_string(),
                kind: PromptBlockKind::Soul,
                title: "soul".to_string(),
                content: render_soul_block(soul),
                source: PromptBlockSource {
                    subsystem: "identity".to_string(),
                    reference: Some(soul.id.clone()),
                },
                priority: 95,
                is_sensitive: true,
            });
        }
        if let Some(persona_id) = resolved_identity.persona_id.as_deref() {
            if let Some(persona) = registry.persona(persona_id) {
                blocks.push(PromptBlock {
                    id: "persona".to_string(),
                    kind: PromptBlockKind::Persona,
                    title: "persona".to_string(),
                    content: render_persona_block(persona),
                    source: PromptBlockSource {
                        subsystem: "identity".to_string(),
                        reference: Some(persona.id.clone()),
                    },
                    priority: 94,
                    is_sensitive: true,
                });
            }
        }
    }

    let scenario_mode = request
        .scenario_profile
        .map(super::build_request::PromptBuildMode::from_scenario_hint)
        .or_else(|| {
            (request.mode != super::build_request::PromptBuildMode::default())
                .then_some(request.mode)
        });
    if let Some(mode) = scenario_mode {
        blocks.push(PromptBlock {
            id: "scenario".to_string(),
            kind: PromptBlockKind::Scenario,
            title: "scenario".to_string(),
            content: render_scenario_block(mode),
            source: PromptBlockSource {
                subsystem: "scenario_profile".to_string(),
                reference: Some(format!("{mode:?}").to_lowercase()),
            },
            priority: 92,
            is_sensitive: false,
        });
    }

    // 2. Web-tool routing guide (Phase 7C, slice 7C.4 parity).
    if let Some(guide) = web_tools_routing_block(&request.registered_tool_names) {
        blocks.push(PromptBlock {
            id: "web_tools_routing_guide".to_string(),
            kind: PromptBlockKind::WebToolsRoutingGuide,
            title: "web_tools_routing_guide".to_string(),
            content: guide,
            source: PromptBlockSource {
                subsystem: "tool_routing".to_string(),
                reference: None,
            },
            priority: 90,
            is_sensitive: false,
        });
    }

    // 2b. Phase M5 closeout — active-strategy overlay block.
    // The active strategy registry resolves to zero or one
    // overlay text (singleton-active enforced by the rollout
    // service); empty string means no active strategy with a
    // runtime effect.
    if let Some(overlay) = request.active_strategy_overlay {
        if !overlay.trim().is_empty() {
            blocks.push(PromptBlock {
                id: "active_strategy_overlay".to_string(),
                kind: PromptBlockKind::ActiveStrategyOverlay,
                title: "active_strategy_overlay".to_string(),
                content: overlay,
                source: PromptBlockSource {
                    subsystem: "learning".to_string(),
                    reference: None,
                },
                priority: 85,
                is_sensitive: false,
            });
        }
    }

    // 3. Memory injection blocks (Pinned / Compiled / Rules /
    //    Retrieved) in the order produced by the memory service.
    if let Some(artifacts) = request.memory_injection {
        let mut memory_block_counters: std::collections::HashMap<
            MemoryInjectionSectionKind,
            usize,
        > = std::collections::HashMap::new();
        for section in artifacts.prompt_sections {
            let counter = memory_block_counters.entry(section.kind).or_insert(0);
            let block_id = format!("{}-{}", title_for_memory_section(section.kind), counter);
            let title = title_for_memory_section(section.kind);
            let kind = PromptBlockKind::from_memory_section(section.kind);

            // MIG-005: Assign priority and is_sensitive based on memory section kind
            let (priority, is_sensitive) = match section.kind {
                MemoryInjectionSectionKind::Pinned => (80, false),
                MemoryInjectionSectionKind::Compiled => (70, false),
                MemoryInjectionSectionKind::Rules => (60, false),
                MemoryInjectionSectionKind::Retrieved => (50, true),
            };

            *counter += 1;
            blocks.push(PromptBlock {
                id: block_id,
                kind,
                title: title.to_string(),
                content: section.content,
                source: PromptBlockSource {
                    subsystem: "memory".to_string(),
                    reference: None,
                },
                priority,
                is_sensitive,
            });
        }
    }

    // 3b. MIG-006: Skill block (optional).
    if !request.active_skill_ids.is_empty() {
        blocks.push(PromptBlock {
            id: "skill".to_string(),
            kind: PromptBlockKind::Skill,
            title: "skill".to_string(),
            content: request.active_skill_ids.join(", "),
            source: PromptBlockSource {
                subsystem: "skill_registry".to_string(),
                reference: None,
            },
            priority: 40,
            is_sensitive: false,
        });
    }

    // 4. MIG-006: Merge external contributions.
    let (blocks, validation_issues) = merge_external_contributions(
        blocks,
        external_contributions,
        request.options.strict_block_validation,
    )?;

    // Compute trace metadata
    let block_hash = compute_block_hash(&blocks);
    let trace_id = compute_trace_id(&request.session_id, &request.user_message, &block_hash);
    let diagnostics = build_diagnostics(trace_id.clone(), &blocks, validation_issues);

    let plan = PromptPlan {
        trace_id,
        block_hash,
        diagnostics,
        blocks,
    };
    let text = plan.join_into_text();
    Ok(PromptPlanResult { plan, text })
}

fn title_for_memory_section(kind: MemoryInjectionSectionKind) -> &'static str {
    match kind {
        MemoryInjectionSectionKind::Pinned => "memory_pinned",
        MemoryInjectionSectionKind::Compiled => "memory_compiled",
        MemoryInjectionSectionKind::Rules => "memory_rules",
        MemoryInjectionSectionKind::Retrieved => "retrieved_memory",
    }
}

fn render_scenario_block(mode: super::build_request::PromptBuildMode) -> String {
    let (name, summary, rules): (&str, &str, &[&str]) = match mode {
        super::build_request::PromptBuildMode::Chat => (
            "Chat",
            "General-purpose interactive assistance across mixed user intents.",
            &[
                "Optimize for clarity, continuity, and practical forward motion.",
                "Stay conversational, but become structured when the user needs a decision or plan.",
            ],
        ),
        super::build_request::PromptBuildMode::Coding => (
            "Coding",
            "Execution-heavy software engineering work inside a live workspace.",
            &[
                "Prefer repository-grounded decisions over generic best-practice recitation.",
                "Read before writing, keep changes scoped, and verify with concrete checks.",
            ],
        ),
        super::build_request::PromptBuildMode::Research => (
            "Research",
            "Evidence-gathering, comparison, and synthesis across sources.",
            &[
                "Separate observed facts from inference and preserve source traceability.",
                "Prefer breadth first, then converge on the most decision-relevant evidence.",
            ],
        ),
        super::build_request::PromptBuildMode::Planning => (
            "Planning",
            "Architecture, sequencing, and rollout-oriented design work.",
            &[
                "Surface alternatives and tradeoffs before locking in a recommendation.",
                "Name dependencies, rollout order, and hidden risks explicitly.",
            ],
        ),
        super::build_request::PromptBuildMode::Review => (
            "Review",
            "Critical evaluation of correctness, regressions, and quality risks.",
            &[
                "Lead with concrete findings, not general summaries.",
                "Prioritize behavioral regressions, missing tests, and hidden edge cases.",
            ],
        ),
    };

    let mut lines = vec![
        format!("# Scenario Profile: {name}"),
        format!("- Summary: {summary}"),
        String::new(),
        "## Operating Rules".to_string(),
    ];
    lines.extend(rules.iter().map(|item| format!("- {item}")));
    lines.join("\n")
}

/// Compute SHA256 hash of all blocks for plan identity.
/// MIG-005: Include source.subsystem in hash computation for stability.
fn compute_block_hash(blocks: &[PromptBlock]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for block in blocks {
        hasher.update(format!(
            "{:?}|{}|{}|{}\n",
            block.kind, block.title, block.source.subsystem, block.content
        ));
    }
    hex::encode(hasher.finalize())
}

/// Compute unique trace ID from session context and block hash.
fn compute_trace_id(session_id: &str, user_message: &str, block_hash: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(user_message.as_bytes());
    hasher.update(block_hash.as_bytes());
    let hex_hash = hex::encode(hasher.finalize());
    format!("trace_{}", &hex_hash[..16])
}

/// Build diagnostic metadata for the plan.
/// MIG-005: Use block.is_sensitive instead of kind matching for redaction.
/// MIG-006: Accept validation_issues parameter.
fn build_diagnostics(
    trace_id: String,
    blocks: &[PromptBlock],
    validation_issues: Vec<PromptValidationIssue>,
) -> PromptPlanDiagnostics {
    let redacted_preview = blocks
        .iter()
        .map(|b| {
            if b.is_sensitive {
                format!(
                    "{:?}: [REDACTED {} chars]",
                    b.kind,
                    b.content.chars().count()
                )
            } else {
                let preview: String = b.content.chars().take(48).collect();
                format!("{:?}: {}", b.kind, preview)
            }
        })
        .collect();

    PromptPlanDiagnostics {
        trace_id: trace_id.clone(),
        block_kinds: blocks.iter().map(|b| b.kind).collect(),
        block_count: blocks.len(),
        redacted_preview,
        validation_issues,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::modules::application::memory_injection_service::{
        MemoryInjectionArtifacts, MemoryInjectionSection,
    };
    use crate::modules::identity::{IdentitySource, ResolvedIdentity};
    use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;

    #[test]
    fn join_into_text_uses_legacy_newline_separator() {
        let plan = PromptPlan {
            trace_id: "trace_test123".to_string(),
            block_hash: "hash123".to_string(),
            diagnostics: PromptPlanDiagnostics {
                trace_id: "trace_test123".to_string(),
                block_kinds: vec![PromptBlockKind::System, PromptBlockKind::RetrievedMemory],
                block_count: 2,
                redacted_preview: vec![],
                validation_issues: vec![],
            },
            blocks: vec![
                PromptBlock {
                    id: "system".to_string(),
                    kind: PromptBlockKind::System,
                    title: "system".to_string(),
                    content: "a".into(),
                    source: PromptBlockSource {
                        subsystem: "system_prompt".to_string(),
                        reference: None,
                    },
                    priority: 100,
                    is_sensitive: true,
                },
                PromptBlock {
                    id: "retrieved_memory-0".to_string(),
                    kind: PromptBlockKind::RetrievedMemory,
                    title: "retrieved_memory".to_string(),
                    content: "b".into(),
                    source: PromptBlockSource {
                        subsystem: "memory".to_string(),
                        reference: None,
                    },
                    priority: 50,
                    is_sensitive: true,
                },
            ],
        };
        assert_eq!(plan.join_into_text(), "a\nb");
        assert_eq!(plan.block_count(), 2);
    }

    #[test]
    fn compute_block_hash_is_stable() {
        let blocks = vec![PromptBlock {
            id: "system".to_string(),
            kind: PromptBlockKind::System,
            title: "system".to_string(),
            content: "test content".into(),
            source: PromptBlockSource {
                subsystem: "system_prompt".to_string(),
                reference: None,
            },
            priority: 100,
            is_sensitive: true,
        }];
        let hash1 = compute_block_hash(&blocks);
        let hash2 = compute_block_hash(&blocks);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[tokio::test]
    async fn memory_section_kinds_map_to_canonical_block_kinds() {
        let artifacts = MemoryInjectionArtifacts {
            prompt_sections: vec![
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Pinned,
                    content: "P".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Compiled,
                    content: "C".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Rules,
                    content: "R".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Retrieved,
                    content: "X".into(),
                },
            ],
            memory_items: Vec::new(),
        };
        let req = BuildPromptPlanRequest {
            session_id: "test_session".to_string(),
            user_message: "test message".to_string(),
            workdir: PathBuf::from("/nonexistent/path/for/planner-tests"),
            current_date: "2026-04-20".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: Some(artifacts),
            active_strategy_overlay: None,
            caller: "prompt_planner_test",
            mode: super::super::PromptBuildMode::default(),
            resolved_identity: None,
            scenario_profile: None,
            active_skill_ids: Vec::new(),
            options: super::super::PromptBuildOptions::default(),
        };
        let result = build_prompt_plan(req, Vec::new())
            .await
            .expect("plan builds");
        assert_eq!(result.plan.blocks[0].kind, PromptBlockKind::System);
        let memory_kinds: Vec<PromptBlockKind> =
            result.plan.blocks.iter().skip(1).map(|b| b.kind).collect();
        assert_eq!(
            memory_kinds,
            vec![
                PromptBlockKind::MemoryInjectionPinned,
                PromptBlockKind::MemoryInjectionCompiled,
                PromptBlockKind::MemoryInjectionRules,
                PromptBlockKind::RetrievedMemory,
            ]
        );
    }

    #[tokio::test]
    async fn identity_and_scenario_blocks_are_emitted() {
        let req = BuildPromptPlanRequest {
            session_id: "test".to_string(),
            user_message: "test".to_string(),
            workdir: PathBuf::from("/nonexistent"),
            current_date: "2026-04-22".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: None,
            active_strategy_overlay: None,
            caller: "test",
            mode: super::super::PromptBuildMode::Planning,
            resolved_identity: Some(ResolvedIdentity {
                soul_id: "if2ai-core".to_string(),
                soul_version: "1".to_string(),
                persona_id: Some("staff-architect".to_string()),
                persona_version: Some("1".to_string()),
                source: IdentitySource::GlobalDefault,
            }),
            scenario_profile: Some(ScenarioProfileHint::Planning),
            active_skill_ids: vec!["skill1".to_string()],
            options: super::super::PromptBuildOptions::default(),
        };
        let result = build_prompt_plan(req, Vec::new())
            .await
            .expect("plan builds");
        assert!(result
            .plan
            .blocks
            .iter()
            .any(|b| b.kind == PromptBlockKind::Soul));
        assert!(result
            .plan
            .blocks
            .iter()
            .any(|b| b.kind == PromptBlockKind::Persona));
        assert!(result
            .plan
            .blocks
            .iter()
            .any(|b| b.kind == PromptBlockKind::Scenario));
        assert!(result
            .plan
            .blocks
            .iter()
            .any(|b| b.kind == PromptBlockKind::Skill));
    }
}

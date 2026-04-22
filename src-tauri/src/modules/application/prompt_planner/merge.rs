//! MIG-006: External contributions merging logic.
//!
//! Allows subsystems (Memory, MCP, Skills, Learning) to contribute
//! prompt blocks without modifying the planner core.

use super::planner::PromptPlannerError;
use super::{PromptBlock, PromptBlockKind, PromptContribution, PromptValidationIssue};
use crate::modules::runtime::prompt::PromptBuildError;

/// Merge external contributions into core blocks.
///
/// MIG-006: Implements the external contribution mechanism from UClaw.
///
/// Rules:
/// - Forbids external contributions of System/Persona kinds (sensitive core blocks)
/// - In strict mode, validation failures cause immediate error
/// - In non-strict mode, validation failures are recorded as issues
/// - External blocks are inserted after their corresponding core block kind
/// - All blocks maintain their original order within their kind group
///
/// Returns: (merged_blocks, validation_issues)
pub(super) fn merge_external_contributions(
    core_blocks: Vec<PromptBlock>,
    external: Vec<PromptContribution>,
    strict_mode: bool,
) -> Result<(Vec<PromptBlock>, Vec<PromptValidationIssue>), PromptPlannerError> {
    let mut issues: Vec<PromptValidationIssue> = Vec::new();
    let mut accepted: Vec<PromptContribution> = Vec::new();

    // Validate external contributions
    for ext in external {
        let forbidden_sensitive = matches!(
            ext.kind,
            PromptBlockKind::System
                | PromptBlockKind::Soul
                | PromptBlockKind::Persona
                | PromptBlockKind::Scenario
        );
        if forbidden_sensitive {
            let issue = PromptValidationIssue {
                code: "forbidden_sensitive_external_block".to_string(),
                message: format!(
                    "external contribution of kind {:?} is forbidden (sensitive core block)",
                    ext.kind
                ),
            };
            if strict_mode {
                // In strict mode, return an error via PromptPlannerError directly
                return Err(PromptPlannerError::Build(PromptBuildError::Io(
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, issue.message.clone()),
                )));
            }
            issues.push(issue);
            continue;
        }
        accepted.push(ext);
    }

    // Merge accepted contributions by kind
    let mut out = Vec::new();
    let mut consumed = vec![false; accepted.len()];

    for core_block in core_blocks {
        let kind = core_block.kind;
        out.push(core_block);

        // Insert external contributions of the same kind after the core block
        for (idx, ext) in accepted.iter().enumerate() {
            if consumed[idx] || ext.kind != kind {
                continue;
            }
            out.push(PromptBlock {
                id: format!("ext-{}-{}", kind_slug(ext.kind), idx),
                kind: ext.kind,
                title: ext.title.clone(),
                content: ext.body.clone(),
                source: ext.source.clone(),
                priority: 10,        // External contributions have lower priority
                is_sensitive: false, // External contributions are non-sensitive by validation
            });
            consumed[idx] = true;
        }
    }

    // Append any remaining external contributions that didn't match a core kind
    for (idx, ext) in accepted.into_iter().enumerate() {
        if consumed[idx] {
            continue;
        }
        out.push(PromptBlock {
            id: format!("ext-{}-{}", kind_slug(ext.kind), idx),
            kind: ext.kind,
            title: ext.title,
            content: ext.body,
            source: ext.source,
            priority: 10,
            is_sensitive: false,
        });
    }

    Ok((out, issues))
}

fn kind_slug(kind: PromptBlockKind) -> &'static str {
    match kind {
        PromptBlockKind::System => "system",
        PromptBlockKind::Soul => "soul",
        PromptBlockKind::Persona => "persona",
        PromptBlockKind::Scenario => "scenario",
        PromptBlockKind::WebToolsRoutingGuide => "web_tools",
        PromptBlockKind::MemoryInjectionPinned => "memory_pinned",
        PromptBlockKind::MemoryInjectionCompiled => "memory_compiled",
        PromptBlockKind::MemoryInjectionRules => "memory_rules",
        PromptBlockKind::RetrievedMemory => "retrieved_memory",
        PromptBlockKind::ActiveStrategyOverlay => "active_strategy",
        PromptBlockKind::Skill => "skill",
        PromptBlockKind::Mcp => "mcp",
    }
}

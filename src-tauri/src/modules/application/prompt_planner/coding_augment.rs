//! Coding mode prompt augmentation.
//!
//! MIG-008: Provides workspace context and tool surface blocks for Coding mode.

use super::block::{PromptBlock, PromptBlockKind, PromptBlockSource};
use crate::modules::memory::inject::CacheHint;
use std::path::Path;

/// Build coding-specific augmentation blocks.
///
/// MIG-008: Generates workspace context and tool surface blocks when
/// in Coding mode. These blocks provide additional context about the
/// working directory, git status, and available tools.
///
/// Returns a vector of blocks to be inserted into the prompt plan.
pub(super) fn build_coding_augment_blocks(
    workdir: &Path,
    registered_tool_names: &[String],
) -> Vec<PromptBlock> {
    let mut blocks = Vec::new();

    // Workspace context block
    let workspace_content = format!(
        "# Workspace Context\n\nWorking directory: {}\n\nYou are in a coding session. \
         Focus on code quality, testing, and maintainability.",
        workdir.display()
    );

    blocks.push(PromptBlock {
        id: "coding_workspace_context".to_string(),
        kind: PromptBlockKind::CodingContext,
        title: "coding_workspace_context".to_string(),
        content: workspace_content,
        source: PromptBlockSource {
            subsystem: "coding_augment".to_string(),
            reference: Some("workspace".to_string()),
        },
        priority: 60,
        is_sensitive: false,
        cache_hint: CacheHint::None,
    });

    // Tool surface block (if tools are available)
    if !registered_tool_names.is_empty() {
        let tool_list = registered_tool_names.join(", ");
        let tool_content = format!(
            "# Available Tools\n\nYou have access to the following tools: {}\n\n\
             Use these tools to read files, execute commands, and interact with the codebase.",
            tool_list
        );

        blocks.push(PromptBlock {
            id: "coding_tool_surface".to_string(),
            kind: PromptBlockKind::CodingContext,
            title: "coding_tool_surface".to_string(),
            content: tool_content,
            source: PromptBlockSource {
                subsystem: "coding_augment".to_string(),
                reference: Some("tool_surface".to_string()),
            },
            priority: 59,
            is_sensitive: false,
            cache_hint: CacheHint::None,
        });
    }

    blocks
}

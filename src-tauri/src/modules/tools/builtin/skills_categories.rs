//! SkillsCategories tool — browse and filter skills by category/tag.
//!
//! Provides category-based browsing of skills. Lists all available categories
//! and skills within a given category.

use std::collections::HashMap;
use std::sync::Arc;

use crate::modules::runtime::prompt::collect_skill_index_entries;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// A skill category with count and sample skills.
#[derive(serde::Serialize)]
struct CategoryInfo {
    name: String,
    count: usize,
    skills: Vec<SkillSummary>,
}

/// Summary of a skill for category listing.
#[derive(serde::Serialize)]
struct SkillSummary {
    name: String,
    description: String,
}

/// Result returned when querying categories.
#[derive(serde::Serialize)]
struct CategoriesResult {
    success: bool,
    categories: Vec<CategoryInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Result when querying skills in a category.
#[derive(serde::Serialize)]
struct SkillsInCategoryResult {
    success: bool,
    category: String,
    skills: Vec<SkillSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Creates the `skills_categories` ToolEntry.
#[must_use]
pub fn skills_categories_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            let category = args.get("category").and_then(|v| v.as_str());

            // Get all skills via the same index as skill_search
            let entries = collect_skill_index_entries(&workdir, None);

            if let Some(cat) = category {
                // Return skills in the specified category
                let normalized_cat = cat.to_lowercase();
                let skills: Vec<SkillSummary> = entries
                    .into_iter()
                    .filter(|e| {
                        e.name.to_lowercase() == normalized_cat
                            || e.description.to_lowercase().contains(&normalized_cat)
                    })
                    .map(|e| SkillSummary {
                        name: e.name,
                        description: e.description,
                    })
                    .collect();

                let result = SkillsInCategoryResult {
                    success: true,
                    category: cat.to_string(),
                    skills,
                    error: None,
                };
                serde_json::to_string_pretty(&result).map_err(|e| ToolError::Handler(e.to_string()))
            } else {
                // Return all categories with counts
                let mut tag_counts: HashMap<String, Vec<SkillSummary>> = HashMap::new();

                for entry in entries {
                    // Use the skill name as its own category (primary category = skill name)
                    let skill_summary = SkillSummary {
                        name: entry.name.clone(),
                        description: entry.description.clone(),
                    };

                    // Group by name as primary category
                    tag_counts
                        .entry(entry.name.clone())
                        .or_default()
                        .push(skill_summary);

                    // Also group by first word of description as category hint
                    if !entry.description.is_empty() {
                        let first_word = entry
                            .description
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_lowercase();
                        if first_word.len() > 2 {
                            let summary = SkillSummary {
                                name: entry.name,
                                description: entry.description,
                            };
                            tag_counts.entry(first_word).or_default().push(summary);
                        }
                    }
                }

                let mut categories: Vec<CategoryInfo> = tag_counts
                    .into_iter()
                    .map(|(name, skills)| CategoryInfo {
                        name,
                        count: skills.len(),
                        skills,
                    })
                    .collect();

                // Sort by count descending
                categories.sort_by(|a, b| b.count.cmp(&a.count));

                let result = CategoriesResult {
                    success: true,
                    categories,
                    error: None,
                };
                serde_json::to_string_pretty(&result).map_err(|e| ToolError::Handler(e.to_string()))
            }
        })
    });

    ToolEntry {
        name: "skills_categories".to_string(),
        toolset: "utility".to_string(),
        description: "Browse skills by category or tag. \
                      Without a category argument, lists all available categories. \
                      With a category, returns skills in that category."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Optional category name to filter by. If not provided, returns all categories."
                }
            }
        }),
        max_result_size: Some(32 * 1024),
        timeout_secs: Some(5),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_categories_tool_entry_has_correct_structure() {
        let entry = skills_categories_tool_entry();
        assert_eq!(entry.name, "skills_categories");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }
}

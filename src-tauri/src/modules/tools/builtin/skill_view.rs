//! skill_view tool — inspect a skill's full content, metadata, and linked files.
//!
//! Equivalent to Hermes `skill_view` tool (`tools/skills_tool.py`).
//!
//! Provides:
//! - Full SKILL.md content
//! - Linked files structure (references/, templates/, scripts/, assets/)
//! - Tags and related_skills
//! - Required environment variables
//! - Readiness status (available / setup_needed)
//! - Optional file_path to read a specific supporting file

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum SKILL.md content returned by skill_view (characters).
/// Keep this modest to avoid context blow-up when model redundantly calls
/// both `skill` and `skill_view`.
const MAX_SKILL_VIEW_CONTENT_CHARS: usize = 4096;

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!(
        "{truncated}\n\n[skill_view content truncated: showing first {max_chars} chars of {count}]"
    )
}

/// Skill view result returned as JSON.
#[derive(serde::Serialize)]
struct SkillViewResult {
    success: bool,
    name: String,
    description: String,
    content: String,
    tags: Vec<String>,
    related_skills: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reference_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    template_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    script_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    asset_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    required_env_vars: Vec<EnvVarInfo>,
    readiness: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_binary: Option<bool>,
}

#[derive(serde::Serialize)]
struct EnvVarInfo {
    name: String,
    description: String,
    required_for: Option<String>,
}

/// Creates the `skill_view` ToolEntry.
#[must_use]
pub fn skill_view_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let workdir = {
                let lock = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("Failed to lock tool context: {e}")))?;
                lock.workdir.clone()
            };

            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'name' argument".into()))?;

            let file_path = args.get("file_path").and_then(|v| v.as_str());

            let result = view_skill(&workdir, name, file_path);
            serde_json::to_string_pretty(&result).map_err(|e| ToolError::Handler(e.to_string()))
        })
    });

    ToolEntry {
        name: "skill_view".to_string(),
        toolset: "utility".to_string(),
        description: "View the full content of a skill including metadata, linked files, \
                      required environment variables, and readiness status. \
                      Use file_path to read a specific supporting file. \
                      Prefer `skill` for activation in normal conversations; \
                      `skill_view` is primarily for inspection/debugging."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Skill name (directory name)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Optional: specific file within skill (e.g., 'references/api.md')"
                }
            },
            "required": ["name"]
        }),
        max_result_size: Some(64 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

/// Internal skill viewing logic.
fn view_skill(workdir: &std::path::Path, name: &str, file_path: Option<&str>) -> SkillViewResult {
    let roots = discover_skill_roots_with_metadata(workdir);

    // Find the skill directory
    let mut skill_dir: Option<PathBuf> = None;
    let mut skill_md_path: Option<PathBuf> = None;

    for root in &roots {
        // Try direct path first
        let direct_path = root.path.join(name);
        if direct_path.is_dir() {
            let md = direct_path.join("SKILL.md");
            if md.is_file() {
                skill_dir = Some(direct_path);
                skill_md_path = Some(md);
                break;
            }
        }
        // Try rglob search by directory name
        if let Ok(entries) = std::fs::read_dir(&root.path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().map(|n| n == name).unwrap_or(false) {
                    let md = path.join("SKILL.md");
                    if md.is_file() {
                        skill_dir = Some(path);
                        skill_md_path = Some(md);
                        break;
                    }
                }
            }
        }
        if skill_dir.is_some() {
            break;
        }
    }

    let Some(skill_md) = skill_md_path else {
        return SkillViewResult {
            success: false,
            name: name.to_string(),
            description: String::new(),
            content: String::new(),
            tags: Vec::new(),
            related_skills: Vec::new(),
            reference_files: Vec::new(),
            template_files: Vec::new(),
            script_files: Vec::new(),
            asset_files: Vec::new(),
            required_env_vars: Vec::new(),
            readiness: "unknown".to_string(),
            path: String::new(),
            error: Some(format!("Skill '{}' not found.", name)),
            file_content: None,
            is_binary: None,
        };
    };

    // Read SKILL.md content
    let raw_content = match std::fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(e) => {
            return SkillViewResult {
                success: false,
                name: name.to_string(),
                description: String::new(),
                content: String::new(),
                tags: Vec::new(),
                related_skills: Vec::new(),
                reference_files: Vec::new(),
                template_files: Vec::new(),
                script_files: Vec::new(),
                asset_files: Vec::new(),
                required_env_vars: Vec::new(),
                readiness: "unknown".to_string(),
                path: format!("skill://{}", name),
                error: Some(format!("Failed to read skill: {}", e)),
                file_content: None,
                is_binary: None,
            };
        }
    };
    let content = truncate_chars(&raw_content, MAX_SKILL_VIEW_CONTENT_CHARS);

    // Parse frontmatter
    let (description, tags, related_skills, required_env_vars, _frontmatter) =
        parse_frontmatter(&raw_content);

    // Check readiness based on required env vars
    let readiness = check_readiness_from_vars(&required_env_vars);

    // If file_path is specified, try to read that specific file
    if let Some(rel_path) = file_path {
        if let Some(ref dir) = skill_dir {
            // Security: prevent path traversal
            if rel_path.contains("..") {
                return SkillViewResult {
                    success: false,
                    name: name.to_string(),
                    description,
                    content,
                    tags,
                    related_skills,
                    reference_files: Vec::new(),
                    template_files: Vec::new(),
                    script_files: Vec::new(),
                    asset_files: Vec::new(),
                    required_env_vars,
                    readiness,
                    path: format!("skill://{}", name),
                    error: Some("Path traversal ('..') is not allowed.".to_string()),
                    file_content: None,
                    is_binary: None,
                };
            }

            let target_file = dir.join(rel_path);

            // Security: verify path is within skill directory
            if !target_file.starts_with(dir) {
                return SkillViewResult {
                    success: false,
                    name: name.to_string(),
                    description,
                    content,
                    tags,
                    related_skills,
                    reference_files: Vec::new(),
                    template_files: Vec::new(),
                    script_files: Vec::new(),
                    asset_files: Vec::new(),
                    required_env_vars,
                    readiness,
                    path: format!("skill://{}", name),
                    error: Some("Path escapes skill directory boundary.".to_string()),
                    file_content: None,
                    is_binary: None,
                };
            }

            if !target_file.is_file() {
                return SkillViewResult {
                    success: false,
                    name: name.to_string(),
                    description,
                    content,
                    tags,
                    related_skills,
                    reference_files: Vec::new(),
                    template_files: Vec::new(),
                    script_files: Vec::new(),
                    asset_files: Vec::new(),
                    required_env_vars,
                    readiness,
                    path: format!("skill://{}", name),
                    error: Some(format!("File '{}' not found in skill.", rel_path)),
                    file_content: None,
                    is_binary: None,
                };
            }

            // Try to read as text
            match std::fs::read_to_string(&target_file) {
                Ok(file_content) => {
                    return SkillViewResult {
                        success: true,
                        name: name.to_string(),
                        description,
                        content,
                        tags,
                        related_skills,
                        reference_files: Vec::new(),
                        template_files: Vec::new(),
                        script_files: Vec::new(),
                        asset_files: Vec::new(),
                        required_env_vars,
                        readiness,
                        path: format!("skill://{}", name),
                        error: None,
                        file_content: Some(file_content),
                        is_binary: Some(false),
                    };
                }
                Err(_) => {
                    // Binary file
                    let size = std::fs::metadata(&target_file)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    return SkillViewResult {
                        success: true,
                        name: name.to_string(),
                        description,
                        content,
                        tags,
                        related_skills,
                        reference_files: Vec::new(),
                        template_files: Vec::new(),
                        script_files: Vec::new(),
                        asset_files: Vec::new(),
                        required_env_vars,
                        readiness,
                        path: format!("skill://{}", name),
                        error: None,
                        file_content: Some(format!(
                            "[Binary file: {}, size: {} bytes]",
                            target_file
                                .file_name()
                                .map(|n| n.to_string_lossy())
                                .unwrap_or_default(),
                            size
                        )),
                        is_binary: Some(true),
                    };
                }
            }
        }
    }

    // Collect linked files
    let (reference_files, template_files, script_files, asset_files) =
        collect_linked_files(skill_dir.as_ref());

    SkillViewResult {
        success: true,
        name: name.to_string(),
        description,
        content,
        tags,
        related_skills,
        reference_files,
        template_files,
        script_files,
        asset_files,
        required_env_vars,
        readiness,
        path: format!("skill://{}", name),
        error: None,
        file_content: None,
        is_binary: None,
    }
}

/// Parse frontmatter from SKILL.md content.
#[allow(clippy::type_complexity)]
fn parse_frontmatter(
    content: &str,
) -> (
    String,
    Vec<String>,
    Vec<String>,
    Vec<EnvVarInfo>,
    HashMap<String, String>,
) {
    let mut description = String::new();
    let mut tags = Vec::new();
    let mut related_skills = Vec::new();
    let mut required_env_vars = Vec::new();
    let metadata_hermes: HashMap<String, String> = HashMap::new();

    // Find frontmatter
    let body = content.strip_prefix("---").unwrap_or(content);
    let Some((header, _)) = body.split_once("---") else {
        return (
            description,
            tags,
            related_skills,
            required_env_vars,
            HashMap::new(),
        );
    };

    let mut in_hermes_block = false;
    let mut hermes_block_lines = Vec::new();

    for line in header.lines() {
        let line = line.trim();
        if line.starts_with("metadata:") || line.starts_with("metadata:") {
            continue;
        }
        if line.starts_with("hermes:") || line.starts_with("  hermes:") {
            in_hermes_block = true;
            hermes_block_lines.clear();
            continue;
        }
        if in_hermes_block {
            if line.is_empty() || (!line.starts_with(' ') && !line.starts_with('\t')) {
                in_hermes_block = false;
            } else {
                hermes_block_lines.push(line);
                continue;
            }
        }

        if line.starts_with("description:") {
            description = line
                .strip_prefix("description:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("tags:") {
            let inner = line
                .strip_prefix("tags:")
                .unwrap_or("")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            tags = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if line.starts_with("related_skills:") {
            let inner = line
                .strip_prefix("related_skills:")
                .unwrap_or("")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            related_skills = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if line.starts_with("required_environment_variables:") {
            let inner = line
                .strip_prefix("required_environment_variables:")
                .unwrap_or("")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            for var in inner.split(',') {
                let var = var.trim().to_string();
                if !var.is_empty() {
                    required_env_vars.push(EnvVarInfo {
                        name: var,
                        description: String::new(),
                        required_for: None,
                    });
                }
            }
        }
    }

    // Parse hermes block
    for line in hermes_block_lines {
        let line = line.trim();
        if line.starts_with("tags:") {
            let inner = line
                .strip_prefix("tags:")
                .unwrap_or("")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            tags = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if line.starts_with("related_skills:") {
            let inner = line
                .strip_prefix("related_skills:")
                .unwrap_or("")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            related_skills = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    (
        description,
        tags,
        related_skills,
        required_env_vars,
        metadata_hermes,
    )
}

/// Collect linked files from a skill directory.
fn collect_linked_files(
    skill_dir: Option<&PathBuf>,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut reference_files = Vec::new();
    let mut template_files = Vec::new();
    let mut script_files = Vec::new();
    let mut asset_files = Vec::new();

    let Some(dir) = skill_dir else {
        return (reference_files, template_files, script_files, asset_files);
    };

    // References
    let references_dir = dir.join("references");
    if references_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&references_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(rel) = path.strip_prefix(dir) {
                        reference_files.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    // Templates
    let templates_dir = dir.join("templates");
    if templates_dir.is_dir() {
        for ext in &["md", "py", "yaml", "yml", "json", "sh"] {
            if let Ok(entries) = glob::glob(
                &templates_dir
                    .join(format!("**/*.{}", ext))
                    .display()
                    .to_string(),
            ) {
                for entry in entries.flatten() {
                    let path = entry;
                    if let Ok(rel) = path.strip_prefix(dir) {
                        template_files.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    // Scripts
    let scripts_dir = dir.join("scripts");
    if scripts_dir.is_dir() {
        for ext in &["py", "sh", "bash", "js", "ts", "rb"] {
            if let Ok(entries) = glob::glob(
                &scripts_dir
                    .join(format!("**/*.{}", ext))
                    .display()
                    .to_string(),
            ) {
                for entry in entries.flatten() {
                    let path = entry;
                    if let Ok(rel) = path.strip_prefix(dir) {
                        script_files.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    // Assets
    let assets_dir = dir.join("assets");
    if assets_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&assets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(rel) = path.strip_prefix(dir) {
                        asset_files.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    (reference_files, template_files, script_files, asset_files)
}

/// Check readiness based on required environment variables.
fn check_readiness_from_vars(env_vars: &[EnvVarInfo]) -> String {
    for var in env_vars {
        if std::env::var(&var.name).is_err() {
            return "setup_needed".to_string();
        }
    }
    "available".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_skill(dir: &std::path::Path, name: &str, content: &str) {
        // Skills must be in a discoverable location like .if2ai/skills/<name>
        let skill_dir = dir.join(".if2ai/skills").join(name);
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), content).expect("write skill file");
    }

    #[test]
    fn test_skill_view_returns_content() {
        let tmp = TempDir::new().expect("create temp dir");
        create_test_skill(
            tmp.path(),
            "test-skill",
            r#"---
description: A test skill
tags: [testing, rust]
---
# Test Skill

This is test content.
"#,
        );

        let result = view_skill(tmp.path(), "test-skill", None);
        assert!(result.success);
        assert_eq!(result.name, "test-skill");
        assert_eq!(result.description, "A test skill");
        assert!(result.content.contains("Test Skill"));
        assert!(result.tags.contains(&"testing".to_string()));
    }

    #[test]
    fn test_skill_view_not_found() {
        let tmp = TempDir::new().expect("create temp dir");
        let result = view_skill(tmp.path(), "nonexistent", None);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_skill_view_reads_linked_file() {
        let tmp = TempDir::new().expect("create temp dir");
        // Skills must be in a discoverable location
        let skill_dir = tmp.path().join(".if2ai/skills/test-skill");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: test\n---\n# Test",
        )
        .expect("write skill");

        let references_dir = skill_dir.join("references");
        std::fs::create_dir_all(&references_dir).expect("create references dir");
        std::fs::write(references_dir.join("api.md"), "# API Reference").expect("write api.md");

        let result = view_skill(tmp.path(), "test-skill", Some("references/api.md"));
        assert!(result.success);
        assert!(result.file_content.is_some());
        assert!(result.file_content.unwrap().contains("API Reference"));
    }

    #[test]
    fn test_readiness_with_missing_env() {
        std::env::remove_var("NONEXISTENT_TEST_VAR_12345");
        let tmp = TempDir::new().expect("create temp dir");
        create_test_skill(
            tmp.path(),
            "test-skill",
            r#"---
required_environment_variables: [NONEXISTENT_TEST_VAR_12345]
---
# Test
"#,
        );

        let result = view_skill(tmp.path(), "test-skill", None);
        assert_eq!(result.readiness, "setup_needed");
    }

    #[test]
    fn skill_view_tool_entry_has_correct_structure() {
        let entry = skill_view_tool_entry();
        assert_eq!(entry.name, "skill_view");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }
}

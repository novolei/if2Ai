//! Skill commands — slash command integration for /skill-name invocation.
//!
//! Ported from Hermes `agent/skill_commands.py` lines 200-326.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use thiserror::Error;

/// Errors for skill command operations.
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("skill read error: {0}")]
    ReadError(String),
    #[error("skill invocation error: {0}")]
    InvocationError(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[allow(dead_code)]
    #[error("parse error: {0}")]
    Parse(String),
}

/// Result type for command operations.
pub type CommandResult<T> = Result<T, CommandError>;

/// Platform mapping for cross-platform skill compatibility.
///
/// Maps user-facing platform names to internal identifiers.
const PLATFORM_MAP: &[(&str, &str)] = &[
    ("macos", "darwin"),
    ("linux", "linux"),
    ("windows", "win32"),
];

/// Directories to exclude when scanning for skills.
const EXCLUDED_SKILL_DIRS: &[&str] = &[".git", ".github", ".hub"];

/// Invalid characters in skill command keys.
///
/// Skill command names must only contain lowercase letters, numbers, and hyphens.
const SKILL_INVALID_CHARS: &str = r"[^a-z0-9-]";

/// Multiple consecutive hyphens are not allowed.
const SKILL_MULTI_HYPHEN: &str = r"-{2,}";

/// Info about a discovered skill command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SkillCommandInfo {
    /// Command name (e.g., "/my-skill").
    pub name: String,
    /// Skill description from SKILL.md.
    pub description: String,
    /// Path to the SKILL.md file.
    pub skill_md_path: PathBuf,
    /// Path to the skill directory.
    pub skill_dir: PathBuf,
    /// Supporting files relative to skill_dir.
    pub supporting_files: Vec<String>,
    /// Supported platforms (e.g., ["macos", "linux"]).
    pub platforms: Vec<String>,
    /// Whether this skill is enabled.
    pub enabled: bool,
    /// Config block content if present.
    pub config_block: Option<String>,
    /// Activation note template.
    pub activation_note: Option<String>,
}

impl SkillCommandInfo {
    /// Check if this skill is available on the given platform.
    #[allow(dead_code)]
    pub fn is_available_on_platform(&self, platform: &str) -> bool {
        // If no platforms specified, available on all
        if self.platforms.is_empty() {
            return true;
        }
        self.platforms.iter().any(|p| {
            let p_lower = p.to_lowercase();
            // Check direct match
            p_lower == platform.to_lowercase()
                // Or check via platform map
                || PLATFORM_MAP.iter().any(|(alias, target)| {
                    (p_lower == *alias && *target == platform) || (p_lower == *target && *alias == platform)
                })
        })
    }
}

/// Skill invocation builder — constructs the message for invoking a skill.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SkillInvocationBuilder {
    /// The activation note message.
    pub activation_note: String,
    /// The skill content from SKILL.md.
    pub skill_content: String,
    /// Setup notes from supporting files.
    pub setup_notes: Vec<String>,
    /// Supporting file paths.
    pub supporting_files: Vec<String>,
    /// Resolved config block.
    pub config_block: Option<String>,
}

impl SkillInvocationBuilder {
    /// Create a new builder with the required fields.
    pub fn new(activation_note: String, skill_content: String) -> Self {
        Self {
            activation_note,
            skill_content,
            setup_notes: Vec::new(),
            supporting_files: Vec::new(),
            config_block: None,
        }
    }

    /// Add a supporting file.
    pub fn with_supporting_file(mut self, path: &str) -> Self {
        self.supporting_files.push(path.to_string());
        self
    }

    /// Add a setup note.
    pub fn with_setup_note(mut self, note: &str) -> Self {
        self.setup_notes.push(note.to_string());
        self
    }

    /// Set the config block.
    pub fn with_config(mut self, config: Option<String>) -> Self {
        self.config_block = config;
        self
    }

    /// Build the full invocation message.
    pub fn build(&self) -> String {
        let mut message = format!("{}\n\n", self.activation_note);

        if !self.setup_notes.is_empty() {
            message.push_str("## Setup Notes\n");
            for note in &self.setup_notes {
                message.push_str(&format!("- {}\n", note));
            }
            message.push('\n');
        }

        message.push_str("## Skill Content\n\n");
        message.push_str(&self.skill_content);
        message.push('\n');

        if let Some(ref config) = self.config_block {
            message.push_str("\n## Configuration\n\n");
            message.push_str(config);
            message.push('\n');
        }

        if !self.supporting_files.is_empty() {
            message.push_str("\n## Supporting Files\n\n");
            for file in &self.supporting_files {
                message.push_str(&format!("- {}\n", file));
            }
            message.push('\n');
        }

        message
    }
}

/// Skill commands manager — discovers and manages skill slash commands.
#[allow(dead_code)]
pub struct SkillCommands {
    commands: RwLock<HashMap<String, SkillCommandInfo>>,
    skills_dir: PathBuf,
    disabled_skills: Vec<String>,
}

impl Default for SkillCommands {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

#[allow(dead_code)]
impl SkillCommands {
    /// Create a new SkillCommands scanner.
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
            skills_dir: skills_dir.into(),
            disabled_skills: Vec::new(),
        }
    }

    /// Create with disabled skills list.
    pub fn with_disabled_skills(skills_dir: impl Into<PathBuf>, disabled: Vec<String>) -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
            skills_dir: skills_dir.into(),
            disabled_skills: disabled,
        }
    }

    /// Scan the skills directory and discover all skills.
    ///
    /// This rescans on each call to pick up new skills.
    pub fn scan(&self) -> CommandResult<HashMap<String, SkillCommandInfo>> {
        let mut commands = HashMap::new();

        if !self.skills_dir.exists() {
            return Ok(commands);
        }

        // Scan subdirectories
        for entry in fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip excluded directories
            if EXCLUDED_SKILL_DIRS.contains(&dir_name) {
                continue;
            }

            // Check if this skill is disabled
            if self.disabled_skills.contains(&dir_name.to_string()) {
                continue;
            }

            // Look for SKILL.md
            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            // Parse the SKILL.md
            match self.parse_skill_md(&skill_md_path, &path) {
                Ok(info) => {
                    let command_key = normalize_command_key(&info.name);
                    commands.insert(command_key, info);
                }
                Err(e) => {
                    eprintln!("failed to parse {}: {}", skill_md_path.display(), e);
                }
            }
        }

        // Update cache
        {
            let mut cached = self
                .commands
                .write()
                .map_err(|e| CommandError::InvocationError(format!("lock poisoned: {}", e)))?;
            *cached = commands.clone();
        }

        Ok(commands)
    }

    /// Get the cached command map.
    pub fn get(&self) -> HashMap<String, SkillCommandInfo> {
        self.commands
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Resolve a command key, handling normalization.
    ///
    /// Converts various input formats to normalized command keys:
    /// - "my skill" -> "my-skill"
    /// - "my_skill" -> "my-skill"
    /// - "/my-skill" -> "my-skill"
    pub fn resolve(&self, command: &str) -> Option<String> {
        let normalized = normalize_command_key(command);
        let commands = self.get();
        if commands.contains_key(&normalized) {
            Some(normalized)
        } else {
            None
        }
    }

    /// Build an invocation message for a skill.
    pub fn build_invocation_message(
        &self,
        cmd_key: &str,
        user_instruction: &str,
    ) -> CommandResult<String> {
        let commands = self.get();
        let info = commands
            .get(cmd_key)
            .ok_or_else(|| CommandError::NotFound(cmd_key.to_string()))?;

        let activation_note = format!(
            "[SYSTEM: The user has invoked skill '{}' with instruction: {}]",
            info.name, user_instruction
        );

        let skill_content = fs::read_to_string(&info.skill_md_path)
            .map_err(|e| CommandError::ReadError(e.to_string()))?;

        let mut builder = SkillInvocationBuilder::new(activation_note, skill_content);

        // Add supporting files
        for supporting_file in &info.supporting_files {
            let file_path = info.skill_dir.join(supporting_file);
            if file_path.exists() {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let setup_note = format!(
                        "File: {} - {}",
                        supporting_file,
                        content.lines().take(3).collect::<Vec<_>>().join(" ")
                    );
                    builder = builder.with_setup_note(&setup_note);
                }
            }
            builder = builder.with_supporting_file(supporting_file);
        }

        // Add config block if present
        if let Some(ref config) = info.config_block {
            builder = builder.with_config(Some(config.clone()));
        }

        Ok(builder.build())
    }

    /// Build a preloaded prompt for session initialization.
    ///
    /// Returns (preloaded_text, skill_names, supporting_file_paths).
    pub fn build_preloaded_prompt(
        &self,
        identifiers: &[String],
    ) -> CommandResult<(String, Vec<String>, Vec<String>)> {
        let commands = self.get();
        let mut preloaded_parts = Vec::new();
        let mut skill_names = Vec::new();
        let mut all_supporting_files = Vec::new();

        for identifier in identifiers {
            let normalized = normalize_command_key(identifier);
            if let Some(info) = commands.get(&normalized) {
                skill_names.push(info.name.clone());

                let content = fs::read_to_string(&info.skill_md_path)
                    .map_err(|e| CommandError::ReadError(e.to_string()))?;

                preloaded_parts.push(format!("## Skill: {}\n\n{}", info.name, content));

                for sf in &info.supporting_files {
                    all_supporting_files.push(info.skill_dir.join(sf).display().to_string());
                }
            }
        }

        let preloaded_text = if preloaded_parts.is_empty() {
            String::new()
        } else {
            format!("# Preloaded Skills\n\n{}\n", preloaded_parts.join("\n\n"))
        };

        Ok((preloaded_text, skill_names, all_supporting_files))
    }

    /// Parse a SKILL.md file and extract metadata.
    fn parse_skill_md(
        &self,
        skill_md_path: &Path,
        skill_dir: &Path,
    ) -> CommandResult<SkillCommandInfo> {
        let content = fs::read_to_string(skill_md_path)?;
        let name = skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract frontmatter if present
        let (description, platforms, config_block, activation_note) = if content.starts_with("---")
        {
            self.parse_frontmatter(&content)?
        } else {
            (String::new(), Vec::new(), None, None)
        };

        // Scan for supporting files (files in skill_dir that aren't SKILL.md)
        let mut supporting_files = Vec::new();
        if let Ok(entries) = fs::read_dir(skill_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name != "SKILL.md" {
                            // Get relative path from skill_dir
                            if let Ok(rel) = path.strip_prefix(skill_dir) {
                                supporting_files.push(rel.display().to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(SkillCommandInfo {
            name,
            description,
            skill_md_path: skill_md_path.to_path_buf(),
            skill_dir: skill_dir.to_path_buf(),
            supporting_files,
            platforms,
            enabled: true,
            config_block,
            activation_note,
        })
    }

    /// Parse YAML frontmatter from SKILL.md content.
    #[allow(clippy::type_complexity)]
    fn parse_frontmatter(
        &self,
        content: &str,
    ) -> CommandResult<(String, Vec<String>, Option<String>, Option<String>)> {
        let mut description = String::new();
        let mut platforms = Vec::new();
        let mut config_block = None;
        let activation_note = None;

        // Find frontmatter boundaries
        let start = content.find("---").map(|i| i + 3);
        let end = start.and_then(|s| content[s..].find("---").map(|e| s + e));

        if let (Some(s), Some(e)) = (start, end) {
            let frontmatter = &content[s..e];

            for line in frontmatter.lines() {
                let line = line.trim();
                if line.starts_with("description:") {
                    description = line
                        .strip_prefix("description:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                } else if line.starts_with("platforms:") {
                    let platforms_str = line.strip_prefix("platforms:").unwrap_or("").trim();
                    // Parse array format: [macos, linux]
                    if platforms_str.starts_with('[') && platforms_str.ends_with(']') {
                        let inner = &platforms_str[1..platforms_str.len() - 1];
                        platforms = inner
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                } else if line.starts_with("config:") || line.starts_with("configuration:") {
                    // Config block follows on subsequent lines
                    let mut config_lines = Vec::new();
                    let rest = &content[e + 3..];
                    for config_line in rest.lines() {
                        let trimmed = config_line.trim();
                        if trimmed.is_empty() || trimmed == "---" {
                            break;
                        }
                        config_lines.push(trimmed.to_string());
                    }
                    if !config_lines.is_empty() {
                        config_block = Some(config_lines.join("\n"));
                    }
                }
            }
        }

        Ok((description, platforms, config_block, activation_note))
    }
}

/// Normalize a command key.
///
/// - Converts spaces and underscores to hyphens
/// - Converts to lowercase
/// - Removes leading slash
/// - Collapses multiple hyphens
#[allow(clippy::collapsible_str_replace)]
#[allow(dead_code)]
pub fn normalize_command_key(input: &str) -> String {
    let invalid_chars_regex = Regex::new(SKILL_INVALID_CHARS).unwrap();
    let multi_hyphen_regex = Regex::new(SKILL_MULTI_HYPHEN).unwrap();

    let normalized = input
        .trim()
        .trim_start_matches('/')
        .to_lowercase()
        .replace(' ', "-")
        .replace('_', "-");

    let normalized = invalid_chars_regex.replace_all(&normalized, "").to_string();
    let normalized = multi_hyphen_regex.replace_all(&normalized, "-").to_string();

    // Trim leading/trailing hyphens
    normalized.trim_matches('-').to_string()
}

/// Check if a platform is compatible.
#[allow(dead_code)]
pub fn is_platform_compatible(skill_platforms: &[String], current_platform: &str) -> bool {
    if skill_platforms.is_empty() {
        return true; // All platforms if not specified
    }

    skill_platforms.iter().any(|p| {
        let p_lower = p.to_lowercase();
        p_lower == current_platform.to_lowercase()
            || PLATFORM_MAP.iter().any(|(alias, target)| {
                (p_lower == *alias && *target == current_platform)
                    || (p_lower == *target && *alias == current_platform)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_command_key() {
        assert_eq!(normalize_command_key("/My-Skill"), "my-skill");
        assert_eq!(normalize_command_key("my skill"), "my-skill");
        assert_eq!(normalize_command_key("my_skill"), "my-skill");
        assert_eq!(normalize_command_key("My Skill 123"), "my-skill-123");
        assert_eq!(normalize_command_key("--my--skill--"), "my-skill");
        assert_eq!(normalize_command_key("ABC"), "abc");
    }

    #[test]
    fn test_platform_compatible() {
        // Empty platforms = all platforms
        assert!(is_platform_compatible(&[], "darwin"));

        // Direct match
        assert!(is_platform_compatible(&["macos".to_string()], "darwin"));
        assert!(!is_platform_compatible(&["linux".to_string()], "darwin"));

        // Platform map
        assert!(is_platform_compatible(&["macos".to_string()], "darwin"));
    }

    #[test]
    fn test_skill_command_info_platform() {
        let info = SkillCommandInfo {
            name: "test-skill".to_string(),
            description: "A test".to_string(),
            skill_md_path: PathBuf::from("/skills/test/SKILL.md"),
            skill_dir: PathBuf::from("/skills/test"),
            supporting_files: vec![],
            platforms: vec!["macos".to_string(), "linux".to_string()],
            enabled: true,
            config_block: None,
            activation_note: None,
        };

        assert!(info.is_available_on_platform("darwin"));
        assert!(info.is_available_on_platform("linux"));
        assert!(!info.is_available_on_platform("win32"));
    }

    #[test]
    fn test_skill_invocation_builder() {
        let builder = SkillInvocationBuilder::new(
            "[SYSTEM: invoked]".to_string(),
            "# My Skill\n\nThis is the content.".to_string(),
        )
        .with_supporting_file("config.json")
        .with_setup_note("Setup step 1")
        .with_config(Some("key: value".to_string()));

        let message = builder.build();
        assert!(message.contains("[SYSTEM: invoked]"));
        assert!(message.contains("# My Skill"));
        assert!(message.contains("config.json"));
        assert!(message.contains("Setup step 1"));
        assert!(message.contains("key: value"));
    }

    #[test]
    fn test_skill_invocation_builder_minimal() {
        let builder = SkillInvocationBuilder::new(
            "[SYSTEM: invoked]".to_string(),
            "# Minimal\n\nContent.".to_string(),
        );
        let message = builder.build();
        assert!(message.contains("[SYSTEM: invoked]"));
        assert!(message.contains("# Minimal"));
        assert!(!message.contains("Setup Notes"));
        assert!(!message.contains("Supporting Files"));
    }

    #[test]
    fn test_skill_commands_scan() {
        let temp = tempfile::TempDir::new().unwrap();
        let skills_dir = temp.path();

        // Create a skill directory with proper frontmatter
        let skill_dir = skills_dir.join("my-test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_content = "\
---
description: A test skill
---
# Test Skill

This is the skill content.
";
        std::fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

        let commands = SkillCommands::new(skills_dir);
        let result = commands.scan().unwrap();

        assert!(result.contains_key("my-test-skill"));
        let info = result.get("my-test-skill").unwrap();
        assert_eq!(info.name, "my-test-skill");
        assert_eq!(info.description, "A test skill");
    }

    #[test]
    fn test_skill_commands_resolve() {
        let temp = tempfile::TempDir::new().unwrap();
        let skills_dir = temp.path();

        let skill_dir = skills_dir.join("my-test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "description: A test skill").unwrap();

        let commands = SkillCommands::new(skills_dir);
        commands.scan().unwrap();

        // Test resolve with various normalizations
        assert_eq!(
            commands.resolve("/my-test-skill"),
            Some("my-test-skill".to_string())
        );
        assert_eq!(
            commands.resolve("my-test-skill"),
            Some("my-test-skill".to_string())
        );
        assert_eq!(
            commands.resolve("My Test Skill"),
            Some("my-test-skill".to_string())
        );
        assert_eq!(
            commands.resolve("my_test_skill"),
            Some("my-test-skill".to_string())
        );
        assert_eq!(commands.resolve("unknown-skill"), None);
    }
}

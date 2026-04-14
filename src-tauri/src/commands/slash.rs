//! SlashCommand Tauri commands — F5 + F17.
//!
//! Exposes parse, list, suggest, execute, list_skills, list_agents
//! to the frontend via Tauri IPC.
//! See docs/bs_gap/08-critical-fix-priority.md §F5, §F17.

use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashSet;
use tauri::State;

use crate::commands::AppState;
use crate::modules::control_plane::audit::{AuditEmitter, SkillDistributionDiagnostic};
use crate::modules::skills::commands::{SkillCommandInfo, SkillCommands};
use crate::modules::tools::registry::{
    validate_distribution_envelope, SkillDistributionChannel, SkillDistributionEnvelope,
};
use crate::modules::tools::toolset::ToolSetRegistry;
use crate::modules::tools::ToolRegistry;

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Result of parsing a slash command input.
#[derive(Serialize)]
pub struct SlashCommandParseResult {
    pub name: String,
    pub matched: bool,
}

/// Spec for a registered slash command.
#[derive(Serialize)]
pub struct SlashCommandSpecDto {
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Info about a discovered skill.
#[derive(Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: String,
    pub source: String,
    pub review_status: String,
    pub status: String,
    pub enabled: bool,
    pub read_only: bool,
    pub shadowed_by: Option<String>,
}

/// Info about a discovered agent.
#[derive(Serialize)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub path: String,
}

// ── Skill toolset availability helpers ───────────────────────────────────────

/// Returns toolset names where ALL tools in the toolset are registered.
///
/// A toolset is considered "available" if every tool it declares is actually
/// registered in the tool registry. This matches the Hermes behavior where
/// a skill with `requires_toolsets: [files]` only activates if all `files`
/// tools (read_file, file_write, etc.) are present.
fn available_toolsets(registry: &ToolRegistry) -> Vec<String> {
    let toolset_registry = ToolSetRegistry::new();
    let registered: HashSet<String> = registry.tool_names().into_iter().collect();

    toolset_registry
        .all_toolsets()
        .iter()
        .filter(|ts| ts.tools.iter().all(|t| registered.contains(t)))
        .map(|ts| ts.name.clone())
        .collect()
}

/// Find a skill's SkillCommandInfo by normalized command key.
fn find_skill_info(workdir: &std::path::Path, skill_key: &str) -> Option<SkillCommandInfo> {
    // Check workspace skills
    let skills_dir = workdir.join(".if2ai/skills");
    if skills_dir.exists() {
        let scanner = SkillCommands::new(&skills_dir);
        if let Ok(commands) = scanner.scan() {
            if let Some(info) = commands.get(skill_key) {
                return Some(info.clone());
            }
            // Try normalize match
            for info in commands.values() {
                if crate::modules::skills::commands::normalize_command_key(&info.name) == skill_key
                {
                    return Some(info.clone());
                }
            }
        }
    }

    // Check other roots (user-level, builtin)
    let roots = crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata(workdir);
    for root in &roots {
        if root.source.as_label() == "remote-quarantine" {
            continue;
        }
        let scanner = SkillCommands::new(&root.path);
        if let Ok(commands) = scanner.scan() {
            if let Some(info) = commands.get(skill_key) {
                return Some(info.clone());
            }
            for info in commands.values() {
                if crate::modules::skills::commands::normalize_command_key(&info.name) == skill_key
                {
                    return Some(info.clone());
                }
            }
        }
    }

    None
}

/// Check if a skill's required toolsets are satisfied.
///
/// Returns (satisfied: bool, fallback_satisfied: bool, message: String).
/// If satisfied is true, the skill can activate normally.
/// If only fallback_satisfied is true, the skill can activate with fallback tools.
/// If neither is true, returns an error message.
fn check_skill_toolset_availability(
    skill_info: &SkillCommandInfo,
    available: &[String],
) -> (bool, bool, String) {
    if skill_info.requires_toolsets.is_empty() {
        return (true, true, String::new());
    }

    let available_set: HashSet<&str> = available.iter().map(|s| s.as_str()).collect();

    // Check if all required toolsets are satisfied
    let all_satisfied = skill_info
        .requires_toolsets
        .iter()
        .all(|ts| available_set.contains(ts.as_str()));

    if all_satisfied {
        return (true, true, String::new());
    }

    // Check if fallback can substitute
    let fallback_set: HashSet<&str> = skill_info
        .fallback_for_toolsets
        .iter()
        .map(|s| s.as_str())
        .collect();

    let fallback_satisfies = skill_info
        .requires_toolsets
        .iter()
        .all(|ts| available_set.contains(ts.as_str()) || fallback_set.contains(ts.as_str()));

    if fallback_satisfies {
        let msg = format!(
            "[TOOLSET FALLBACK] Skill '{}' using fallback for toolsets: {:?}",
            skill_info.name, skill_info.requires_toolsets
        );
        (false, true, msg)
    } else {
        let missing: Vec<&str> = skill_info
            .requires_toolsets
            .iter()
            .filter(|ts| !available_set.contains(ts.as_str()))
            .map(|s| s.as_str())
            .collect();
        let msg = format!(
            "[TOOLSET UNAVAILABLE] Skill '{}' requires toolsets {:?} which are not available. Missing: {:?}",
            skill_info.name, skill_info.requires_toolsets, missing
        );
        (false, false, msg)
    }
}

// ── Builtin slash command specs ───────────────────────────────────────────────

/// Static list of builtin slash command specs.
fn builtin_specs() -> Vec<SlashCommandSpecDto> {
    vec![
        SlashCommandSpecDto {
            name: "/help".into(),
            description: "Show available slash commands".into(),
            category: "utility".into(),
        },
        SlashCommandSpecDto {
            name: "/clear".into(),
            description: "Clear the conversation history".into(),
            category: "session".into(),
        },
        SlashCommandSpecDto {
            name: "/skills".into(),
            description: "List available skills".into(),
            category: "utility".into(),
        },
        SlashCommandSpecDto {
            name: "/agents".into(),
            description: "List available agents".into(),
            category: "utility".into(),
        },
    ]
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

/// Parse a slash command input and return whether it matches a known command.
#[tauri::command]
#[allow(dead_code)]
pub fn parse_slash_command(input: String) -> SlashCommandParseResult {
    let specs = builtin_specs();
    let name = input.split_whitespace().next().unwrap_or("").to_string();
    let matched = specs.iter().any(|s| s.name == name);
    SlashCommandParseResult { name, matched }
}

/// List all registered slash commands.
#[tauri::command]
#[allow(dead_code)]
pub fn list_slash_commands() -> Vec<SlashCommandSpecDto> {
    builtin_specs()
}

/// Suggest slash commands matching the input prefix.
#[tauri::command]
#[allow(dead_code)]
pub fn suggest_slash_commands(input: String, limit: Option<usize>) -> Vec<String> {
    let specs = builtin_specs();
    let lim = limit.unwrap_or(10);
    specs
        .iter()
        .filter(|s| s.name.starts_with(&input))
        .take(lim)
        .map(|s| s.name.clone())
        .collect()
}

/// Execute a slash command and return the result message.
#[tauri::command]
#[allow(dead_code)]
pub fn execute_slash_command(
    state: State<'_, AppState>,
    input: String,
    _session_id: String,
) -> Result<String, String> {
    let name = input.split_whitespace().next().unwrap_or("").to_string();

    match name.as_str() {
        "/help" => {
            let specs = list_slash_commands();
            let lines: Vec<String> = specs
                .iter()
                .map(|s| format!("{} — {}", s.name, s.description))
                .collect();
            Ok(lines.join("\n"))
        }
        "/clear" => {
            // Frontend should clear the session UI; backend returns confirmation.
            Ok("Conversation cleared.".to_string())
        }
        "/skills" => {
            let workdir = std::env::current_dir().unwrap_or_default();
            handle_skills_command(&workdir, &input)
        }
        "/agents" => {
            let agents = list_agents_impl(&std::env::current_dir().unwrap_or_default());
            if agents.is_empty() {
                Ok("No agents found.".to_string())
            } else {
                let lines: Vec<String> = agents
                    .iter()
                    .map(|a| format!("{} — {}", a.name, a.description))
                    .collect();
                Ok(lines.join("\n"))
            }
        }
        _ => {
            // Try to resolve as a skill slash command: /skill-name [instruction]
            let workdir = std::env::current_dir().unwrap_or_default();
            let skill_key = name.trim_start_matches('/').to_lowercase();

            // Find skill info for toolset validation
            if let Some(skill_info) = find_skill_info(&workdir, &skill_key) {
                // Check toolset availability
                let available = available_toolsets(&state.tool_registry);
                let (satisfied, fallback_satisfied, msg) =
                    check_skill_toolset_availability(&skill_info, &available);

                if !satisfied && !fallback_satisfied {
                    // Skill requires toolsets that are not available
                    return Err(msg);
                }

                // Build invocation
                match resolve_skill_slash_invocation(&workdir, &input) {
                    Some(mut invocation) => {
                        if !msg.is_empty() {
                            // Prepend warning message
                            invocation = format!("{}\n\n{}", msg, invocation);
                        }
                        Ok(invocation)
                    }
                    None => Err(format!("Unknown command: {name}")),
                }
            } else {
                match resolve_skill_slash_invocation(&workdir, &input) {
                    Some(invocation) => Ok(invocation),
                    None => Err(format!("Unknown command: {name}")),
                }
            }
        }
    }
}

/// Build a skill invocation message from a `/skill-name [instruction]` input.
///
/// Returns `None` if the command does not match any installed skill.
/// Returns `Some(invocation_message)` if matched — the caller should inject this
/// as a user message into the agent conversation.
#[tauri::command]
#[allow(dead_code)]
pub fn resolve_skill_slash(cwd: Option<String>, input: String) -> Option<String> {
    let workdir = cwd
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    resolve_skill_slash_invocation(&workdir, &input)
}

/// Internal helper: resolve `/skill-name instruction` to a built invocation message.
fn resolve_skill_slash_invocation(workdir: &std::path::Path, input: &str) -> Option<String> {
    if !input.starts_with('/') {
        return None;
    }
    let mut parts = input.splitn(2, char::is_whitespace);
    let command = parts.next()?;
    let instruction = parts.next().unwrap_or("").trim();

    // Strip leading slash and check if this matches a skill
    let skill_name = command.trim_start_matches('/');
    if skill_name.is_empty() {
        return None;
    }

    // Scan skills directory for a match
    let skills_dir = workdir.join(".if2ai/skills");
    let scanner = SkillCommands::new(&skills_dir);
    if let Ok(commands) = scanner.scan() {
        if commands.contains_key(skill_name)
            || commands.contains_key(&skill_name.replace('-', "_"))
            || commands.values().any(|info| {
                crate::modules::skills::commands::normalize_command_key(&info.name) == skill_name
            })
        {
            return scanner
                .build_invocation_message(skill_name, instruction)
                .ok();
        }
    }

    // Also check other roots (user-level, builtin)
    let roots = crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata(workdir);
    for root in &roots {
        if root.source.as_label() == "remote-quarantine" {
            continue;
        }
        let scanner = SkillCommands::new(&root.path);
        if let Ok(commands) = scanner.scan() {
            if commands.contains_key(skill_name) {
                return scanner
                    .build_invocation_message(skill_name, instruction)
                    .ok();
            }
        }
    }

    None
}

/// List available skills discovered in skill directories.
#[tauri::command]
#[allow(dead_code)]
pub fn list_skills(cwd: Option<String>) -> Vec<SkillInfo> {
    let workdir = cwd
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    list_skills_impl(&workdir)
}

fn list_skills_impl(workdir: &std::path::Path) -> Vec<SkillInfo> {
    let roots = crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata(workdir);
    let mut skills = Vec::new();
    let state = load_skill_state(workdir);
    let mut winners: std::collections::BTreeMap<String, (usize, String)> =
        std::collections::BTreeMap::new();
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root.path) {
            let mut sorted_entries: Vec<std::fs::DirEntry> = entries.flatten().collect();
            sorted_entries.sort_by_key(|item| item.path());
            for entry in sorted_entries {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.is_file() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let key = name.to_lowercase();
                    let review_status = read_skill_status(&entry.path(), root.source);
                    let default_enabled =
                        review_status == "active" || review_status == "review_passed";
                    let path_key = entry.path().to_string_lossy().into_owned();
                    let enabled = state
                        .get(&path_key)
                        .copied()
                        .or_else(|| state.get(&key).copied())
                        .unwrap_or(default_enabled);
                    let shadowed_by = winners.get(&key).and_then(|(winner_rank, winner_label)| {
                        if *winner_rank <= root.precedence {
                            Some(winner_label.clone())
                        } else {
                            None
                        }
                    });
                    if !winners.contains_key(&key)
                        || winners
                            .get(&key)
                            .is_some_and(|(rank, _)| root.precedence < *rank)
                    {
                        winners.insert(
                            key.clone(),
                            (
                                root.precedence,
                                format!("{}@{}", name, root.source.as_label()),
                            ),
                        );
                    }
                    skills.push(SkillInfo {
                        name,
                        description: String::new(),
                        path: entry.path().to_string_lossy().into_owned(),
                        source: root.source.as_label().to_string(),
                        review_status: review_status.clone(),
                        status: if !enabled
                            && (review_status == "active" || review_status == "review_passed")
                        {
                            "disabled".to_string()
                        } else {
                            review_status
                        },
                        enabled,
                        read_only: root.read_only,
                        shadowed_by,
                    });
                }
            }
        }
    }
    skills.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    skills
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillManifestLite {
    review: SkillReviewLite,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillReviewLite {
    status: crate::modules::tools::registry::SkillReviewStatus,
}

fn read_skill_status(
    skill_dir: &std::path::Path,
    source: crate::modules::tools::builtin::skill::SkillSource,
) -> String {
    let manifest = skill_dir.join("skill.json");
    let Ok(raw) = std::fs::read_to_string(manifest) else {
        if let Some(status) =
            crate::modules::tools::builtin::skill::fallback_review_status_without_manifest(
                source, skill_dir,
            )
        {
            return status.to_string();
        }
        return "draft".to_string();
    };
    serde_json::from_str::<SkillManifestLite>(&raw)
        .map(|manifest| match manifest.review.status {
            crate::modules::tools::registry::SkillReviewStatus::Draft => "draft".to_string(),
            crate::modules::tools::registry::SkillReviewStatus::Quarantine => {
                "quarantine".to_string()
            }
            crate::modules::tools::registry::SkillReviewStatus::ReviewPassed => {
                "review_passed".to_string()
            }
            crate::modules::tools::registry::SkillReviewStatus::Active => "active".to_string(),
            crate::modules::tools::registry::SkillReviewStatus::Disabled => "disabled".to_string(),
        })
        .unwrap_or_else(|_| "draft".to_string())
}

fn skill_state_path(workdir: &std::path::Path) -> std::path::PathBuf {
    workdir.join(".if2ai").join("skill-state.json")
}

fn load_skill_state(workdir: &std::path::Path) -> std::collections::BTreeMap<String, bool> {
    let path = skill_state_path(workdir);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return std::collections::BTreeMap::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_skill_state(
    workdir: &std::path::Path,
    state: &std::collections::BTreeMap<String, bool>,
) -> Result<(), String> {
    let path = skill_state_path(workdir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create state dir failed: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| format!("write skill state failed: {e}"))?;
    Ok(())
}

fn handle_skills_command(workdir: &std::path::Path, input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if let Some(args) = trimmed.strip_prefix("/skills validate-remote-path ") {
        return validate_remote_distribution_path(workdir, args);
    }
    if let Some(name) = trimmed.strip_prefix("/skills create ") {
        return create_skill_draft(workdir, name.trim());
    }
    if let Some(target_path) = trimmed.strip_prefix("/skills review-path ") {
        return review_skill_path(workdir, target_path.trim());
    }
    if let Some(target_path) = trimmed.strip_prefix("/skills approve-path ") {
        return approve_skill_path(workdir, target_path.trim());
    }
    if let Some(target_path) = trimmed.strip_prefix("/skills rollback-path ") {
        return rollback_skill_path(workdir, target_path.trim());
    }
    if let Some(target_path) = trimmed.strip_prefix("/skills enable-path ") {
        return set_skill_enabled(workdir, target_path.trim(), true, true);
    }
    if let Some(target_path) = trimmed.strip_prefix("/skills disable-path ") {
        return set_skill_enabled(workdir, target_path.trim(), false, true);
    }
    let mut tokens = input.split_whitespace();
    let _ = tokens.next();
    let action = tokens.next();
    let target = tokens.next();
    match (action, target) {
        (Some("enable"), Some(name)) => set_skill_enabled(workdir, name, true, false),
        (Some("disable"), Some(name)) => set_skill_enabled(workdir, name, false, false),
        _ => {
            let skills = list_skills_impl(workdir);
            if skills.is_empty() {
                Ok("📭 No skills found. Add skills from Settings > Skills Market.".to_string())
            } else {
                let total = skills.len();
                let enabled_count = skills.iter().filter(|s| s.enabled).count();
                let header = format!(
                    "📋 Skills ({} total, {} enabled)\n{}\n",
                    total,
                    enabled_count,
                    "─".repeat(40)
                );
                let lines: Vec<String> = skills
                    .iter()
                    .map(|s| {
                        let status_icon = match s.status.as_str() {
                            "active" | "review_passed" => "✅",
                            "quarantine" => "🔒",
                            "draft" => "📝",
                            "disabled" => "❌",
                            _ => "⚠️",
                        };
                        let enabled_icon = if s.enabled { "🟢" } else { "⚪" };
                        let shadow_info = s
                            .shadowed_by
                            .as_ref()
                            .map_or(String::new(), |by| format!("\n   └ shadowed by {}", by));
                        let source_label = match s.source.as_str() {
                            "builtin" => "🏠 builtin",
                            "user" => "👤 user",
                            "workspace" => "📁 workspace",
                            "remote-quarantine" => "🔒 quarantine",
                            other => other,
                        };
                        format!(
                            "{}{} {} {}\n   └ {} | status: {}{}",
                            enabled_icon,
                            status_icon,
                            s.name,
                            source_label,
                            if s.enabled { "enabled" } else { "disabled" },
                            s.status,
                            shadow_info
                        )
                    })
                    .collect();
                Ok(format!("{}{}", header, lines.join("\n\n")))
            }
        }
    }
}

fn validate_remote_distribution_path(
    workdir: &std::path::Path,
    args: &str,
) -> Result<String, String> {
    let mut parts = args.split_whitespace();
    let channel = parts.next().ok_or_else(|| "missing channel".to_string())?;
    let checksum = parts.next().ok_or_else(|| "missing checksum".to_string())?;
    let signature = parts
        .next()
        .ok_or_else(|| "missing signature".to_string())?;
    let skill_path = parts
        .next()
        .ok_or_else(|| "missing skill path".to_string())?;
    let channel = match channel {
        "stable" => SkillDistributionChannel::Stable,
        "canary" => SkillDistributionChannel::Canary,
        other => return Err(format!("unsupported channel: {other}")),
    };
    let canonical_workdir = workdir
        .canonicalize()
        .map_err(|e| format!("invalid workspace path: {e}"))?;
    let canonical_skill = std::path::PathBuf::from(skill_path)
        .canonicalize()
        .map_err(|e| format!("invalid skill artifact path: {e}"))?;
    if !canonical_skill.starts_with(&canonical_workdir) {
        return Err("artifact path outside workspace".to_string());
    }
    let content = std::fs::read_to_string(&canonical_skill)
        .map_err(|e| format!("read skill artifact failed: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    hasher.update(content.as_bytes());
    let digest = hasher.finalize();
    let actual_checksum = hex::encode(digest);
    if actual_checksum != checksum {
        return Err(format!(
            "checksum mismatch: expected {checksum}, got {actual_checksum}"
        ));
    }
    let envelope = SkillDistributionEnvelope {
        channel,
        checksum: actual_checksum.clone(),
        signature: signature.to_string(),
        quarantine: true,
    };
    validate_distribution_envelope(&envelope).map_err(ToString::to_string)?;
    let trace_id = AuditEmitter::new_trace_id();
    AuditEmitter::skill_distribution(
        &trace_id,
        "slash.skills",
        "remote-distribution",
        workdir,
        SkillDistributionDiagnostic {
            channel: match channel {
                SkillDistributionChannel::Stable => "stable",
                SkillDistributionChannel::Canary => "canary",
            },
            checksum: &actual_checksum,
            signature,
            request_id: None,
        },
    );
    Ok("distribution artifact verified".to_string())
}

fn set_skill_enabled(
    workdir: &std::path::Path,
    target: &str,
    enable: bool,
    by_path: bool,
) -> Result<String, String> {
    let skills = list_skills_impl(workdir);
    let selected = if by_path {
        skills
            .iter()
            .find(|item| item.path == target)
            .ok_or_else(|| format!("Skill path '{target}' not found"))?
    } else {
        skills
            .iter()
            .find(|item| item.name.eq_ignore_ascii_case(target))
            .ok_or_else(|| format!("Skill '{target}' not found"))?
    };
    if enable && !(selected.review_status == "active" || selected.review_status == "review_passed")
    {
        return Err(format!(
            "Skill '{}' cannot be enabled because status is '{}'",
            selected.name, selected.review_status
        ));
    }
    let mut state = load_skill_state(workdir);
    state.insert(selected.path.clone(), enable);
    save_skill_state(workdir, &state)?;
    Ok(format!(
        "Skill '{}' {}",
        selected.name,
        if enable { "enabled" } else { "disabled" }
    ))
}

fn create_skill_draft(workdir: &std::path::Path, name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("skill name is required".to_string());
    }
    let safe_name = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let skill_dir = workdir.join(".if2ai/skills").join(&safe_name);
    std::fs::create_dir_all(&skill_dir).map_err(|e| format!("create skill dir failed: {e}"))?;
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.is_file() {
        std::fs::write(
            &skill_md,
            format!(
                "---\nname: {safe_name}\ndescription: Draft skill\n---\n# {safe_name}\n\nWrite your skill here.\n"
            ),
        )
        .map_err(|e| format!("write SKILL.md failed: {e}"))?;
    }
    let manifest = skill_dir.join("skill.json");
    let manifest_json = serde_json::json!({
      "id": safe_name,
      "version": "0.1.0",
      "apiVersion": "v1",
      "minAppVersion": "0.1.0",
      "capabilities": ["custom"],
      "review": {
        "status": "draft",
        "riskLevel": "medium",
        "lastReviewedAt": ""
      }
    });
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write skill.json failed: {e}"))?;
    let envelope = SkillDistributionEnvelope {
        channel: SkillDistributionChannel::Stable,
        checksum: "local-draft".to_string(),
        signature: "local-draft".to_string(),
        quarantine: true,
    };
    validate_distribution_envelope(&envelope).map_err(ToString::to_string)?;
    let trace_id = AuditEmitter::new_trace_id();
    AuditEmitter::skill_distribution(
        &trace_id,
        "slash.skills",
        &safe_name,
        workdir,
        SkillDistributionDiagnostic {
            channel: "stable",
            checksum: "local-draft",
            signature: "local-draft",
            request_id: None,
        },
    );
    Ok(format!("create skill draft: {}", skill_dir.display()))
}

fn review_skill_path(workdir: &std::path::Path, target_path: &str) -> Result<String, String> {
    let full_path = std::path::PathBuf::from(target_path)
        .canonicalize()
        .map_err(|e| format!("invalid skill path: {e}"))?;
    if !is_trusted_skill_root_path(&full_path, workdir) {
        return Err("review blocked: path outside trusted skill roots".to_string());
    }
    let skill_md = full_path.join("SKILL.md");
    let raw =
        std::fs::read_to_string(&skill_md).map_err(|e| format!("read SKILL.md failed: {e}"))?;
    let review_result = crate::modules::tools::builtin::skill::local_review_skill_content(&raw);
    let status = if review_result.is_ok() {
        "review_passed"
    } else {
        "quarantine"
    };
    let manifest = full_path.join("skill.json");
    let mut manifest_json = if manifest.is_file() {
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(&manifest)
                .map_err(|e| format!("read skill.json failed: {e}"))?,
        )
        .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    manifest_json["review"] = serde_json::json!({
      "status": status,
      "riskLevel": if review_result.is_ok() { "low" } else { "high" },
      "lastReviewedAt": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write skill.json failed: {e}"))?;
    match review_result {
        Ok(()) => Ok("review: pass".to_string()),
        Err(reason) => Ok(format!("review: blocked ({reason})")),
    }
}

fn approve_skill_path(workdir: &std::path::Path, target_path: &str) -> Result<String, String> {
    let canonical_skill = std::path::PathBuf::from(target_path)
        .canonicalize()
        .map_err(|e| format!("invalid skill path: {e}"))?;
    if !is_trusted_skill_root_path(&canonical_skill, workdir) {
        return Err("approval blocked: path outside trusted skill roots".to_string());
    }
    crate::modules::tools::builtin::skill::approve_skill_proposal(&canonical_skill)?;
    let trace_id = AuditEmitter::new_trace_id();
    AuditEmitter::skill_enable(
        &trace_id,
        "slash.skills",
        "skill_proposal_approval",
        workdir,
        "adoption",
        None,
    );
    Ok("approval: promoted to active".to_string())
}

fn rollback_skill_path(workdir: &std::path::Path, target_path: &str) -> Result<String, String> {
    let canonical_skill = std::path::PathBuf::from(target_path)
        .canonicalize()
        .map_err(|e| format!("invalid skill path: {e}"))?;
    if !is_trusted_skill_root_path(&canonical_skill, workdir) {
        return Err("rollback blocked: path outside trusted skill roots".to_string());
    }
    crate::modules::tools::builtin::skill::rollback_skill_proposal(&canonical_skill)?;
    let trace_id = AuditEmitter::new_trace_id();
    AuditEmitter::skill_enable(
        &trace_id,
        "slash.skills",
        "skill_proposal_rollback",
        workdir,
        "rollback",
        None,
    );
    Ok("rollback: moved to quarantine".to_string())
}

/// Returns `true` when `skill_path` falls under a known, writable skill root.
///
/// Accepts paths within:
/// - workspace-level `.if2ai/skills`, `.if2ai/skills-quarantine`, `.claw/skills`, `.codex/skills`
/// - user-level `$HOME/.if2ai/skills` and `$HOME/.if2ai/skills-quarantine`
/// - the `IF2AI_HOME` custom override directory
///
/// This replaces the old `starts_with(workdir)` check that incorrectly blocked
/// user-level skills stored under `~/.if2ai/skills/`.
fn is_trusted_skill_root_path(skill_path: &std::path::Path, workdir: &std::path::Path) -> bool {
    let mut allowed_roots: Vec<std::path::PathBuf> = Vec::new();

    // Workspace-level skill directories
    for rel in &[
        ".if2ai/skills",
        ".if2ai/skills-quarantine",
        ".if2ai/skills-proposals",
        ".claw/skills",
        ".codex/skills",
    ] {
        allowed_roots.push(workdir.join(rel));
    }

    // User-level: $HOME/.if2ai/skills
    if let Ok(home) = std::env::var("HOME") {
        let base = std::path::PathBuf::from(&home).join(".if2ai");
        allowed_roots.push(base.join("skills"));
        allowed_roots.push(base.join("skills-quarantine"));
        allowed_roots.push(base.join("skills-proposals"));
    }

    // User-level: $IF2AI_HOME override
    if let Ok(h) = std::env::var("IF2AI_HOME") {
        let base = std::path::PathBuf::from(&h);
        allowed_roots.push(base.join("skills"));
        allowed_roots.push(base.join("skills-quarantine"));
    }

    allowed_roots.iter().any(|root| {
        root.canonicalize()
            .map(|canonical_root| skill_path.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

/// List available agents discovered in agent directories.
#[tauri::command]
#[allow(dead_code)]
pub fn list_agents(cwd: Option<String>) -> Vec<AgentInfo> {
    let workdir = cwd
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    list_agents_impl(&workdir)
}

fn list_agents_impl(workdir: &std::path::Path) -> Vec<AgentInfo> {
    let mut agents = Vec::new();

    // Search .if2ai/agents/ and .claude/agents/
    for dir in &[".if2ai/agents", ".claude/agents"] {
        let agents_dir = workdir.join(dir);
        if agents_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&agents_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        agents.push(AgentInfo {
                            name: entry.file_name().to_string_lossy().into_owned(),
                            description: String::new(),
                            path: entry.path().to_string_lossy().into_owned(),
                        });
                    }
                }
            }
        }
    }

    agents
}

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
    let mut specs = vec![
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
    ];
    specs.extend(git_slash_specs());
    specs
}

/// Git-related slash commands; kept beside the builtin list so the
/// frontend's completion menu and the dispatcher in
/// [`crate::modules::git::slash`] can never drift out of sync.
fn git_slash_specs() -> Vec<SlashCommandSpecDto> {
    [
        ("/branch", "List, create, or switch git branches"),
        ("/worktree", "List, add, remove, or prune git worktrees"),
        ("/diff", "Show staged and unstaged diffs"),
        (
            "/commit",
            "Stage all changes and create a commit (caller supplies the message)",
        ),
        (
            "/commit-push-pr",
            "Commit, push the branch, and open a pull request via gh",
        ),
        (
            "/pr",
            "Open a pull request via gh; falls back to a draft when gh is missing",
        ),
        (
            "/issue",
            "Open a GitHub issue via gh; falls back to a draft when gh is missing",
        ),
    ]
    .into_iter()
    .map(|(name, description)| SlashCommandSpecDto {
        name: name.into(),
        description: description.into(),
        category: "git".into(),
    })
    .collect()
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
///
/// Returns builtin commands first, then skill-based slash commands discovered
/// from all known skill roots.
#[tauri::command]
#[allow(dead_code)]
pub fn suggest_slash_commands(input: String, limit: Option<usize>) -> Vec<String> {
    let lim = limit.unwrap_or(10);
    let mut results: Vec<String> = builtin_specs()
        .iter()
        .filter(|s| s.name.starts_with(&input))
        .map(|s| s.name.clone())
        .collect();

    if results.len() < lim {
        let workdir = std::env::current_dir().unwrap_or_default();
        let roots =
            crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata(&workdir);
        let mut seen: std::collections::HashSet<String> = results.iter().cloned().collect();
        'outer: for root in &roots {
            if let Ok(entries) = std::fs::read_dir(&root.path) {
                for entry in entries.flatten() {
                    if results.len() >= lim {
                        break 'outer;
                    }
                    let skill_name = entry.file_name().to_string_lossy().to_string();
                    let candidate = format!("/{skill_name}");
                    if candidate.starts_with(&input) && seen.insert(candidate.clone()) {
                        // Only suggest if a SKILL.md actually exists
                        if entry.path().join("SKILL.md").exists() {
                            results.push(candidate);
                        }
                    }
                }
            }
        }
    }
    results
}

/// Resolve the active project's workdir for a given session.
///
/// Resolution order:
/// 1. Restore the session via `session_manager`; on failure fall back to
///    the host-process CWD (legacy behaviour).
/// 2. If `session.project_id` is non-empty, look up the project via
///    `project_manager` and use its `workdir`.
/// 3. Otherwise fall back to the host-process CWD.
///
/// This is the canonical way for the slash IPC surface to find the
/// "current project root", so that `/branch`, `/diff`, `/skills`, ... in a
/// session bound to project A don't accidentally inspect the bundle
/// directory of the if2ai host app.
async fn resolve_session_workdir(state: &AppState, session_id: &str) -> std::path::PathBuf {
    resolve_session_workdir_inner(&state.session_manager, &state.project_manager, session_id).await
}

/// Pure variant of [`resolve_session_workdir`] that takes the two
/// managers it actually uses, so unit tests can stand it up against
/// real `SessionManager` / `ProjectManager` instances backed by tmp
/// dirs without having to construct the full `AppState`.
async fn resolve_session_workdir_inner(
    session_manager: &std::sync::Arc<crate::modules::session::SessionManager>,
    project_manager: &std::sync::Arc<crate::modules::projects::ProjectManager>,
    session_id: &str,
) -> std::path::PathBuf {
    let fallback = || std::env::current_dir().unwrap_or_default();
    if session_id.is_empty() {
        return fallback();
    }
    let session = match session_manager.restore_session(session_id).await {
        Ok(session) => session,
        Err(_) => return fallback(),
    };
    if session.project_id.is_empty() {
        return fallback();
    }
    match project_manager.get_project(&session.project_id).await {
        Ok(project) => project.workdir,
        Err(_) => fallback(),
    }
}

/// Execute a slash command and return the result message.
#[tauri::command]
#[allow(dead_code)]
pub async fn execute_slash_command(
    state: State<'_, AppState>,
    input: String,
    session_id: String,
) -> Result<String, String> {
    let name = input.split_whitespace().next().unwrap_or("").to_string();
    // Resolve once and reuse for every dispatcher below.  Any branch that
    // previously called `std::env::current_dir()` would otherwise pick up
    // the Tauri host-process CWD (the if2ai bundle dir), not the active
    // project's workdir — which is exactly the bug we are fixing here.
    let workdir = resolve_session_workdir(&state, &session_id).await;

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
        "/skills" => handle_skills_command(&workdir, &input),
        "/agents" => {
            let agents = list_agents_impl(&workdir);
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
            // First: try git slash commands (`/branch`, `/worktree`, ...)
            // against the resolved project workdir.  The typed git IPC under
            // `crate::commands::git` accepts an explicit `cwd` from the
            // frontend; this fallback path now mirrors that behaviour by
            // using `resolve_session_workdir` instead of the host CWD.
            if let Some(rendered) = try_dispatch_git_slash(&workdir, &input) {
                return rendered;
            }

            // Try to resolve as a skill slash command: /skill-name [instruction]
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

/// Attempt to handle `input` as a git slash command (`/branch`,
/// `/worktree`, `/diff`, `/commit`, `/commit-push-pr`, `/pr`, `/issue`).
///
/// Returns:
/// - `Some(Ok(text))` — git command handled, text is the user-facing
///   response.
/// - `Some(Err(message))` — git command attempted but failed; message
///   is the structured `GitError::Display` rendering.
/// - `None` — `input` is not a git slash command; caller should fall
///   through to its other dispatchers.
///
/// `commit` / `commit-push-pr` / `pr` / `issue` need additional
/// fields (commit message, PR title, branch hint) that the legacy
/// frontend slash IPC does not transport.  When called via the legacy
/// surface those commands return a usage hint pointing to the typed
/// IPC under `crate::commands::git` (added by the `ipc` TODO of the
/// migration plan).
///
/// **CWD**: callers must pass the active project's workdir as `cwd`.
/// In [`execute_slash_command`] this is resolved from the incoming
/// `session_id` via [`resolve_session_workdir`], so the dispatcher
/// runs against the project the user actually has open — not the
/// host-process CWD (which on a packaged Tauri build is the if2ai
/// bundle directory and would otherwise leak into every git command).
fn try_dispatch_git_slash(cwd: &std::path::Path, input: &str) -> Option<Result<String, String>> {
    use crate::modules::git::slash::{dispatch, known_command_names, SlashRequest};

    let mut tokens = input.split_whitespace();
    let raw_name = tokens.next()?;
    let trimmed = raw_name.trim_start_matches('/').to_ascii_lowercase();
    if !known_command_names().contains(&trimmed.as_str()) {
        return None;
    }

    let args: Vec<&str> = tokens.collect();
    let needs_message = matches!(
        trimmed.as_str(),
        "commit" | "commit-push-pr" | "pr" | "issue"
    );
    if needs_message {
        return Some(Ok(format!(
            "/{trimmed} requires a message (and title for PR/Issue) — call the typed git IPC under `commands::git` from the frontend instead."
        )));
    }

    let request = SlashRequest {
        name: trimmed.as_str(),
        args,
        message: None,
        title: None,
        branch_hint: None,
        cwd,
    };
    let result = dispatch(&request);
    Some(match result {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err(format!("Unknown git slash command: /{trimmed}")),
        Err(err) => Err(err.to_string()),
    })
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

fn resolve_skill_command_key(
    commands: &std::collections::HashMap<String, SkillCommandInfo>,
    input_skill: &str,
) -> Option<String> {
    let normalized = crate::modules::skills::commands::normalize_command_key(input_skill);
    if commands.contains_key(&normalized) {
        return Some(normalized);
    }
    commands.values().find_map(|info| {
        let by_name = crate::modules::skills::commands::normalize_command_key(&info.name);
        if by_name == normalized {
            Some(by_name)
        } else {
            None
        }
    })
}

/// Internal helper: resolve `/skill-name instruction` to a Hermes-style
/// invocation message that embeds the full skill content server-side.
fn resolve_skill_slash_invocation(workdir: &std::path::Path, input: &str) -> Option<String> {
    if !input.starts_with('/') {
        return None;
    }
    let mut parts = input.splitn(2, char::is_whitespace);
    let command = parts.next()?;
    let instruction = parts.next().unwrap_or("").trim();

    // Strip leading slash and check if this matches a skill.
    let skill_name = command.trim_start_matches('/');
    if skill_name.is_empty() {
        return None;
    }

    // Scan workspace skills directory for a match.
    let skills_dir = workdir.join(".if2ai/skills");
    let scanner = SkillCommands::new(&skills_dir);
    if let Ok(commands) = scanner.scan() {
        if let Some(command_key) = resolve_skill_command_key(&commands, skill_name) {
            return scanner
                .build_invocation_message(&command_key, instruction)
                .ok();
        }
    }

    // Also check other roots (user-level, builtin).
    let roots = crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata(workdir);
    for root in &roots {
        if root.source.as_label() == "remote-quarantine" {
            continue;
        }
        let scanner = SkillCommands::new(&root.path);
        if let Ok(commands) = scanner.scan() {
            if let Some(command_key) = resolve_skill_command_key(&commands, skill_name) {
                return scanner
                    .build_invocation_message(&command_key, instruction)
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

#[cfg(test)]
mod tests {
    //! CWD regression suite — prevents the `/branch` etc. slash dispatch
    //! from regressing back to using the host-process CWD instead of
    //! the active session's project workdir.
    //!
    //! The fix lives in [`resolve_session_workdir_inner`]; before it
    //! existed every slash command saw `std::env::current_dir()` (the
    //! Tauri bundle directory), which silently leaked information
    //! across projects.  Each test below codifies one branch of that
    //! resolver so a future refactor can't quietly bring the bug back.
    use super::resolve_session_workdir_inner;
    use crate::modules::projects::ProjectManager;
    use crate::modules::session::SessionManager;
    use std::env::temp_dir;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uuid::Uuid;

    struct Fixture {
        sessions: Arc<SessionManager>,
        projects: Arc<ProjectManager>,
        sessions_root: PathBuf,
        projects_root: PathBuf,
    }

    impl Fixture {
        async fn new(label: &str) -> Self {
            let root = temp_dir().join(format!("if2ai_slash_cwd_{label}_{}", Uuid::new_v4()));
            let sessions_root = root.join("sessions");
            let projects_root = root.join("projects");
            tokio::fs::create_dir_all(&sessions_root).await.unwrap();
            tokio::fs::create_dir_all(&projects_root).await.unwrap();
            // SessionManager and ProjectManager must agree on the
            // project storage root so `restore_session` can find the
            // project entries the test seeded.
            let sessions = SessionManager::new(sessions_root.clone(), projects_root.clone());
            let projects = ProjectManager::new(projects_root.clone());
            Self {
                sessions: Arc::new(sessions),
                projects: Arc::new(projects),
                sessions_root,
                projects_root,
            }
        }

        async fn cleanup(self) {
            let _ = tokio::fs::remove_dir_all(&self.sessions_root).await;
            let _ = tokio::fs::remove_dir_all(&self.projects_root).await;
        }
    }

    #[tokio::test]
    async fn resolves_to_project_workdir_when_session_has_one() {
        let fx = Fixture::new("with-project").await;
        // Create a project at a real on-disk dir so canonicalize() works
        // for any downstream caller (the resolver itself doesn't
        // canonicalize but other layers do).
        let workdir = fx.sessions_root.parent().unwrap().join("repo");
        tokio::fs::create_dir_all(&workdir).await.unwrap();
        let project = fx
            .projects
            .create_project("Demo".to_string(), workdir.clone())
            .await
            .expect("create project");
        let session = fx
            .sessions
            .create_session_for_project(&project.id, "Test".to_string())
            .await
            .expect("create session");

        let resolved = resolve_session_workdir_inner(&fx.sessions, &fx.projects, &session.id).await;
        assert_eq!(
            resolved, workdir,
            "session bound to project should resolve to project workdir, not host CWD",
        );

        fx.cleanup().await;
    }

    #[tokio::test]
    async fn falls_back_to_cwd_when_session_id_empty() {
        let fx = Fixture::new("empty-id").await;
        let expected = std::env::current_dir().unwrap_or_default();
        let resolved = resolve_session_workdir_inner(&fx.sessions, &fx.projects, "").await;
        assert_eq!(
            resolved, expected,
            "empty session_id should fall back to process CWD",
        );
        fx.cleanup().await;
    }

    #[tokio::test]
    async fn falls_back_to_cwd_when_session_id_unknown() {
        let fx = Fixture::new("unknown-id").await;
        let expected = std::env::current_dir().unwrap_or_default();
        let resolved =
            resolve_session_workdir_inner(&fx.sessions, &fx.projects, "definitely-not-a-session")
                .await;
        assert_eq!(
            resolved, expected,
            "unknown session id must not crash; fall back to host CWD",
        );
        fx.cleanup().await;
    }

    #[tokio::test]
    async fn falls_back_to_cwd_when_session_has_no_project() {
        let fx = Fixture::new("no-project").await;
        let session = fx
            .sessions
            .create_session("Detached".to_string())
            .await
            .expect("create session");
        let expected = std::env::current_dir().unwrap_or_default();
        let resolved = resolve_session_workdir_inner(&fx.sessions, &fx.projects, &session.id).await;
        assert_eq!(
            resolved, expected,
            "session with empty project_id should fall back, not invoke get_project",
        );
        fx.cleanup().await;
    }
}

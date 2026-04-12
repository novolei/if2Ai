//! SlashCommand Tauri commands — F5 + F17.
//!
//! Exposes parse, list, suggest, execute, list_skills, list_agents
//! to the frontend via Tauri IPC.
//! See docs/bs_gap/08-critical-fix-priority.md §F5, §F17.

use serde::Serialize;
use tauri::State;

use crate::commands::AppState;

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
}

/// Info about a discovered agent.
#[derive(Serialize)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub path: String,
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
    _state: State<'_, AppState>,
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
            let skills = list_skills_impl(&std::env::current_dir().unwrap_or_default());
            if skills.is_empty() {
                Ok("No skills found.".to_string())
            } else {
                let lines: Vec<String> = skills
                    .iter()
                    .map(|s| format!("{} — {}", s.name, s.description))
                    .collect();
                Ok(lines.join("\n"))
            }
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
        _ => Err(format!("Unknown command: {name}")),
    }
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
    let roots = crate::modules::tools::builtin::skill::discover_skill_roots(workdir);
    let mut skills = Vec::new();
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.is_file() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    skills.push(SkillInfo {
                        name,
                        description: String::new(),
                        path: entry.path().to_string_lossy().into_owned(),
                    });
                }
            }
        }
    }
    skills
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

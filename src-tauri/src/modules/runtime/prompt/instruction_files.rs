//! Instruction-file discovery, dedup, render + git/project context helpers.
//!
//! Extracted from `runtime/prompt/mod.rs` in GFR-T1-G-1 (pure structural
//! move; function bodies byte-identical).

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{
    prepend_bullets, ContextFile, ProjectContext, MAX_INSTRUCTION_FILE_CHARS,
    MAX_TOTAL_INSTRUCTION_CHARS,
};

pub(super) fn discover_instruction_files(cwd: &Path) -> std::io::Result<Vec<ContextFile>> {
    let mut directories = Vec::new();
    let mut cursor = Some(cwd);
    while let Some(dir) = cursor {
        directories.push(dir.to_path_buf());
        cursor = dir.parent();
    }
    directories.reverse();

    let mut files = Vec::new();
    for dir in directories {
        for candidate in [
            dir.join("CLAW.md"),
            dir.join("CLAW.local.md"),
            dir.join(".claw").join("CLAW.md"),
            dir.join(".claw").join("instructions.md"),
        ] {
            push_context_file(&mut files, candidate)?;
        }
    }
    Ok(dedupe_instruction_files(files))
}

pub(super) fn push_context_file(
    files: &mut Vec<ContextFile>,
    path: PathBuf,
) -> std::io::Result<()> {
    match fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => {
            files.push(ContextFile { path, content });
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn read_git_status(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["--no-optional-locks", "status", "--short", "--branch"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn read_git_diff(cwd: &Path) -> Option<String> {
    let mut sections = Vec::new();

    let staged = read_git_output(cwd, &["diff", "--cached"])?;
    if !staged.trim().is_empty() {
        sections.push(format!("Staged changes:\n{}", staged.trim_end()));
    }

    let unstaged = read_git_output(cwd, &["diff"])?;
    if !unstaged.trim().is_empty() {
        sections.push(format!("Unstaged changes:\n{}", unstaged.trim_end()));
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

pub(super) fn read_git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

pub(super) fn render_project_context(project_context: &ProjectContext) -> String {
    let mut lines = vec!["# Project context".to_string()];
    let mut bullets = vec![
        format!("Today's date is {}.", project_context.current_date),
        format!("Working directory: {}", project_context.cwd.display()),
    ];
    if !project_context.instruction_files.is_empty() {
        bullets.push(format!(
            "Claw instruction files discovered: {}.",
            project_context.instruction_files.len()
        ));
    }
    lines.extend(prepend_bullets(bullets));
    if let Some(status) = &project_context.git_status {
        lines.push(String::new());
        lines.push("Git status snapshot:".to_string());
        lines.push(status.clone());
    }
    if let Some(diff) = &project_context.git_diff {
        lines.push(String::new());
        lines.push("Git diff snapshot:".to_string());
        lines.push(diff.clone());
    }
    lines.join("\n")
}

pub(super) fn render_instruction_files(files: &[ContextFile]) -> String {
    let mut sections = vec!["# Claw instructions".to_string()];
    let mut remaining_chars = MAX_TOTAL_INSTRUCTION_CHARS;
    for file in files {
        if remaining_chars == 0 {
            sections.push(
                "_Additional instruction content omitted after reaching the prompt budget._"
                    .to_string(),
            );
            break;
        }

        let raw_content = truncate_instruction_content(&file.content, remaining_chars);
        let rendered_content = render_instruction_content(&raw_content);
        let consumed = rendered_content.chars().count().min(remaining_chars);
        remaining_chars = remaining_chars.saturating_sub(consumed);

        sections.push(format!("## {}", describe_instruction_file(file, files)));
        sections.push(rendered_content);
    }
    sections.join("\n\n")
}

pub(super) fn dedupe_instruction_files(files: Vec<ContextFile>) -> Vec<ContextFile> {
    let mut deduped = Vec::new();
    let mut seen_hashes = Vec::new();

    for file in files {
        let normalized = normalize_instruction_content(&file.content);
        let hash = stable_content_hash(&normalized);
        if seen_hashes.contains(&hash) {
            continue;
        }
        seen_hashes.push(hash);
        deduped.push(file);
    }

    deduped
}

pub(super) fn normalize_instruction_content(content: &str) -> String {
    collapse_blank_lines(content).trim().to_string()
}

pub(super) fn stable_content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn describe_instruction_file(file: &ContextFile, files: &[ContextFile]) -> String {
    let path = display_context_path(&file.path);
    let scope = files
        .iter()
        .filter_map(|candidate| candidate.path.parent())
        .find(|parent| file.path.starts_with(parent))
        .map_or_else(
            || "workspace".to_string(),
            |parent| parent.display().to_string(),
        );
    format!("{path} (scope: {scope})")
}

pub(super) fn truncate_instruction_content(content: &str, remaining_chars: usize) -> String {
    let hard_limit = MAX_INSTRUCTION_FILE_CHARS.min(remaining_chars);
    let trimmed = content.trim();
    if trimmed.chars().count() <= hard_limit {
        return trimmed.to_string();
    }

    let mut output = trimmed.chars().take(hard_limit).collect::<String>();
    output.push_str("\n\n[truncated]");
    output
}

pub(super) fn render_instruction_content(content: &str) -> String {
    truncate_instruction_content(content, MAX_INSTRUCTION_FILE_CHARS)
}

pub(super) fn display_context_path(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(super) fn collapse_blank_lines(content: &str) -> String {
    let mut result = String::new();
    let mut previous_blank = false;
    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && previous_blank {
            continue;
        }
        result.push_str(line.trim_end());
        result.push('\n');
        previous_blank = is_blank;
    }
    result
}

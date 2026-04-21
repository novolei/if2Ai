//! Tool result heuristics — text-pattern detectors used to decide whether
//! a model claim was backed by an actual mutating tool call, or whether
//! a shell command likely modified files.
//!
//! Extracted from `commands/agent.rs` in GFR-006b (pure structural move,
//! function bodies byte-identical).

pub(crate) fn contains_unverified_file_claim(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "已创建",
        "已写入",
        "已删除",
        "已修改",
        "created",
        "written",
        "deleted",
        "successfully created",
        "successfully wrote",
        "successfully deleted",
    ];
    patterns
        .iter()
        .any(|p| text.contains(p) || lower.contains(p))
}

pub(crate) fn is_mutating_tool_success(
    tool_name: &str,
    input_json: &str,
    is_error: bool,
) -> bool {
    if is_error {
        return false;
    }

    let write_tools = [
        "file_write",
        "write_file",
        "file_edit",
        "edit_file",
        "NotebookEdit",
        "TodoWrite",
        "memory_store",
        "memory_forget",
        "memory_purge",
        "cron_add",
        "cron_remove",
        "cron_run",
    ];
    if write_tools.contains(&tool_name) {
        return true;
    }

    // Shell-like tools can mutate files; inspect command heuristically.
    if ["bash", "PowerShell", "REPL"].contains(&tool_name) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(input_json) {
            let command = value.get("command").and_then(|v| v.as_str()).unwrap_or("");
            return shell_command_likely_mutates_files(command);
        }
    }

    false
}

pub(crate) fn shell_command_likely_mutates_files(command: &str) -> bool {
    let normalized = command.to_lowercase();
    let mutation_markers = [
        "rm ",
        "mv ",
        "cp ",
        "touch ",
        "mkdir ",
        "rmdir ",
        "chmod ",
        "chown ",
        "sed -i",
        "perl -0pi",
        "python -c",
        "python - <<",
        "node -e",
        "tee ",
        "printf ",
        "cat >",
        "cat <<",
        "echo >",
        "echo >>",
        ">>",
        " > ",
        "| tee",
        "git add",
        "git mv",
        "git rm",
        "install -d",
    ];

    mutation_markers
        .iter()
        .any(|marker| normalized.contains(marker))
}

pub(crate) fn extract_skill_proposal_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("skill_proposal:"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
// harness symbol marker: skill_proposal|draft|approval

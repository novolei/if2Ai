//! Slash-command dispatch for the git module.
//!
//! Each user-visible command lives in its own file (`branch_cmd.rs`,
//! `worktree_cmd.rs`, `diff_cmd.rs`, `commit_cmd.rs`, `pr_cmd.rs`,
//! `issue_cmd.rs`, `commit_push_pr_cmd.rs`).  The frontend / IPC layer
//! parses the raw `/name args...` string, hands the name and remaining
//! tokens to [`dispatch`], and renders the returned text directly.
//!
//! Slash semantics intentionally diverge from the `claw-cli` reference
//! library: this layer **does not** call the LLM to generate commit
//! messages or PR titles.  Higher-level orchestration (`turn_service`,
//! the desktop UI) is responsible for that and supplies the text.

use std::path::Path;

use super::error::GitResult;

pub(crate) mod branch_cmd;
pub(crate) mod commit_cmd;
pub(crate) mod commit_push_pr_cmd;
pub(crate) mod diff_cmd;
pub(crate) mod issue_cmd;
pub(crate) mod pr_cmd;
pub(crate) mod worktree_cmd;

/// Inputs to a slash command request.
#[derive(Debug, Clone)]
pub(crate) struct SlashRequest<'a> {
    /// Command name without the leading slash, lower-cased.
    pub(crate) name: &'a str,
    /// Whitespace-tokenised positional arguments.
    pub(crate) args: Vec<&'a str>,
    /// `commit` / `commit-push-pr` / `pr` / `issue` need a message body
    /// supplied by the caller; absent for inspection-only commands.
    pub(crate) message: Option<&'a str>,
    /// PR / issue title for `pr_cmd` / `issue_cmd` / `commit_push_pr_cmd`.
    pub(crate) title: Option<&'a str>,
    /// Context hint passed through to commit-push-pr branch naming
    /// when the user is on the default branch and we need to fork.
    pub(crate) branch_hint: Option<&'a str>,
    /// Working directory the command runs in.
    pub(crate) cwd: &'a Path,
}

/// Outcome surface for [`dispatch`]:
///
/// - `Ok(Some(text))` — command handled, text is the user-facing
///   response.
/// - `Ok(None)` — `name` is not a git slash command (caller can route
///   to its own dispatcher).
/// - `Err(...)` — typed git failure; caller renders via `String::from`.
pub(crate) fn dispatch(request: &SlashRequest<'_>) -> GitResult<Option<String>> {
    match request.name {
        "branch" => branch_cmd::handle(request).map(Some),
        "worktree" => worktree_cmd::handle(request).map(Some),
        "diff" => diff_cmd::handle(request).map(Some),
        "commit" => commit_cmd::handle(request).map(Some),
        "commit-push-pr" => commit_push_pr_cmd::handle(request).map(Some),
        "pr" => pr_cmd::handle(request).map(Some),
        "issue" => issue_cmd::handle(request).map(Some),
        _ => Ok(None),
    }
}

/// Static list of names this dispatcher recognises; used by
/// `commands/slash.rs::builtin_specs` so the front-end completion list
/// stays in sync with the dispatcher.
#[must_use]
pub(crate) fn known_command_names() -> &'static [&'static str] {
    &[
        "branch",
        "worktree",
        "diff",
        "commit",
        "commit-push-pr",
        "pr",
        "issue",
    ]
}

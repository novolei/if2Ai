//! `gh issue create` wrapper.

use std::path::Path;
use std::process::Command;

use super::super::command::{run_stdout_async, GH_BIN};
use super::super::error::{GitError, GitResult};
use super::is_gh_available;
use super::pr::parse_pr_url;

/// Inputs for `gh issue create`.
#[derive(Debug, Clone)]
pub(crate) struct IssueCreateRequest<'a> {
    pub(crate) title: &'a str,
    pub(crate) body_file: &'a Path,
}

/// Create a GitHub issue, returning its URL.
pub(crate) fn create(cwd: &Path, request: &IssueCreateRequest<'_>) -> GitResult<String> {
    if !is_gh_available() {
        return Err(GitError::MissingBinary(GH_BIN));
    }

    let body_path = request.body_file.to_string_lossy().into_owned();
    let args: [&str; 6] = [
        "issue",
        "create",
        "--title",
        request.title,
        "--body-file",
        body_path.as_str(),
    ];

    let output = Command::new(GH_BIN).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(GitError::from_output(GH_BIN, &args, &output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `gh issue create` prints the issue URL on its own line; the same
    // helper that handles `gh pr create` works.
    parse_pr_url(stdout.as_ref()).ok_or_else(|| {
        GitError::Parse(format!(
            "could not extract issue URL from `gh issue create` stdout: {stdout}"
        ))
    })
}

/// Async sibling of [`create`] backed by `tokio::process::Command`.
///
/// Same rationale as [`super::pr::create_async`] — `gh issue create`
/// is a network round-trip; running it through tokio frees the
/// blocking-task pool while we wait on GitHub.
pub(crate) async fn create_async(
    cwd: &Path,
    request: &IssueCreateRequest<'_>,
) -> GitResult<String> {
    if !is_gh_available() {
        return Err(GitError::MissingBinary(GH_BIN));
    }

    let body_path = request.body_file.to_string_lossy().into_owned();
    let args: [&str; 6] = [
        "issue",
        "create",
        "--title",
        request.title,
        "--body-file",
        body_path.as_str(),
    ];

    let stdout = run_stdout_async(GH_BIN, &args, cwd).await?;
    parse_pr_url(&stdout).ok_or_else(|| {
        GitError::Parse(format!(
            "could not extract issue URL from `gh issue create` stdout: {stdout}"
        ))
    })
}

//! `/commit` — stage everything and create a commit.
//!
//! The commit message **must** be supplied via [`SlashRequest::message`]
//! (the slash layer does not call out to the LLM here; that is the
//! caller's responsibility — typically the desktop UI's commit drawer).

use super::super::commit::commit_all_with_message;
use super::super::error::{GitError, GitResult};
use super::SlashRequest;

/// Dispatch a `/commit` request — stage everything and commit using
/// the message supplied via [`SlashRequest::message`].
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    let message = req.message.ok_or(GitError::CommitMessageRequired)?.trim();
    if message.is_empty() {
        return Err(GitError::EmptyCommitMessage);
    }

    match commit_all_with_message(req.cwd, message) {
        Ok(()) => Ok(format!("Commit\n  Result           created\n\n{message}")),
        Err(GitError::NoWorkspaceChanges) => Ok(
            "Commit\n  Result           skipped\n  Reason           no workspace changes"
                .to_string(),
        ),
        Err(other) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req<'a>(cwd: &'a Path, message: Option<&'a str>) -> SlashRequest<'a> {
        SlashRequest {
            name: "commit",
            args: vec![],
            message,
            title: None,
            branch_hint: None,
            cwd,
        }
    }

    #[test]
    fn missing_message_returns_typed_error() {
        let repo = TestRepo::new("slash-commit-no-msg");
        let err = handle(&req(repo.path(), None)).expect_err("must err");
        assert!(matches!(err, GitError::CommitMessageRequired));
    }

    #[test]
    fn whitespace_message_returns_empty_message_error() {
        let repo = TestRepo::new("slash-commit-blank");
        let err = handle(&req(repo.path(), Some("   \n"))).expect_err("must err");
        assert!(matches!(err, GitError::EmptyCommitMessage));
    }

    #[test]
    fn clean_repo_reports_skipped_not_error() {
        let repo = TestRepo::new("slash-commit-skipped");
        let out = handle(&req(repo.path(), Some("noop"))).expect("ok");
        assert!(out.contains("skipped"));
        assert!(out.contains("no workspace changes"));
    }

    #[test]
    fn dirty_repo_creates_commit() {
        let repo = TestRepo::new("slash-commit-go");
        std::fs::write(repo.path().join("a.txt"), "x\n").expect("write");
        let out = handle(&req(repo.path(), Some("feat: add a"))).expect("commit");
        assert!(out.contains("created"));
        assert!(out.contains("feat: add a"));
    }
}

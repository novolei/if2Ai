//! `/issue` — open a GitHub Issue via `gh`, or fall back to a draft
//! text response when `gh` is not installed.

use super::super::commit::CommitMessageFile;
use super::super::error::{GitError, GitResult};
use super::super::github::{
    is_gh_available,
    issue::{create as gh_issue_create, IssueCreateRequest},
};
use super::SlashRequest;

/// Dispatch an `/issue` request — open an issue via `gh`, or render a
/// draft when `gh` is not installed.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    if !is_gh_available() {
        let title = req.title.map(str::trim).unwrap_or("(untitled)");
        let body = req.message.unwrap_or("").trim();
        return Ok(format!(
            "Issue draft (gh not installed)\n  Title            {title}\n\n{body}"
        ));
    }

    let title = req
        .title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(GitError::MissingRequired("title"))?;
    let body = req.message.unwrap_or("").trim();

    let body_file = CommitMessageFile::create(if body.is_empty() {
        "(no body provided)\n"
    } else {
        body
    })?;
    let url = gh_issue_create(
        req.cwd,
        &IssueCreateRequest {
            title,
            body_file: body_file.path(),
        },
    )?;
    Ok(format!(
        "Issue\n  Result           created\n  Title            {title}\n  URL              {url}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req<'a>(cwd: &'a Path, title: Option<&'a str>, body: Option<&'a str>) -> SlashRequest<'a> {
        SlashRequest {
            name: "issue",
            args: vec![],
            message: body,
            title,
            branch_hint: None,
            cwd,
        }
    }

    #[test]
    fn missing_title_with_gh_present_returns_typed_missing_required() {
        if !crate::modules::git::github::is_gh_available() {
            return;
        }
        let repo = TestRepo::new("slash-issue-no-title");
        let err = handle(&req(repo.path(), None, Some("body"))).expect_err("must err");
        assert!(matches!(err, GitError::MissingRequired("title")));
    }

    #[test]
    fn missing_gh_renders_draft_even_without_title() {
        if crate::modules::git::github::is_gh_available() {
            return;
        }
        let repo = TestRepo::new("slash-issue-draft");
        let out = handle(&req(repo.path(), None, Some("body"))).expect("draft ok");
        assert!(out.contains("Issue draft (gh not installed)"));
    }
}

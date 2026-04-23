//! `/pr` — open a GitHub Pull Request via `gh`, or fall back to a
//! draft text response when `gh` is not installed.

use super::super::branch::detect_default_branch;
use super::super::commit::CommitMessageFile;
use super::super::error::{GitError, GitResult};
use super::super::github::{
    is_gh_available,
    pr::{create as gh_pr_create, PrCreateRequest},
};
use super::SlashRequest;

/// Dispatch a `/pr` request — open a PR via `gh`, or render a draft
/// when `gh` is not installed.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    // Check `gh` first so that on machines without it the user always
    // gets the draft fallback regardless of whether they remembered to
    // pass a title.  (Issue P5-2 from the slash code review.)
    if !is_gh_available() {
        let title = req.title.map(str::trim).unwrap_or("(untitled)");
        let body = req.message.unwrap_or("").trim();
        return Ok(format!(
            "PR draft (gh not installed)\n  Title            {title}\n\n{body}"
        ));
    }

    let title = req
        .title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(GitError::MissingRequired("title"))?;
    let body = req.message.unwrap_or("").trim();

    let body_file = CommitMessageFile::create(if body.is_empty() {
        // gh requires a body file even if empty; supply a placeholder.
        "(no body provided)\n"
    } else {
        body
    })?;
    let base = detect_default_branch(req.cwd)?;
    let outcome = gh_pr_create(
        req.cwd,
        &PrCreateRequest {
            title,
            body_file: body_file.path(),
            base: base.as_str(),
        },
    )?;
    Ok(format!(
        "PR\n  Result           {result}\n  Title            {title}\n  Base             {base}\n  URL              {url}",
        result = if outcome.was_existing { "existing" } else { "created" },
        url = outcome.url
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req<'a>(cwd: &'a Path, title: Option<&'a str>, body: Option<&'a str>) -> SlashRequest<'a> {
        SlashRequest {
            name: "pr",
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
            // Cannot exercise the gh-present branch on this machine;
            // the gh-missing branch is covered by the next test.
            return;
        }
        let repo = TestRepo::new("slash-pr-no-title");
        let err = handle(&req(repo.path(), None, Some("body"))).expect_err("must err");
        assert!(matches!(err, GitError::MissingRequired("title")));
    }

    #[test]
    fn missing_gh_renders_draft_even_without_title() {
        if crate::modules::git::github::is_gh_available() {
            return;
        }
        let repo = TestRepo::new("slash-pr-draft");
        let out = handle(&req(repo.path(), None, Some("body"))).expect("draft ok");
        assert!(out.contains("PR draft (gh not installed)"));
    }

    // The "gh available + happy path" is not tested because creating
    // a real PR requires network + auth; the §10/§7 plan defers gh
    // runtime tests by design.
}

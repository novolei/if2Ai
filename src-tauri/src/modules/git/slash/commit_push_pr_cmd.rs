//! `/commit-push-pr` — high-level orchestration:
//! 1. If on the default branch, create a derived branch from
//!    `branch_hint` (or `title`) and switch to it.
//! 2. If the working tree is dirty, run `commit_all_with_message`.
//! 3. If `<default>...HEAD` produces no diff, skip push + PR.
//! 4. Push the branch with `--set-upstream origin`.
//! 5. Open a PR via `gh pr create`, falling back to `gh pr view` if a
//!    PR already exists; surface the URL either way.
//!
//! `gh` is **required** for this command (mirrors the reference
//! library's behaviour); the `/pr` command on its own degrades to a
//! draft response when `gh` is missing.

use std::path::PathBuf;

use super::super::branch::{build_branch_name, current_branch, detect_default_branch};
use super::super::command::{git_ok, git_stdout, GH_BIN};
use super::super::commit::{commit_all_with_message, has_workspace_changes, CommitMessageFile};
use super::super::error::{GitError, GitResult};
use super::super::github::{
    is_gh_available,
    pr::{
        create as gh_pr_create, create_async as gh_pr_create_async, push_branch_set_upstream,
        push_branch_set_upstream_async, PrCreateRequest,
    },
};
use super::SlashRequest;

/// Dispatch a `/commit-push-pr` request — derive a branch if needed,
/// commit dirty changes, push to origin, and open the PR via `gh`.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    if !is_gh_available() {
        return Err(GitError::MissingBinary(GH_BIN));
    }

    let title = req
        .title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(GitError::MissingRequired("title"))?;
    let body = req.message.unwrap_or("").trim();

    let default_branch = detect_default_branch(req.cwd)?;
    let mut branch = current_branch(req.cwd)?;
    let mut created_branch = false;
    if branch == default_branch {
        let hint = req
            .branch_hint
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(title);
        let next_branch = build_branch_name(hint);
        git_ok(req.cwd, &["switch", "-c", next_branch.as_str()])?;
        branch = next_branch;
        created_branch = true;
    }

    let mut commit_section: Option<String> = None;
    if has_workspace_changes(req.cwd)? {
        if body.is_empty() {
            return Err(GitError::CommitMessageRequired);
        }
        match commit_all_with_message(req.cwd, body) {
            Ok(()) => {
                commit_section = Some(format!("Commit\n  Result           created\n\n{body}"));
            }
            Err(GitError::NoWorkspaceChanges) => {
                // Race: the tree became clean between checks; treat as
                // a benign no-op so push/PR can proceed.
            }
            Err(other) => return Err(other),
        }
    }

    let branch_diff = git_stdout(
        req.cwd,
        &["diff", "--stat", &format!("{default_branch}...HEAD")],
    )?;
    if branch_diff.trim().is_empty() {
        return Ok(
            "Commit/Push/PR\n  Result           skipped\n  Reason           no branch changes to push or open as a pull request"
                .to_string(),
        );
    }

    push_branch_set_upstream(req.cwd, branch.as_str())?;

    let body_file = CommitMessageFile::create(if body.is_empty() {
        "(no body provided)\n"
    } else {
        body
    })?;
    let outcome = gh_pr_create(
        req.cwd,
        &PrCreateRequest {
            title,
            body_file: body_file.path(),
            base: default_branch.as_str(),
        },
    )?;

    let result_label = if outcome.was_existing {
        "existing"
    } else {
        "created"
    };
    let branch_action =
        created_branch.then(|| "  Branch action    created and switched".to_string());

    // Build the response by collecting from an iterator so the
    // optional "Branch action" line is positioned by structure rather
    // than by a fragile `lines.insert(2, ...)` index.
    let lines: Vec<String> = std::iter::once("Commit/Push/PR".to_string())
        .chain(std::iter::once(format!(
            "  Result           {result_label}"
        )))
        .chain(branch_action)
        .chain(std::iter::once(format!("  Branch           {branch}")))
        .chain(std::iter::once(format!(
            "  Base             {default_branch}"
        )))
        .chain(std::iter::once(format!(
            "  URL              {url}",
            url = outcome.url
        )))
        .chain(
            commit_section
                .into_iter()
                .flat_map(|report| [String::new(), report].into_iter()),
        )
        .collect();
    Ok(lines.join("\n"))
}

/// Local-only step group: pre-flight + branch derivation + commit +
/// branch-diff probe.  Returns everything the async network steps need
/// to know.  Designed to run inside `spawn_blocking` so the IPC layer
/// doesn't have to thread sync git calls onto the tokio scheduler.
struct LocalPlan {
    /// Branch we'll push to origin.
    branch: String,
    /// Repo's default branch (used as PR base).
    default_branch: String,
    /// Did we just create the branch?  Surfaces in the response.
    created_branch: bool,
    /// Optional commit-section text to splice into the response when we
    /// produced a new commit on this run.
    commit_section: Option<String>,
    /// `Some(path-to-tempfile)` when we want to actually push + open a
    /// PR; `None` when we early-return with "skipped" (no branch diff).
    body_file: Option<CommitMessageFile>,
}

fn run_local_plan(
    cwd: &std::path::Path,
    title: &str,
    body: &str,
    branch_hint: Option<&str>,
) -> GitResult<Option<LocalPlan>> {
    let default_branch = detect_default_branch(cwd)?;
    let mut branch = current_branch(cwd)?;
    let mut created_branch = false;
    if branch == default_branch {
        let hint = branch_hint
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(title);
        let next_branch = build_branch_name(hint);
        git_ok(cwd, &["switch", "-c", next_branch.as_str()])?;
        branch = next_branch;
        created_branch = true;
    }

    let mut commit_section: Option<String> = None;
    if has_workspace_changes(cwd)? {
        if body.is_empty() {
            return Err(GitError::CommitMessageRequired);
        }
        match commit_all_with_message(cwd, body) {
            Ok(()) => {
                commit_section = Some(format!("Commit\n  Result           created\n\n{body}"));
            }
            Err(GitError::NoWorkspaceChanges) => {
                // Race: tree became clean between checks.
            }
            Err(other) => return Err(other),
        }
    }

    let branch_diff = git_stdout(
        cwd,
        &["diff", "--stat", &format!("{default_branch}...HEAD")],
    )?;
    if branch_diff.trim().is_empty() {
        // Caller writes the "skipped" response; signal absence of work
        // by returning `None`.
        return Ok(None);
    }

    let body_file = CommitMessageFile::create(if body.is_empty() {
        "(no body provided)\n"
    } else {
        body
    })?;

    Ok(Some(LocalPlan {
        branch,
        default_branch,
        created_branch,
        commit_section,
        body_file: Some(body_file),
    }))
}

fn render_skipped() -> String {
    "Commit/Push/PR\n  Result           skipped\n  Reason           no branch changes to push or open as a pull request"
        .to_string()
}

fn render_success(
    plan: &LocalPlan,
    pr_outcome: &super::super::github::pr::PrCreateOutcome,
) -> String {
    let result_label = if pr_outcome.was_existing {
        "existing"
    } else {
        "created"
    };
    let branch_action = plan
        .created_branch
        .then(|| "  Branch action    created and switched".to_string());
    let lines: Vec<String> = std::iter::once("Commit/Push/PR".to_string())
        .chain(std::iter::once(format!(
            "  Result           {result_label}"
        )))
        .chain(branch_action)
        .chain(std::iter::once(format!(
            "  Branch           {branch}",
            branch = plan.branch,
        )))
        .chain(std::iter::once(format!(
            "  Base             {default}",
            default = plan.default_branch,
        )))
        .chain(std::iter::once(format!(
            "  URL              {url}",
            url = pr_outcome.url,
        )))
        .chain(
            plan.commit_section
                .clone()
                .into_iter()
                .flat_map(|report| [String::new(), report].into_iter()),
        )
        .collect();
    lines.join("\n")
}

/// Async sibling of [`handle`] for the typed IPC.
///
/// Local git steps still run on the blocking-task pool (sub-ms each,
/// not worth a tokio scheduler hop) but the two **network** legs —
/// `git push` and `gh pr create` — go straight through
/// [`super::super::github::pr::push_branch_set_upstream_async`] and
/// [`gh_pr_create_async`] so the blocking pool is freed for the
/// duration of the GitHub API round-trips.
pub(crate) async fn handle_async(
    cwd: PathBuf,
    title: String,
    body: String,
    branch_hint: Option<String>,
) -> GitResult<String> {
    if !is_gh_available() {
        return Err(GitError::MissingBinary(GH_BIN));
    }
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(GitError::MissingRequired("title"));
    }
    let body = body.trim().to_string();

    // Stage 1: all sync local git ops on the blocking pool.
    let local_cwd = cwd.clone();
    let local_title = title.clone();
    let local_body = body.clone();
    let local_hint = branch_hint.clone();
    let plan = tokio::task::spawn_blocking(move || {
        run_local_plan(
            &local_cwd,
            local_title.as_str(),
            local_body.as_str(),
            local_hint.as_deref(),
        )
    })
    .await
    .map_err(|join_err| {
        GitError::Internal(format!("commit_push_pr blocking task panicked: {join_err}"))
    })??;

    let Some(mut plan) = plan else {
        return Ok(render_skipped());
    };

    // Stage 2: network legs go directly on the tokio scheduler.
    push_branch_set_upstream_async(&cwd, plan.branch.as_str()).await?;
    let body_file = plan
        .body_file
        .take()
        .ok_or_else(|| GitError::Internal("body_file missing from local plan".to_string()))?;
    let outcome = gh_pr_create_async(
        &cwd,
        &PrCreateRequest {
            title: title.as_str(),
            body_file: body_file.path(),
            base: plan.default_branch.as_str(),
        },
    )
    .await?;
    drop(body_file);

    Ok(render_success(&plan, &outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req<'a>(
        cwd: &'a Path,
        title: Option<&'a str>,
        message: Option<&'a str>,
        hint: Option<&'a str>,
    ) -> SlashRequest<'a> {
        SlashRequest {
            name: "commit-push-pr",
            args: vec![],
            message,
            title,
            branch_hint: hint,
            cwd,
        }
    }

    #[test]
    fn errors_when_gh_missing_or_title_blank() {
        let repo = TestRepo::new("slash-cpp-precheck");
        let err = handle(&req(repo.path(), None, Some("body"), None)).expect_err("must err");
        // Either the local environment has `gh` (and we hit
        // MissingRequired("title")), or it does not
        // (MissingBinary("gh")).  Both are valid pre-flights.
        assert!(matches!(
            err,
            GitError::MissingRequired("title") | GitError::MissingBinary("gh")
        ));
    }
}

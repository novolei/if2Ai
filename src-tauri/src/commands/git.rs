//! Tauri IPC for the git module.
//!
//! Each command takes an explicit top-level `cwd` parameter (the
//! project's `workdir`), delegates to [`crate::modules::git`], and
//! wraps the synchronous subprocess work in
//! `tokio::task::spawn_blocking` so the async runtime is never blocked
//! by `git` / `gh` latency.
//!
//! All commands intentionally use **flat positional arguments** (no
//! struct-shaped `request` parameter); this keeps `cwd` at the same
//! depth across the whole IPC surface so the front-end has a single
//! invocation rule (`{ cwd, ...rest }`).  See plan §10.1 #4.
//!
//! Errors are surfaced as `String` (Tauri convention) via the
//! `impl From<GitError> for String` blanket conversion.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::commands::AppState;
use crate::modules::control_plane::audit::{AuditEmitter, FailureDiagnostic};
use crate::modules::git::{
    branch as git_branch, commit as git_commit_mod,
    github::{self as git_github, issue as git_issue, pr as git_pr},
    repo as git_repo,
    slash::commit_push_pr_cmd,
    status as git_status_mod, worktree as git_worktree,
};
use crate::modules::projects::Project;
use crate::modules::runtime::permissions::PermissionMode;

/// Run a synchronous closure on the blocking-task pool and convert the
/// `JoinError` into a flat `String` for Tauri.  Closures themselves
/// return `Result<T, String>` (use `.map_err(String::from)?` to flatten
/// `GitError`).
async fn run_blocking<F, T>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| err.to_string())?
}

/// **Sandbox guard for every git IPC.**  Verifies that `cwd` resolves
/// (after symlink/.. canonicalisation) to one of the workdirs the
/// `ProjectManager` has on file.
///
/// This is the single bottleneck that prevents a compromised front-end
/// (or an injected prompt that ends up calling Tauri IPC) from running
/// `git commit -A` / `gh pr create` against an arbitrary directory on
/// disk — the IPC takes a raw `cwd: String` from the renderer, so
/// without this check the surface is "ambient authority over every
/// repo on the user's machine".
///
/// Returns the canonicalised path on success so callers can pass that
/// (already-resolved) value into the blocking pool instead of the raw
/// user-supplied string.
async fn assert_cwd_in_registered_projects(state: &AppState, cwd: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(cwd);
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|e| format!("invalid cwd '{cwd}': {e}"))?;
    let projects = state
        .project_manager
        .list_projects()
        .await
        .map_err(|e| format!("failed to list projects: {e}"))?;
    for project in projects {
        let project_path = Path::new(&project.workdir);
        // Canonicalise per-entry so we tolerate symlinks / case
        // differences on macOS HFS+; entries that no longer exist on
        // disk are silently skipped (they can't authorise anything).
        let Ok(canonical_project) = project_path.canonicalize() else {
            continue;
        };
        if canonical_candidate == canonical_project {
            return Ok(canonical_candidate);
        }
    }
    Err(format!(
        "cwd '{cwd}' is not a registered project workdir; refuse to run git here",
    ))
}

/// One-stop audit wrapper for a mutating git IPC.
///
/// Emits `tool_execution_started` at the seam, runs `work` to
/// completion, then emits `tool_execution_finished` on success or
/// `tool_execution_failed` (with the backend's stringified error as
/// `reason`).  Returns the work's `Result` unchanged so callers don't
/// have to remap.
///
/// `tool_name` is the IPC command identifier (e.g. `"git_commit"`),
/// not the underlying CLI name — that way the audit log filters by the
/// surface the user actually invoked.  `session_id` defaults to
/// `"git_ipc"` since these commands run outside the streaming agent
/// loop and have no native session attached.
async fn with_git_audit<F, T>(
    tool_name: &'static str,
    cwd: &std::path::Path,
    work: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let trace_id = AuditEmitter::new_trace_id();
    let session_id = "git_ipc";
    let permission_mode = PermissionMode::WorkspaceWrite;
    AuditEmitter::tool_execution_started(
        &trace_id,
        session_id,
        tool_name,
        cwd,
        permission_mode,
        Some(trace_id.as_str()),
    );
    let started = std::time::Instant::now();
    match work.await {
        Ok(value) => {
            AuditEmitter::tool_execution_finished(
                &trace_id,
                session_id,
                tool_name,
                cwd,
                permission_mode,
                started.elapsed(),
                Some(trace_id.as_str()),
            );
            Ok(value)
        }
        Err(reason) => {
            AuditEmitter::tool_execution_failed(
                &trace_id,
                session_id,
                tool_name,
                cwd,
                permission_mode,
                started.elapsed(),
                FailureDiagnostic {
                    reason: reason.as_str(),
                    error_code: "git_ipc_error",
                    failure_stage: "git_ipc",
                    retryable: false,
                    request_id: Some(trace_id.as_str()),
                },
            );
            Err(reason)
        }
    }
}

// ── Status / Diff ────────────────────────────────────────────────────────────

/// Return the human-readable `git status --short --branch` snapshot for
/// `cwd`.  Resolves to `null` when the working tree is clean enough
/// that `git status` produces no output at all.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_status(state: State<'_, AppState>, cwd: String) -> Result<Option<String>, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || git_status_mod::read_status(&cwd).map_err(String::from)).await
}

/// Return the staged + unstaged diff as a single human-readable block.
/// Resolves to `null` when the tree is clean.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_diff(
    state: State<'_, AppState>,
    cwd: String,
    full: Option<bool>,
) -> Result<Option<String>, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let mode = if full.unwrap_or(false) {
        git_status_mod::DiffMode::Full
    } else {
        git_status_mod::DiffMode::Stat
    };
    run_blocking(move || git_status_mod::read_diff_with_mode(&cwd, mode).map_err(String::from))
        .await
}

/// Cheap probe: is the project workdir inside a git working tree?
///
/// Returns `false` when `git rev-parse --show-toplevel` fails (no
/// `.git`, missing `git` binary, etc.) — never raises.  The front-end
/// uses this to decide whether to enable BranchPicker / GitActionsPicker
/// vs surface "no Git repository" + an init-now affordance.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_is_repo(state: State<'_, AppState>, cwd: String) -> Result<bool, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || Ok(git_repo::is_inside_repo(&cwd))).await
}

/// `git init` in `cwd` — turn an empty / non-git project workdir into
/// a fresh git repository.  Idempotent; the front-end is expected to
/// re-probe `git_is_repo` afterwards to flip its disabled state.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_init_repo(state: State<'_, AppState>, cwd: String) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_init_repo", &cwd, async move {
        run_blocking(move || git_repo::init_repo(&cwd_for_work).map_err(String::from)).await
    })
    .await
}

// ── Branch ──────────────────────────────────────────────────────────────────

/// Return the verbose branch listing (`git branch --list --verbose`).
#[tauri::command]
#[allow(dead_code)]
pub async fn git_branches(state: State<'_, AppState>, cwd: String) -> Result<String, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || git_branch::list_branches_verbose(&cwd).map_err(String::from)).await
}

/// Return the currently checked-out branch (`git branch --show-current`).
/// Returns a typed-error string when the repository is in detached
/// HEAD state ([`crate::modules::git::GitError::NoBranch`]).
#[tauri::command]
#[allow(dead_code)]
pub async fn git_current_branch(state: State<'_, AppState>, cwd: String) -> Result<String, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || git_branch::current_branch(&cwd).map_err(String::from)).await
}

/// Return the repository's default branch using the resolution chain
/// `origin/HEAD` → `main` → `master` → `init.defaultBranch` →
/// current branch.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_default_branch(state: State<'_, AppState>, cwd: String) -> Result<String, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || git_branch::detect_default_branch(&cwd).map_err(String::from)).await
}

/// Switch the working tree to an existing branch (`git checkout <name>`).
///
/// Errors are surfaced as the underlying `git` stderr verbatim — the
/// branch picker UI is expected to render them in a toast (dirty tree,
/// branch missing, etc.).
#[tauri::command]
#[allow(dead_code)]
pub async fn git_checkout_branch(
    state: State<'_, AppState>,
    cwd: String,
    name: String,
) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_checkout_branch", &cwd, async move {
        run_blocking(move || git_branch::checkout(&cwd_for_work, &name).map_err(String::from)).await
    })
    .await
}

/// Create a new branch at HEAD and check it out (`git checkout -b <name>`).
///
/// `name` is taken verbatim — caller (frontend) is responsible for
/// trimming whitespace and warning the user about invalid characters.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_create_branch(
    state: State<'_, AppState>,
    cwd: String,
    name: String,
) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_create_branch", &cwd, async move {
        run_blocking(move || {
            git_branch::create_and_checkout(&cwd_for_work, &name).map_err(String::from)
        })
        .await
    })
    .await
}

// ── Worktree ────────────────────────────────────────────────────────────────

/// Return the human-readable `git worktree list` output.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_worktrees(state: State<'_, AppState>, cwd: String) -> Result<String, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    run_blocking(move || git_worktree::list(&cwd).map_err(String::from)).await
}

/// Add a worktree at `target`.  When `branch` is `Some(_)`:
/// - existing branch → checks it out into the new worktree;
/// - non-existing branch → creates it via `git worktree add … -b`.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_add_worktree(
    state: State<'_, AppState>,
    cwd: String,
    target: String,
    branch: Option<String>,
) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_add_worktree", &cwd, async move {
        run_blocking(move || {
            let target_path = PathBuf::from(&target);
            match branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
                Some(branch) => git_worktree::add_with_branch(&cwd_for_work, &target_path, branch),
                None => git_worktree::add(&cwd_for_work, &target_path),
            }
            .map_err(String::from)
        })
        .await
    })
    .await
}

/// Remove the worktree at `target` (does not force-detach).
#[tauri::command]
#[allow(dead_code)]
pub async fn git_remove_worktree(
    state: State<'_, AppState>,
    cwd: String,
    target: String,
) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_remove_worktree", &cwd, async move {
        run_blocking(move || {
            git_worktree::remove(&cwd_for_work, &PathBuf::from(&target)).map_err(String::from)
        })
        .await
    })
    .await
}

/// `git worktree prune` — drop administrative entries for directories
/// that no longer exist on disk.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_prune_worktrees(state: State<'_, AppState>, cwd: String) -> Result<(), String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_prune_worktrees", &cwd, async move {
        run_blocking(move || git_worktree::prune(&cwd_for_work).map_err(String::from)).await
    })
    .await
}

/// Combined "create worktree → register as project" orchestration.
///
/// Implements §4 of the git-module follow-ups: previously
/// `git_add_worktree` only created the working-tree directory, leaving
/// the user with a fresh checkout that the `ProjectManager` knew
/// nothing about — so the side bar didn't update and there was no way
/// to "继续聊 in this worktree" without a manual refresh + re-add.
///
/// This IPC takes the same `cwd` + `target` + optional `branch` shape
/// as `git_add_worktree`, plus an optional `project_name`:
/// 1. Asserts `cwd` is a registered project (sandbox guard);
/// 2. Runs `git worktree add` (with `-b <branch>` when the branch is
///    absent, plain checkout when it already exists);
/// 3. Canonicalises the new worktree path;
/// 4. Calls `ProjectManager::create_project` with the canonical path
///    so the new worktree shows up in the project list immediately;
/// 5. Returns the freshly-created `Project` so the front-end can
///    switch the active session into it without a second round-trip.
///
/// `project_name` defaults to the directory basename of `target` —
/// matches the convention `ProjectRail` already uses for display
/// names.  `branch.is_none()` is allowed (just `git worktree add`)
/// but most callers will pass a branch hint.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_create_worktree_project(
    state: State<'_, AppState>,
    cwd: String,
    target: String,
    branch: Option<String>,
    project_name: Option<String>,
) -> Result<Project, String> {
    let cwd_canon = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_audit = cwd_canon.clone();
    let project_manager = state.project_manager.clone();
    with_git_audit("git_create_worktree_project", &cwd_for_audit, async move {
        let target_path_for_work = std::path::PathBuf::from(&target);
        let cwd_for_work = cwd_canon.clone();
        let branch_for_work = branch.clone();
        // Stage 1: create the worktree on disk (blocking subprocess).
        run_blocking(move || {
            match branch_for_work
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
            {
                Some(branch) => {
                    git_worktree::add_with_branch(&cwd_for_work, &target_path_for_work, branch)
                }
                None => git_worktree::add(&cwd_for_work, &target_path_for_work),
            }
            .map_err(String::from)
        })
        .await?;

        // Stage 2: canonicalise the new path so the project workdir
        // matches what `assert_cwd_in_registered_projects` will compare
        // against on subsequent IPCs (avoids a same-target-different-
        // symlink mismatch the next time the user runs `/branch` etc.).
        let target_path = std::path::PathBuf::from(&target);
        let canonical_target = target_path.canonicalize().map_err(|e| {
            format!("worktree '{target}' was created but cannot be canonicalised: {e}")
        })?;

        // Stage 3: derive a default project name (last path component)
        // if the caller didn't supply one.
        let display_name = project_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                canonical_target
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "worktree".to_string());

        // Stage 4: register with ProjectManager so the side bar /
        // project list / session loader all see it on next refresh.
        project_manager
            .create_project(display_name, canonical_target)
            .await
            .map_err(|e| format!("worktree created but project register failed: {e}"))
    })
    .await
}

// ── Commit ──────────────────────────────────────────────────────────────────

/// Outcome of [`git_commit`] / [`git_commit_push_pr`] commit step.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CommitOutcome {
    /// `"created"` when a commit was actually recorded; `"skipped"`
    /// when the working tree was clean and there was nothing to commit.
    pub status: String,
    /// Trimmed commit message that was used (or a human-readable
    /// reason for `"skipped"`).
    pub message: String,
}

/// Stage every change (`git add -A`) and commit with `message`.
///
/// Returns `status="created"` on success or `status="skipped"` when
/// the tree had no changes (idempotent UX).  All other failures —
/// missing message, `git add` failure, `git commit` failure — surface
/// as the structured error `String`.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_commit(
    state: State<'_, AppState>,
    cwd: String,
    message: String,
) -> Result<CommitOutcome, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("git_commit", &cwd, async move {
        run_blocking(move || {
            match git_commit_mod::commit_all_with_message(&cwd_for_work, &message) {
                Ok(()) => Ok(CommitOutcome {
                    status: "created".to_string(),
                    message: message.trim().to_string(),
                }),
                Err(crate::modules::git::GitError::NoWorkspaceChanges) => Ok(CommitOutcome {
                    status: "skipped".to_string(),
                    message: "no workspace changes".to_string(),
                }),
                Err(other) => Err(String::from(other)),
            }
        })
        .await
    })
    .await
}

// ── GitHub: PR / Issue ──────────────────────────────────────────────────────

/// Probe whether the `gh` binary is reachable on `PATH`.  Used by the
/// front-end to decide between the live IPC paths and a "draft" UX.
#[tauri::command]
#[allow(dead_code)]
pub async fn gh_available() -> Result<bool, String> {
    // PATH probing is sub-millisecond but we still go through
    // spawn_blocking for symmetry with the rest of this surface and to
    // keep the future-safety promise of "no IO on the async runtime".
    run_blocking(|| Ok(git_github::is_gh_available())).await
}

/// PR create response surfaced to the front-end.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CreatePrResponse {
    /// URL of the created or already-existing pull request.
    pub url: String,
    /// `true` when `gh pr create` failed and we resolved the URL via
    /// `gh pr view --json url` because a PR already existed for the
    /// current branch.
    pub was_existing: bool,
    /// The base branch the PR was opened against (resolved from the
    /// repo's default branch when `base` is `None`).
    pub base: String,
}

/// Open a pull request via `gh pr create`.  When `base` is `None` the
/// repo's default branch is detected automatically.
#[tauri::command]
#[allow(dead_code)]
pub async fn gh_create_pr(
    state: State<'_, AppState>,
    cwd: String,
    title: String,
    body: String,
    base: Option<String>,
) -> Result<CreatePrResponse, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("gh_create_pr", &cwd, async move {
        // Sync prep (sub-millisecond): default-branch probe + tempfile
        // body still go through spawn_blocking.  Then the actual
        // network-bound `gh pr create` runs on tokio so the blocking
        // pool stays free for other tools.
        let cwd_for_prep = cwd_for_work.clone();
        let body_for_prep = body.clone();
        let base_for_prep = base.clone();
        let (base_resolved, body_file) = run_blocking(move || {
            let base = match base_for_prep
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(b) => b.to_string(),
                None => git_branch::detect_default_branch(&cwd_for_prep).map_err(String::from)?,
            };
            let body_file =
                git_commit_mod::CommitMessageFile::create(if body_for_prep.trim().is_empty() {
                    "(no body provided)\n"
                } else {
                    body_for_prep.as_str()
                })
                .map_err(String::from)?;
            Ok::<_, String>((base, body_file))
        })
        .await?;

        let outcome = git_pr::create_async(
            &cwd_for_work,
            &git_pr::PrCreateRequest {
                title: title.trim(),
                body_file: body_file.path(),
                base: base_resolved.as_str(),
            },
        )
        .await
        .map_err(String::from)?;
        // Hold body_file across the await point so the underlying
        // tempfile isn't dropped until `gh` finishes reading it.
        drop(body_file);
        Ok(CreatePrResponse {
            url: outcome.url,
            was_existing: outcome.was_existing,
            base: base_resolved,
        })
    })
    .await
}

/// Open a GitHub issue via `gh issue create`.  Returns the issue URL.
#[tauri::command]
#[allow(dead_code)]
pub async fn gh_create_issue(
    state: State<'_, AppState>,
    cwd: String,
    title: String,
    body: String,
) -> Result<String, String> {
    let cwd = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd.clone();
    with_git_audit("gh_create_issue", &cwd, async move {
        // Same split as gh_create_pr: sync tempfile prep on the
        // blocking pool, then the network call directly on tokio.
        let body_for_prep = body.clone();
        let body_file = run_blocking(move || {
            git_commit_mod::CommitMessageFile::create(if body_for_prep.trim().is_empty() {
                "(no body provided)\n"
            } else {
                body_for_prep.as_str()
            })
            .map_err(String::from)
        })
        .await?;
        let url = git_issue::create_async(
            &cwd_for_work,
            &git_issue::IssueCreateRequest {
                title: title.trim(),
                body_file: body_file.path(),
            },
        )
        .await
        .map_err(String::from)?;
        drop(body_file);
        Ok(url)
    })
    .await
}

// ── /commit-push-pr orchestration as a typed IPC ────────────────────────────

/// Composite "commit + push + open PR" command.
///
/// Mirrors the `/commit-push-pr` slash semantics:
/// 1. If on the default branch, derive a new branch from
///    `branch_hint` (or the PR title) and switch to it.
/// 2. If the working tree is dirty, `git add -A` + `git commit`.
/// 3. If `<default>...HEAD` produces no diff, **skip** push + PR.
/// 4. `git push --set-upstream origin <branch>`.
/// 5. `gh pr create` (falls back to `gh pr view --json url` when the
///    PR already exists).
///
/// Returns the formatted human-readable response that the slash layer
/// produces; the front-end can surface it directly.
///
/// `gh` is **required** (mirrors the reference implementation) — when
/// missing the call fails with `MissingBinary("gh")`.
#[tauri::command]
#[allow(dead_code)]
pub async fn git_commit_push_pr(
    state: State<'_, AppState>,
    cwd: String,
    title: String,
    body: String,
    branch_hint: Option<String>,
) -> Result<String, String> {
    let cwd_path = assert_cwd_in_registered_projects(&state, &cwd).await?;
    let cwd_for_work = cwd_path.clone();
    with_git_audit("git_commit_push_pr", &cwd_path, async move {
        // Network legs (`git push` + `gh pr create`) run on the tokio
        // scheduler via `handle_async`; only the local git steps are
        // shipped to the blocking pool, freeing it during the multi-
        // second GitHub round-trip.
        commit_push_pr_cmd::handle_async(cwd_for_work, title, body, branch_hint)
            .await
            .map_err(String::from)
    })
    .await
}

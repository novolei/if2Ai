//! `git worktree {list,add,remove,prune}` wrappers.
//!
//! Centralises every worktree operation behind the [`super::error`]
//! model so call sites (Tauri IPC, slash commands, project manager)
//! all see structured errors instead of ad-hoc string-formatted
//! `io::Error::other(...)`.

use std::path::{Path, PathBuf};

use super::branch::branch_exists;
use super::command::{git_ok, git_stdout};
use super::error::{GitError, GitResult};

/// `git worktree list` — human-readable default format.
///
/// The output is intended for direct display (slash command response,
/// IPC payload) and is **not** the machine-parseable `--porcelain`
/// format.  Callers that need structured data should request a
/// dedicated variant rather than parsing this string.
pub(crate) fn list(cwd: &Path) -> GitResult<String> {
    Ok(git_stdout(cwd, &["worktree", "list"])?.trim().to_string())
}

/// `git worktree add <target>` with no branch override.
pub(crate) fn add(cwd: &Path, target: &Path) -> GitResult<()> {
    let target_str = path_arg(target);
    git_ok(cwd, &["worktree", "add", target_str.as_str()])
}

/// `git worktree add <target> <branch>` if the branch already exists,
/// otherwise `git worktree add <target> -b <branch>` to create it.
///
/// Mirrors the slash-command semantics from
/// `/rust/crates/commands/src/lib.rs::handle_worktree_slash_command`.
pub(crate) fn add_with_branch(cwd: &Path, target: &Path, branch: &str) -> GitResult<()> {
    let target_str = path_arg(target);
    if branch_exists(cwd, branch) {
        git_ok(cwd, &["worktree", "add", target_str.as_str(), branch])
    } else {
        git_ok(cwd, &["worktree", "add", target_str.as_str(), "-b", branch])
    }
}

/// `git worktree remove <target>` — does not force-detach.
pub(crate) fn remove(cwd: &Path, target: &Path) -> GitResult<()> {
    let target_str = path_arg(target);
    git_ok(cwd, &["worktree", "remove", target_str.as_str()])
}

/// `git worktree prune` — clean up administrative entries for
/// directories that no longer exist.
pub(crate) fn prune(cwd: &Path) -> GitResult<()> {
    git_ok(cwd, &["worktree", "prune"])
}

/// Higher-level helper used by the project layer to create a sibling
/// worktree with a deterministic name and branch prefix.
///
/// Behaviour mirrors the original `create_permanent_worktree` Tauri
/// command: places the new worktree in `<parent>/<slug>-worktree` and
/// creates (or checks out) the branch `<branch_prefix>/<slug>` for it.
///
/// This function owns the **only** target-path computation (preflight
/// existence check + the eventual `git worktree add` argument), so
/// callers cannot drift out of sync with future renames.
///
/// Returns the absolute path to the created worktree on success.
pub(crate) fn create_named(
    source_dir: &Path,
    parent: &Path,
    slug: &str,
    branch_prefix: &str,
) -> GitResult<PathBuf> {
    let target_dir = parent.join(format!("{slug}-worktree"));
    if target_dir.exists() {
        return Err(GitError::WorktreeAlreadyExists(target_dir));
    }
    let branch_name = format!("{branch_prefix}/{slug}");
    add_with_branch(source_dir, &target_dir, branch_name.as_str())?;
    Ok(target_dir)
}

/// Resolve a [`Path`] to a `String` argument for the `git` CLI.
///
/// On macOS and Windows paths are guaranteed UTF-8, and on Linux the
/// shell environments If2Ai ships into are UTF-8 by default; non-UTF-8
/// path components would be replaced with `U+FFFD` here, which `git`
/// would then refuse with a clear error of its own.  We therefore
/// accept the lossy conversion as a documented platform assumption.
fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;

    #[test]
    fn list_on_seed_repo_includes_main_path() {
        let repo = TestRepo::new("worktree-list");
        let out = list(repo.path()).expect("list ok");
        let canonical = repo
            .path()
            .canonicalize()
            .expect("canon repo")
            .to_string_lossy()
            .into_owned();
        assert!(
            out.contains(canonical.as_str()),
            "list output must contain primary worktree path: {out}\nexpected substring: {canonical}"
        );
    }

    #[test]
    fn add_with_branch_creates_new_branch_when_absent() {
        let repo = TestRepo::new("worktree-add-new");
        // Place the worktree *inside* the TempDir so its lifetime is
        // tied to TestRepo: even if a later assertion panics, drop()
        // will recursively clean up.
        let target = repo.path().join("wt-new");
        add_with_branch(repo.path(), &target, "feature/new-branch").expect("add ok");

        let listing = list(repo.path()).expect("list after add");
        assert!(
            listing.contains("feature/new-branch")
                || listing.contains(target.to_string_lossy().as_ref()),
            "expected new worktree in listing: {listing}"
        );
    }

    #[test]
    fn create_named_uses_parent_and_branch_prefix() {
        let repo = TestRepo::new("create-named");
        // Use repo.path() itself as the parent so the created worktree
        // (`demo-slug-worktree/`) stays inside the TempDir.
        let parent = repo.path();
        let slug = "demo-slug";
        let created = create_named(repo.path(), parent, slug, "if2ai").expect("create ok");

        assert!(
            created.ends_with("demo-slug-worktree"),
            "unexpected target dir: {}",
            created.display()
        );
        let listing = list(repo.path()).expect("list");
        assert!(
            listing.contains("if2ai/demo-slug")
                || listing.contains(created.to_string_lossy().as_ref()),
            "expected created branch in listing: {listing}"
        );
    }

    #[test]
    fn create_named_rejects_existing_target_with_typed_error() {
        let repo = TestRepo::new("create-named-exists");
        let parent = repo.path();
        // Pre-create the directory the helper would otherwise build.
        std::fs::create_dir_all(parent.join("collide-worktree")).expect("mkdir collide");

        let err = create_named(repo.path(), parent, "collide", "if2ai").expect_err("must fail");
        match err {
            GitError::WorktreeAlreadyExists(path) => {
                assert!(
                    path.ends_with("collide-worktree"),
                    "unexpected path: {}",
                    path.display()
                );
            }
            other => panic!("expected WorktreeAlreadyExists, got {other:?}"),
        }
    }
}

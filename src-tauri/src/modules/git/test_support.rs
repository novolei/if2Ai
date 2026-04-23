//! Test-only helpers for spinning up disposable git repositories.
//!
//! Compiled only under `#[cfg(test)]`; mirrors the `init_git_repo`
//! pattern from the `/rust` reference library so each test gets an
//! isolated working tree it can mutate freely.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A self-cleaning temporary directory that already contains a git
/// repository with a single seed commit on `main` and an identity
/// configured locally (so commit-creating tests do not depend on the
/// developer's global git config).
pub(crate) struct TestRepo {
    /// Held to keep the directory alive for the lifetime of the test.
    _dir: TempDir,
    path: PathBuf,
}

impl TestRepo {
    pub(crate) fn new(label: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("if2ai-git-test-{label}-"))
            .tempdir()
            .expect("create temp dir for git test repo");
        let path = dir.path().to_path_buf();

        run(&path, "git", &["init", "--initial-branch=main"])
            .or_else(|_| {
                // Older `git` (< 2.28) lacks --initial-branch; fall back.
                run(&path, "git", &["init"])?;
                run(&path, "git", &["branch", "-m", "main"])
            })
            .expect("git init must succeed");

        run(&path, "git", &["config", "user.name", "If2Ai Tests"]).expect("git config user.name");
        run(&path, "git", &["config", "user.email", "tests@if2ai.local"])
            .expect("git config user.email");
        run(&path, "git", &["config", "commit.gpgsign", "false"]).expect("disable gpg sign");

        std::fs::write(path.join("README.md"), "seed\n").expect("write seed file");
        run(&path, "git", &["add", "README.md"]).expect("git add seed");
        run(&path, "git", &["commit", "-m", "chore: seed"]).expect("git commit seed");

        Self { _dir: dir, path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

fn run(cwd: &Path, program: &str, args: &[&str]) -> std::io::Result<()> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

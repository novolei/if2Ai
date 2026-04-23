//! Git + GitHub CLI integration layer.
//!
//! Consolidates the git-related helpers that were previously scattered
//! across `runtime/prompt`, `commands/project`, and the legacy
//! `/rust/crates/{commands,runtime,claw-cli}` reference implementation.
//!
//! The module is intentionally split by responsibility — every file
//! owns a single bounded concern and stays well below the 800-line
//! god-file watchlist:
//!
//! - [`error`]    — [`GitError`] / [`GitResult`] (single error model)
//! - [`command`]  — subprocess execution + PATH probing
//! - [`status`]   — `git status` / `git diff` snapshots
//! - [`branch`]   — current / exists / default-branch detection + slug
//! - [`repo`]     — repository discovery (`rev-parse --show-toplevel`)
//! - [`worktree`] — `git worktree {list,add,remove,prune}`
//! - [`commit`]   — staging + RAII commit-message tempfile
//! - [`github`]   — `gh pr` / `gh issue` wrappers + URL parsing
//! - [`slash`]    — `/branch`, `/worktree`, `/diff`, `/commit`,
//!                  `/commit-push-pr`, `/pr`, `/issue` dispatcher
//!
//! All operations are synchronous (CLI spawning); the Tauri command
//! layer is responsible for wrapping them in `tokio::task::spawn_blocking`.

pub(crate) mod branch;
pub(crate) mod command;
pub(crate) mod commit;
pub(crate) mod error;
pub(crate) mod github;
pub(crate) mod repo;
pub(crate) mod slash;
pub(crate) mod status;
pub(crate) mod worktree;

#[cfg(test)]
pub(crate) mod test_support;

// Re-export the error model at the module root so siblings (commands
// layer, runtime, slash dispatch) can refer to it without depending on
// the internal sub-module path.
#[allow(unused_imports)]
pub(crate) use error::{GitError, GitResult};

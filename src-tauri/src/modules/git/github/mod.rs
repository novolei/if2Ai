//! GitHub-CLI (`gh`) integration.
//!
//! Pure subprocess wrappers around `gh pr create / view`,
//! `gh issue create`, and the URL-parsing helpers needed to surface
//! the resulting URL to the user.  Slash and IPC layers consume these
//! directly; if `gh` is missing they branch on
//! [`is_gh_available`] and fall back to draft text.

pub(crate) mod issue;
pub(crate) mod pr;

use super::command::{command_exists, GH_BIN};

/// Returns `true` if the `gh` binary is reachable on `PATH`.
///
/// Centralised here so the slash and IPC layers do not need to know
/// the binary's name; callers downgrade to "draft" responses when this
/// returns `false`.
#[must_use]
pub(crate) fn is_gh_available() -> bool {
    command_exists(GH_BIN)
}
